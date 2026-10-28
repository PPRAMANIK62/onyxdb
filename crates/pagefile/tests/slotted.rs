//! Random operations on one slotted page, checked against a `Vec` of items.

use pagefile::slotted::{SLOT_LEN, max_item};
use pagefile::{Error, Page, Slotted};
use vfs::Rng;

#[test]
fn random_operations_match_a_vec() {
    for seed in 1..=50 {
        run(seed);
    }
}

fn run(seed: u64) {
    let mut rng = Rng::new(seed);
    let mut page = Page::new(512);
    let mut model: Vec<Vec<u8>> = Vec::new();
    Slotted::init(&mut page, 7);

    for step in 0..2000 {
        let mut s = Slotted::open(&mut page).unwrap();
        let len = rng.below(60);
        let item = rng.bytes(len);
        match rng.below(4) {
            0 | 1 => {
                let i = rng.below(model.len() + 1);
                match s.insert(i, &item) {
                    Ok(()) => model.insert(i, item),
                    Err(Error::PageFull { need, room }) => {
                        assert_eq!(need, item.len() + SLOT_LEN);
                        assert!(
                            need > room,
                            "seed {seed} step {step}: refused an item that fits"
                        );
                    }
                    Err(e) => panic!("seed {seed} step {step}: {e}"),
                }
            }
            2 if !model.is_empty() => {
                let i = rng.below(model.len());
                s.remove(i);
                model.remove(i);
            }
            3 if !model.is_empty() => {
                let i = rng.below(model.len());
                if s.replace(i, &item).is_ok() {
                    model[i] = item;
                }
            }
            _ => s.compact(),
        }
        // `open` checks that the header, the slots and the garbage count still add up.
        let s = Slotted::open(&page).unwrap_or_else(|e| panic!("seed {seed} step {step}: {e}"));
        assert_eq!(s.tag(), 7);
        assert_eq!(s.len(), model.len());
        for (i, item) in model.iter().enumerate() {
            assert_eq!(
                s.get(i),
                Some(item.as_slice()),
                "seed {seed} step {step} item {i}"
            );
        }
    }
}

#[test]
fn a_full_page_compacts_to_make_room() {
    let mut page = Page::new(512);
    let mut s = Slotted::init(&mut page, 0);
    // Fill the page with 40-byte items until the next one doesn't fit.
    let mut count = 0;
    while s.insert(count, &[b'a' + count as u8 % 26; 40]).is_ok() {
        count += 1;
    }
    assert_eq!(count, 11); // 11 × (40 + 4) = 484 of the 500 usable bytes after the header
    // Remove two items in the middle. Their bytes become garbage, not free gap, so only compaction can reuse them.
    s.remove(3);
    s.remove(3);
    s.insert(3, &[b'z'; 80]).unwrap();
    assert_eq!(s.garbage(), 0, "the insert compacted the page first");
    assert_eq!(s.get(3), Some(&[b'z'; 80][..]));
}

#[test]
fn the_largest_item_fills_the_page() {
    let mut page = Page::new(512);
    let mut s = Slotted::init(&mut page, 0);
    let big = vec![1; max_item(512)];
    s.insert(0, &big).unwrap();
    assert_eq!(s.room(), 0);
    assert!(s.insert(1, b"").is_err(), "even an empty item needs a slot");
}
