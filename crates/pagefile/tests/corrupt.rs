//! Damaged files: flipped bits, torn writes, and files that aren't page files.

use std::path::Path;

use pagefile::{Error, PageFile};
use vfs::{Mem, Open, Vfs};

/// Makes a file `p.pf` that holds only the header page, with its user bytes
/// set so the page isn't mostly zeros.
fn header_file(mem: &Mem) {
    let mut file = PageFile::create(mem, "p.pf", 512).unwrap();
    file.set_user([0xab; 32]).unwrap();
    file.sync().unwrap();
}

/// Opens `p.pf` as raw bytes, to damage it.
fn raw(mem: &Mem) -> Box<dyn vfs::File> {
    mem.open(Path::new("p.pf"), Open::Existing).unwrap()
}

#[test]
fn other_files_are_refused() {
    let mem = Mem::new();
    let other = mem.open(Path::new("other.db"), Open::CreateNew).unwrap();
    other
        .write_at(b"SQLite format 3\0 and then some more bytes", 0)
        .unwrap();
    assert!(matches!(
        PageFile::open(&mem, "other.db"),
        Err(Error::NotAPageFile)
    ));
    assert!(matches!(
        PageFile::create(&mem, "bad.pf", 1000),
        Err(Error::BadPageSize(1000))
    ));
}

#[test]
fn a_cut_short_file_is_refused() {
    let mem = Mem::new();
    header_file(&mem);
    raw(&mem).set_len(300).unwrap();
    assert!(matches!(
        PageFile::open(&mem, "p.pf"),
        Err(Error::Truncated {
            len: 300,
            expected: 512
        })
    ));
}

#[test]
fn every_single_bit_flip_is_caught() {
    let mem = Mem::new();
    header_file(&mem);
    let raw = raw(&mem);
    let mut flipped = 0;
    // Page 0 is bytes 0..512. Flip each of its 4096 bits in turn, checksum included.
    for byte in 0..512u64 {
        for bit in 0..8 {
            let mut b = [0u8];
            raw.read_at(&mut b, byte).unwrap();
            raw.write_at(&[b[0] ^ (1 << bit)], byte).unwrap();
            // `open` checks the magic, the version and the page size before the
            // checksum, so a flip in those bytes is caught by their own check.
            let opened = PageFile::open(&mem, "p.pf");
            let refused = match byte {
                0..8 => matches!(opened, Err(Error::NotAPageFile)),
                8..10 => matches!(opened, Err(Error::UnsupportedVersion(_))),
                10..14 => matches!(opened, Err(Error::BadPageSize(_))),
                _ => matches!(opened, Err(Error::Checksum { page: 0, .. })),
            };
            assert!(
                refused,
                "flipping bit {bit} of byte {byte} wasn't caught by the right check"
            );
            raw.write_at(&b, byte).unwrap();
            flipped += 1;
        }
    }
    assert_eq!(flipped, 4096);
}

#[test]
fn a_torn_header_is_caught() {
    let mut torn = 0;
    for seed in 0..200 {
        let mem = Mem::new();
        header_file(&mem);
        // Write new user bytes without syncing, then lose power. The crash may
        // keep the old header, the new one, or a sector torn between them.
        PageFile::open(&mem, "p.pf")
            .unwrap()
            .set_user([0xcd; 32])
            .unwrap();
        mem.crash(seed);
        match PageFile::open(&mem, "p.pf") {
            Ok(file) => {
                assert!(file.header().user == [0xab; 32] || file.header().user == [0xcd; 32])
            }
            Err(Error::Checksum { page: 0, .. }) => torn += 1,
            Err(e) => panic!("seed {seed}: {e}"),
        }
    }
    assert!(torn > 0, "some of 200 crashes should tear the header");
}
