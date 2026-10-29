//! Random and damaged bytes. Every function that reads a page's layout must
//! return an error for bad bytes, never panic.
//!
//! After the bytes are damaged, each page's checksum is computed again. That
//! way the damage gets past the checksum and reaches the code that reads the
//! layout, which is the code these tests are for.

use std::path::Path;

use pagefile::check::check;
use pagefile::header::MAGIC;
use pagefile::page::BODY;
use pagefile::{Header, Page, PageFile, PageKind, PageStore, Slotted, freelist, overflow};
use vfs::{Mem, Open, Rng, Vfs};

/// The page size every test here uses.
const SIZE: usize = 512;

/// Fills a page with slotted items, then changes a few bytes of its slotted
/// header and slots.
fn damaged_slotted(rng: &mut Rng) -> Page {
    let mut page = Page::new(SIZE);
    let mut slotted = Slotted::init(&mut page, 1);
    for _ in 0..rng.below(10) {
        let len = rng.below(40);
        let item = rng.bytes(len);
        let _ = slotted.insert(slotted.len(), &item);
    }
    for _ in 0..1 + rng.below(4) {
        let at = BODY + rng.below(40);
        page.bytes_mut()[at] = rng.bytes(1)[0];
    }
    page
}

#[test]
fn bad_slotted_pages_return_errors() {
    let mut rng = Rng::new(1);
    let mut opened = 0;
    for case in 0..20_000 {
        // Half the pages are random bytes, and half are real pages with a few bytes changed.
        let mut page = if case % 2 == 0 {
            let mut page = Page::new(SIZE);
            page.bytes_mut().copy_from_slice(&rng.bytes(SIZE));
            page.bytes_mut()[0] = PageKind::Slotted as u8;
            page
        } else {
            damaged_slotted(&mut rng)
        };
        let Ok(mut slotted) = Slotted::open(&mut page) else {
            continue;
        };
        opened += 1;
        for i in 0..slotted.len() {
            slotted.get(i);
        }
        let len = rng.below(60);
        let item = rng.bytes(len);
        let _ = slotted.insert(rng.below(slotted.len() + 1), &item);
        if !slotted.is_empty() {
            slotted.remove(rng.below(slotted.len()));
        }
        slotted.compact();
    }
    // If almost nothing opens, the test isn't reaching the code it's for.
    assert!(opened > 1_000, "only {opened} of 20000 pages opened");
}

#[test]
fn bad_headers_return_errors() {
    let mut rng = Rng::new(2);
    for _ in 0..20_000 {
        let mut page = Page::new(SIZE);
        page.bytes_mut().copy_from_slice(&rng.bytes(SIZE));
        page.bytes_mut()[..8].copy_from_slice(&MAGIC);
        // Version 1, so decoding goes on past the version check.
        page.bytes_mut()[8..10].copy_from_slice(&1u16.to_le_bytes());
        let _ = Header::decode(&page);
    }
}

/// Damages some bytes of `p.pf`, sometimes cuts it short, and then computes
/// every whole page's checksum again.
fn damage_file(mem: &Mem, rng: &mut Rng) {
    let raw = mem.open(Path::new("p.pf"), Open::Existing).unwrap();
    let mut bytes = vec![0; raw.len().unwrap() as usize];
    raw.read_at(&mut bytes, 0).unwrap();
    for _ in 0..1 + rng.below(6) {
        let at = rng.below(bytes.len());
        bytes[at] = rng.bytes(1)[0];
    }
    if rng.below(3) == 0 {
        bytes.truncate(rng.below(bytes.len()));
    }
    let (pages, _) = bytes.as_chunks_mut::<SIZE>();
    for chunk in pages {
        let mut page = Page::new(SIZE);
        page.bytes_mut().copy_from_slice(chunk);
        page.seal();
        chunk.copy_from_slice(page.bytes());
    }
    raw.set_len(0).unwrap();
    raw.write_at(&bytes, 0).unwrap();
}

#[test]
fn damaged_files_return_errors() {
    let mut rng = Rng::new(3);
    let mut opened = 0;
    for _ in 0..1_000 {
        // A file with a 1500-byte value in an overflow chain, a free page,
        // and a slotted page with two items.
        let mem = Mem::new();
        let mut file = PageFile::create(&mem, "p.pf", SIZE as u32).unwrap();
        let chain = overflow::write_chain(&mut file, &rng.bytes(1500)).unwrap();
        let freed = freelist::allocate(&mut file).unwrap();
        let items = freelist::allocate(&mut file).unwrap();
        freelist::free(&mut file, freed).unwrap();
        let mut page = file.new_page();
        let mut slotted = Slotted::init(&mut page, 0);
        slotted.insert(0, b"hello").unwrap();
        slotted.insert(1, b"world").unwrap();
        file.write(items, &mut page).unwrap();
        drop(file);

        damage_file(&mem, &mut rng);
        let Ok(mut file) = PageFile::open(&mem, "p.pf") else {
            continue;
        };
        opened += 1;
        let _ = check(&file);
        let _ = overflow::read_chain(&file, chain, 1500);
        if file.read(items, &mut page).is_ok()
            && let Ok(slotted) = Slotted::open(&page)
        {
            for i in 0..slotted.len() {
                slotted.get(i);
            }
        }
        let _ = freelist::allocate(&mut file);
        let _ = overflow::free_chain(&mut file, chain);
    }
    assert!(opened > 100, "only {opened} of 1000 files opened");
}
