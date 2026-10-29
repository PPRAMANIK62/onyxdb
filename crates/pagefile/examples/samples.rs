//! Writes a set of small sample page files into a directory, one for each
//! page layout and each kind of damage, for inspecting with the `pagefile` command.
//!
//!     cargo run --example samples -- DIR

use std::path::Path;

use pagefile::overflow::write_chain;
use pagefile::{PageFile, PageNo, PageStore, Result, Slotted, freelist};
use vfs::{Disk, Open, Vfs};

fn main() -> Result<()> {
    let dir = std::env::args().nth(1).expect("usage: samples DIR");
    let dir = Path::new(&dir);
    let disk = Disk::new(dir);
    let fresh = |name: &str| {
        let _ = std::fs::remove_file(dir.join(name));
        name.to_string()
    };
    let copy =
        |from: &str, to: &str| std::fs::copy(dir.join(from), dir.join(fresh(to))).map(|_| ());

    // An empty file: just the header.
    PageFile::create(&disk, fresh("empty.pf"), 512)?;

    // The running example: one slotted page. Items are inserted out of
    // order on purpose, so slot order and byte order differ.
    let mut file = PageFile::create(&disk, fresh("fruit.pf"), 512)?;
    let n = freelist::allocate(&mut file)?;
    let mut page = file.new_page();
    let mut s = Slotted::init(&mut page, 0);
    s.insert(0, b"banana")?;
    s.insert(0, b"apple")?;
    s.insert(2, b"cherry")?;
    file.write(n, &mut page)?;

    // The same page after removing banana, and after compacting.
    copy("fruit.pf", "holes.pf").expect("copy fruit.pf");
    let mut file = PageFile::open(&disk, "holes.pf")?;
    file.read(n, &mut page)?;
    Slotted::open(&mut page)?.remove(1);
    file.write(n, &mut page)?;
    copy("holes.pf", "packed.pf").expect("copy holes.pf");
    let mut file = PageFile::open(&disk, "packed.pf")?;
    file.read(n, &mut page)?;
    Slotted::open(&mut page)?.compact();
    file.write(n, &mut page)?;

    // Five pages, then pages 2 and 4 freed.
    let mut file = PageFile::create(&disk, fresh("free.pf"), 512)?;
    for name in [&b"one"[..], b"two", b"three", b"four", b"five"] {
        let n = freelist::allocate(&mut file)?;
        Slotted::init(&mut page, 0).insert(0, name)?;
        file.write(n, &mut page)?;
    }
    freelist::free(&mut file, PageNo::new(2).expect("2 is not 0"))?;
    freelist::free(&mut file, PageNo::new(4).expect("4 is not 0"))?;

    // A 1200-byte value in an overflow chain, with a slotted page that
    // records where the chain starts and how long the value is.
    let mut file = PageFile::create(&disk, fresh("long.pf"), 512)?;
    let n = freelist::allocate(&mut file)?;
    let text: Vec<u8> = (0..1200).map(|i| b"0123456789"[i % 10]).collect();
    let first = write_chain(&mut file, &text)?;
    let mut pointer = Vec::new();
    pointer.extend_from_slice(&first.get().to_le_bytes());
    pointer.extend_from_slice(&(text.len() as u32).to_le_bytes());
    Slotted::init(&mut page, 0).insert(0, &pointer)?;
    file.write(n, &mut page)?;

    // fruit.pf with one bit flipped in "apple".
    copy("fruit.pf", "flipped.pf").expect("copy fruit.pf");
    let raw = disk.open(Path::new("flipped.pf"), Open::Existing)?;
    let mut b = [0u8];
    raw.read_at(&mut b, 512 + 497)?;
    raw.write_at(&[b[0] ^ 0b0000_0001], 512 + 497)?;

    // A free page dropped from the freelist: the header forgot page 4.
    copy("free.pf", "leaked.pf").expect("copy free.pf");
    let mut file = PageFile::open(&disk, "leaked.pf")?;
    freelist::allocate(&mut file)?;
    file.read(PageNo::new(4).expect("4 is not 0"), &mut page)?;
    page.reset(pagefile::PageKind::Free);
    file.write(PageNo::new(4).expect("4 is not 0"), &mut page)?;
    Ok(())
}
