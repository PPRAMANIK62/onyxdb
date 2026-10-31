//! The page table, checked against a `HashMap`.

use std::collections::HashMap;

use bufpool::PageTable;
use pagefile::PageNo;
use vfs::Rng;

/// Page number `n`. Panics on 0.
fn no(n: u32) -> PageNo {
    PageNo::new(n).expect("page numbers start at 1")
}

#[test]
fn removing_from_a_run_keeps_the_rest() {
    // Three keys with the same home slot make a run of three slots.
    let mut table = PageTable::new(4);
    let home = table.home(no(1));
    let same: Vec<PageNo> = (1..1000)
        .map(no)
        .filter(|&k| table.home(k) == home)
        .take(3)
        .collect();
    for (i, &k) in same.iter().enumerate() {
        table.insert(k, i);
    }
    // Emptying the first slot of the run must not hide the other two.
    assert_eq!(table.remove(same[0]), Some(0));
    assert_eq!(table.get(same[0]), None);
    assert_eq!(table.get(same[1]), Some(1));
    assert_eq!(table.get(same[2]), Some(2));
}

#[test]
fn random_operations_match_a_hash_map() {
    for seed in 1..=50 {
        let mut rng = Rng::new(seed);
        let mut table = PageTable::new(8);
        let mut model = HashMap::new();
        for step in 0..2000 {
            // Keys from a small range, so they collide and form runs.
            let key = no(1 + rng.below(40) as u32);
            match rng.below(3) {
                // Insert, as long as the table has room for a new key.
                0 if model.len() < 8 || model.contains_key(&key) => {
                    let value = rng.below(1000);
                    assert_eq!(
                        table.insert(key, value),
                        model.insert(key, value),
                        "seed {seed} step {step}"
                    );
                }
                1 => assert_eq!(
                    table.remove(key),
                    model.remove(&key),
                    "seed {seed} step {step}"
                ),
                _ => assert_eq!(
                    table.get(key),
                    model.get(&key).copied(),
                    "seed {seed} step {step}"
                ),
            }
            assert_eq!(table.len(), model.len());
        }
    }
}
