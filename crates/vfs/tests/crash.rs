//! What a crash keeps: synced data and names always, the rest as the model allows.

use std::collections::BTreeSet;
use std::path::Path;

use vfs::{Error, Mem, Open, Rng, Vfs};

/// Reads all of the file at `path`.
fn read_all(vfs: &dyn Vfs, path: &str) -> Vec<u8> {
    let file = vfs.open(Path::new(path), Open::Existing).unwrap();
    let mut buf = vec![0; file.len().unwrap() as usize];
    file.read_at(&mut buf, 0).unwrap();
    buf
}

#[test]
fn every_byte_is_synced_or_written_later() {
    for seed in 0..300 {
        let mut rng = Rng::new(seed);
        let mem = Mem::new();
        let file = mem.open(Path::new("f"), Open::CreateNew).unwrap();
        mem.sync_dir(Path::new("")).unwrap();
        let synced = rng.bytes(2000);
        file.write_at(&synced, 0).unwrap();
        file.sync().unwrap();

        // After the crash, each byte may hold its synced value (zero past the
        // synced end), or anything a later write put there.
        let mut allowed: Vec<BTreeSet<u8>> = (0..4000)
            .map(|i| BTreeSet::from([*synced.get(i).unwrap_or(&0)]))
            .collect();
        for _ in 0..5 {
            let offset = rng.below(3000);
            let len = 1 + rng.below(900);
            let bytes = rng.bytes(len);
            file.write_at(&bytes, offset as u64).unwrap();
            for (i, &b) in bytes.iter().enumerate() {
                allowed[offset + i].insert(b);
            }
        }
        mem.crash(seed);

        let after = read_all(&mem, "f");
        assert!(
            after.len() >= synced.len(),
            "seed {seed}: synced bytes were lost"
        );
        for (i, b) in after.iter().enumerate() {
            assert!(
                allowed[i].contains(b),
                "seed {seed}: byte {i} is {b}, which nothing wrote there"
            );
        }
    }
}

#[test]
fn a_sector_is_kept_lost_or_torn_from_one_end() {
    let mut seen = BTreeSet::new();
    for seed in 0..1000 {
        let mem = Mem::new();
        let file = mem.open(Path::new("f"), Open::CreateNew).unwrap();
        mem.sync_dir(Path::new("")).unwrap();
        file.write_at(&[b'A'; 512], 0).unwrap();
        file.sync().unwrap();
        file.write_at(&[b'B'; 512], 0).unwrap();
        mem.crash(seed);

        let after = read_all(&mem, "f");
        let bs = after.iter().filter(|&&b| b == b'B').count();
        let shape = if bs == 0 {
            "old"
        } else if bs == 512 {
            "new"
        } else if after[..bs].iter().all(|&b| b == b'B') {
            "new front"
        } else if after[512 - bs..].iter().all(|&b| b == b'B') {
            "new back"
        } else {
            panic!("seed {seed}: the new bytes aren't a run from one end of the sector")
        };
        seen.insert(shape);
    }
    assert_eq!(
        seen.len(),
        4,
        "every shape should turn up in 1000 crashes: {seen:?}"
    );
}

#[test]
fn names_survive_in_order() {
    let p = Path::new;
    for seed in 0..200 {
        let mem = Mem::new();
        mem.open(p("kept"), Open::CreateNew).unwrap();
        mem.sync_dir(p("")).unwrap();
        for name in ["a", "b", "c"] {
            mem.open(p(name), Open::CreateNew).unwrap();
        }
        mem.crash(seed);

        let names = mem.list(p("")).unwrap();
        let names: Vec<_> = names.iter().filter_map(|n| n.to_str()).collect();
        let allowed: [&[&str]; 4] = [
            &["kept"],
            &["a", "kept"],
            &["a", "b", "kept"],
            &["a", "b", "c", "kept"],
        ];
        assert!(
            allowed.contains(&names.as_slice()),
            "seed {seed}: {names:?} isn't a prefix of the creates"
        );
    }
}

#[test]
fn handles_from_before_a_crash_are_stale() {
    let mem = Mem::new();
    let file = mem.open(Path::new("f"), Open::CreateNew).unwrap();
    mem.crash(1);
    assert!(matches!(file.write_at(b"x", 0), Err(Error::Stale(_))));
}

#[test]
fn a_crash_releases_every_lock() {
    let mem = Mem::new();
    let file = mem.open(Path::new("f"), Open::CreateNew).unwrap();
    mem.sync_dir(Path::new("")).unwrap();
    file.try_lock().unwrap();
    mem.crash(1);
    let after = mem.open(Path::new("f"), Open::Existing).unwrap();
    after.try_lock().unwrap();
}

#[test]
fn the_power_goes_off_after_n_changes() {
    let mem = Mem::new();
    let file = mem.open(Path::new("f"), Open::CreateNew).unwrap();
    mem.power_off_after(2);
    file.write_at(b"one", 0).unwrap();
    file.sync().unwrap();
    assert!(matches!(file.write_at(b"two", 3), Err(Error::PowerOff)));
    assert!(matches!(file.sync(), Err(Error::PowerOff)));
    mem.crash(1);
    // The file's name was never synced, so whether it survived depends on the seed.
    let file = mem
        .open(Path::new("f"), Open::CreateNew)
        .or_else(|_| mem.open(Path::new("f"), Open::Existing));
    file.unwrap().write_at(b"back on", 0).unwrap();
}
