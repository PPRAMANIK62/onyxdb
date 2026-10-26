//! Damaged files: flipped bits, torn writes, and files that aren't page files.

mod common;

use std::os::unix::fs::FileExt;

use common::TempPath;
use pagefile::{Error, PageFile};

/// Makes a file that holds only the header page, with its user bytes set so
/// the page isn't mostly zeros, and returns its path.
fn header_file(name: &str) -> TempPath {
    let path = TempPath::new(name);
    let mut file = PageFile::create(path.path(), 512).unwrap();
    file.set_user([0xab; 32]).unwrap();
    path
}

#[test]
fn other_files_are_refused() {
    let path = TempPath::new("other");
    std::fs::write(path.path(), b"SQLite format 3\0 and then some more bytes").unwrap();
    assert!(matches!(
        PageFile::open(path.path()),
        Err(Error::NotAPageFile)
    ));

    let path = TempPath::new("bad-size");
    assert!(matches!(
        PageFile::create(path.path(), 1000),
        Err(Error::BadPageSize(1000))
    ));
}

#[test]
fn a_cut_short_file_is_refused() {
    let path = header_file("short");
    let raw = std::fs::OpenOptions::new()
        .write(true)
        .open(path.path())
        .unwrap();
    raw.set_len(300).unwrap();
    assert!(matches!(
        PageFile::open(path.path()),
        Err(Error::Truncated {
            len: 300,
            expected: 512
        })
    ));
}

#[test]
fn every_single_bit_flip_is_caught() {
    let path = header_file("flip");
    let raw = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path.path())
        .unwrap();
    let mut flipped = 0;
    // Page 0 is bytes 0..512. Flip each of its 4096 bits in turn, checksum included.
    for byte in 0..512u64 {
        for bit in 0..8 {
            let mut b = [0u8];
            raw.read_exact_at(&mut b, byte).unwrap();
            raw.write_all_at(&[b[0] ^ (1 << bit)], byte).unwrap();
            // `open` checks the magic, the version and the page size before the
            // checksum, so a flip in those bytes is caught by their own check.
            let refused = match byte {
                0..8 => matches!(PageFile::open(path.path()), Err(Error::NotAPageFile)),
                8..10 => matches!(
                    PageFile::open(path.path()),
                    Err(Error::UnsupportedVersion(_))
                ),
                10..14 => matches!(PageFile::open(path.path()), Err(Error::BadPageSize(_))),
                _ => matches!(
                    PageFile::open(path.path()),
                    Err(Error::Checksum { page: 0, .. })
                ),
            };
            assert!(
                refused,
                "flipping bit {bit} of byte {byte} wasn't caught by the right check"
            );
            raw.write_all_at(&b, byte).unwrap();
            flipped += 1;
        }
    }
    assert_eq!(flipped, 4096);
}

#[test]
fn a_torn_write_is_caught() {
    let path = header_file("torn");
    let raw = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path.path())
        .unwrap();
    let mut old = [0u8; 512];
    raw.read_exact_at(&mut old, 0).unwrap();

    // Write a new version of page 0, then put back the old second half, as if
    // the power failed halfway through the write. The new user bytes are in
    // the first half and the checksum is in the second, so they now disagree.
    PageFile::open(path.path())
        .unwrap()
        .set_user([0xcd; 32])
        .unwrap();
    raw.write_all_at(&old[256..], 256).unwrap();

    assert!(matches!(
        PageFile::open(path.path()),
        Err(Error::Checksum { page: 0, .. })
    ));
}
