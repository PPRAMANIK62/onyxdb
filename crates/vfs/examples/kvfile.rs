//! A tiny key-value file, saved whole after every change, in two ways:
//! written over itself, or replaced through a temporary file. `crash_test`
//! cuts the power after every change each way makes, and checks what's left.
//!
//!     cargo run --example kvfile

use std::path::Path;

use vfs::{Mem, Open, Vfs, crash_test};

/// The three versions of the file, saved in order.
const VERSIONS: [&str; 3] = [
    "apple=1\n",
    "apple=1\nbanana=2\n",
    "apple=2\nbanana=2\ncherry=3\n",
];

/// Saves `text` by writing it over the old file in place.
fn save_in_place(vfs: &dyn Vfs, text: &str) -> vfs::Result<()> {
    let path = Path::new("store");
    let file = if vfs.exists(path)? {
        vfs.open(path, Open::Existing)?
    } else {
        let file = vfs.open(path, Open::CreateNew)?;
        vfs.sync_dir(Path::new(""))?;
        file
    };
    file.write_at(text.as_bytes(), 0)?;
    file.set_len(text.len() as u64)?;
    file.sync()
}

/// Saves `text` into a temporary file and renames it over the old one.
fn save_by_rename(vfs: &dyn Vfs, text: &str) -> vfs::Result<()> {
    let tmp = vfs.open(Path::new("store.tmp"), Open::CreateNew)?;
    tmp.write_at(text.as_bytes(), 0)?;
    tmp.sync()?;
    vfs.rename(Path::new("store.tmp"), Path::new("store"))?;
    vfs.sync_dir(Path::new(""))
}

/// After a crash, `store` must hold a whole version, no older than the last
/// one whose save returned. `saved` counts the saves that returned. Before
/// the first one returns, nothing was promised.
fn check(mem: &Mem, saved: &usize) -> Result<(), String> {
    if *saved == 0 {
        return Ok(());
    }
    let path = Path::new("store");
    if !mem.exists(path).map_err(|e| e.to_string())? {
        return Err(format!("store is gone after {saved} saves"));
    }
    let file = mem.open(path, Open::Existing).map_err(|e| e.to_string())?;
    let mut buf = vec![0; file.len().map_err(|e| e.to_string())? as usize];
    file.read_at(&mut buf, 0).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    match VERSIONS.iter().position(|v| *v == text) {
        Some(i) if i + 1 >= *saved => Ok(()),
        Some(i) => Err(format!(
            "store went back to version {} after {saved} saves",
            i + 1
        )),
        None => Err(format!("store holds {text:?}, which is no version at all")),
    }
}

/// A way of saving the file.
type Save = fn(&dyn Vfs, &str) -> vfs::Result<()>;

fn main() {
    let ways: [(&str, Save); 2] = [("in place", save_in_place), ("by rename", save_by_rename)];
    for (name, save) in ways {
        let workload = |mem: &Mem, saved: &mut usize| {
            for text in VERSIONS {
                save(mem, text)?;
                *saved += 1;
            }
            Ok(())
        };
        println!("{name}: {}", crash_test(20, workload, check));
    }
}
