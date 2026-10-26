//! Random operations run on the real disk and in memory must agree.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::TempDir;
use vfs::{Disk, File, Mem, Open, Rng, Vfs};

/// The names the operations pick from. `d` is created first.
const NAMES: [&str; 6] = ["a", "b", "c", "d/a", "d/b", "d/c"];

/// The result of an operation, reduced to something both file systems can
/// agree on: the value, or the name of the error's variant.
fn outcome<T: std::fmt::Debug>(result: vfs::Result<T>) -> String {
    match result {
        Ok(value) => format!("ok {value:?}"),
        Err(e) => format!("{e:?}")
            .split(['(', ' ', '{'])
            .next()
            .unwrap_or("")
            .to_string(),
    }
}

#[test]
fn disk_and_memory_agree() {
    for seed in 0..20 {
        let dir = TempDir::new("agree");
        let systems: [Box<dyn Vfs>; 2] = [Box::new(Disk::new(dir.path())), Box::new(Mem::new())];
        run(seed, &systems);
    }
}

/// Runs 300 random operations from `seed` on every system in `systems`,
/// checking that each one gives the same outcome everywhere.
fn run(seed: u64, systems: &[Box<dyn Vfs>; 2]) {
    let mut rng = Rng::new(seed);
    let mut open: BTreeMap<PathBuf, [Box<dyn File>; 2]> = BTreeMap::new();
    for vfs in systems {
        vfs.create_dir(Path::new("d")).unwrap();
    }
    for step in 0..300 {
        let path = PathBuf::from(NAMES[rng.below(NAMES.len())]);
        let other = path.with_file_name(["a", "b", "c"][rng.below(3)]);
        let (offset, len) = (rng.below(1500) as u64, 1 + rng.below(700));
        let bytes = rng.bytes(len);
        let op = rng.below(9);
        let mut results = Vec::new();
        for (i, vfs) in systems.iter().enumerate() {
            let file = open.get(&path).map(|files| &files[i]);
            results.push(match (op, file) {
                (0, _) => outcome(vfs.open(&path, Open::CreateNew).map(|_| ())),
                (1, Some(f)) => outcome(f.write_at(&bytes, offset)),
                (2, Some(f)) => {
                    let mut buf = vec![0; len];
                    outcome(f.read_at(&mut buf, offset).map(|()| buf))
                }
                (3, Some(f)) => outcome(f.set_len(offset)),
                (4, Some(f)) => outcome(f.sync().and_then(|()| f.len())),
                (5, _) => outcome(vfs.rename(&path, &other)),
                (6, _) => outcome(vfs.remove(&path)),
                (7, _) => outcome(
                    vfs.list(Path::new("d"))
                        .and_then(|d| Ok((d, vfs.list(Path::new(""))?))),
                ),
                _ => outcome(vfs.exists(&path)),
            });
        }
        assert_eq!(
            results[0], results[1],
            "seed {seed} step {step}: op {op} on {path:?}"
        );
        // Open the file on both systems whenever it exists, to write to it later.
        if let (Ok(disk), Ok(mem)) = (
            systems[0].open(&path, Open::Existing),
            systems[1].open(&path, Open::Existing),
        ) {
            open.insert(path, [disk, mem]);
        }
    }
    // At the end, every file holds the same bytes on both.
    for name in NAMES {
        let path = Path::new(name);
        let read = |vfs: &dyn Vfs| -> Option<Vec<u8>> {
            let file = vfs.open(path, Open::Existing).ok()?;
            let mut buf = vec![0; file.len().ok()? as usize];
            file.read_at(&mut buf, 0).ok()?;
            Some(buf)
        };
        assert_eq!(
            read(&*systems[0]),
            read(&*systems[1]),
            "seed {seed}: {name}"
        );
    }
}
