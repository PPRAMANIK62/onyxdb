//! Random operations on a `PageFile`, checked against a plain in-memory model.

use std::collections::{BTreeMap, BTreeSet};

use pagefile::{PageFile, PageKind, PageNo, PageStore, Slotted, check::check, freelist};
use vfs::{Mem, Rng};

#[test]
fn freed_pages_are_reused_before_the_file_grows() {
    let mut file = PageFile::create(&Mem::new(), "reuse.pf", 512).unwrap();
    let pages: Vec<PageNo> = (0..5)
        .map(|_| freelist::allocate(&mut file).unwrap())
        .collect();
    assert_eq!(file.page_count(), 6);

    freelist::free(&mut file, pages[1]).unwrap();
    freelist::free(&mut file, pages[3]).unwrap();
    // The freelist is a stack: the page freed last comes back first.
    assert_eq!(freelist::allocate(&mut file).unwrap(), pages[3]);
    assert_eq!(freelist::allocate(&mut file).unwrap(), pages[1]);
    assert_eq!(
        file.page_count(),
        6,
        "no new pages while free ones were left"
    );
    assert_eq!(freelist::allocate(&mut file).unwrap().get(), 6);
}

#[test]
fn freeing_twice_is_an_error() {
    let mut file = PageFile::create(&Mem::new(), "double.pf", 512).unwrap();
    let n = freelist::allocate(&mut file).unwrap();
    freelist::free(&mut file, n).unwrap();
    assert!(matches!(
        freelist::free(&mut file, n),
        Err(pagefile::Error::DoubleFree(_))
    ));
}

/// What the file should hold: the item written to each page in use, and which pages are free.
#[derive(Default)]
struct Model {
    pages: BTreeMap<u32, Vec<u8>>,
    free: BTreeSet<u32>,
}

#[test]
fn random_operations_match_the_model() {
    for seed in 1..=20 {
        run(seed);
    }
}

/// Runs 400 random steps from `seed`, checks the file against the model, and
/// returns the file.
fn run(seed: u64) -> PageFile {
    let mut rng = Rng::new(seed);
    let mem = Mem::new();
    let mut file = PageFile::create(&mem, "model.pf", 512).unwrap();
    let mut model = Model::default();
    let mut page = file.new_page();

    for step in 0..400 {
        match rng.below(10) {
            // Allocate a page and write one random item into it.
            0..=3 => {
                let n = freelist::allocate(&mut file).unwrap();
                assert!(
                    !model.pages.contains_key(&n.get()),
                    "seed {seed} step {step}: page {n} handed out twice"
                );
                model.free.remove(&n.get());
                let len = 1 + rng.below(100);
                let item = rng.bytes(len);
                Slotted::init(&mut page, 0).insert(0, &item).unwrap();
                file.write(n, &mut page).unwrap();
                model.pages.insert(n.get(), item);
            }
            // Free a page that's in use.
            4..=5 if !model.pages.is_empty() => {
                let number = *model
                    .pages
                    .keys()
                    .nth(rng.below(model.pages.len()))
                    .unwrap();
                freelist::free(&mut file, PageNo::new(number).unwrap()).unwrap();
                model.pages.remove(&number);
                model.free.insert(number);
            }
            // Close the file and open it again.
            6 => {
                drop(file);
                file = PageFile::open(&mem, "model.pf").unwrap();
            }
            // Read a page in use and compare it with the model.
            _ if !model.pages.is_empty() => {
                let (&number, item) = model
                    .pages
                    .iter()
                    .nth(rng.below(model.pages.len()))
                    .unwrap();
                file.read(PageNo::new(number).unwrap(), &mut page).unwrap();
                assert_eq!(
                    Slotted::open(&page).unwrap().get(0),
                    Some(item.as_slice()),
                    "seed {seed} step {step}"
                );
            }
            _ => {}
        }
    }

    // Every page the model knows about must say the same in the file.
    for (&number, item) in &model.pages {
        file.read(PageNo::new(number).unwrap(), &mut page).unwrap();
        assert_eq!(Slotted::open(&page).unwrap().get(0), Some(item.as_slice()));
    }
    for &number in &model.free {
        file.read(PageNo::new(number).unwrap(), &mut page).unwrap();
        assert_eq!(page.kind(), Some(PageKind::Free));
    }
    assert_eq!(file.header().free_count as usize, model.free.len());
    file
}

#[test]
fn random_operations_leave_a_healthy_file() {
    for seed in 1..=20 {
        let file = run(seed);
        let report = check(&file).unwrap();
        assert!(
            report.problems.is_empty(),
            "seed {seed}: {:?}",
            report.problems
        );
    }
}
