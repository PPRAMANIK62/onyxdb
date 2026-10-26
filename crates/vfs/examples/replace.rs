//! Replaces a file by writing a temporary file and renaming it over the old
//! one, three ways, then counts what 1000 crashes left behind each time.
//!
//!     cargo run --example replace

use std::collections::BTreeMap;
use std::path::Path;

use vfs::{Mem, Open, Vfs};

/// Replaces `config` with `contents`: write a temporary file, maybe sync it,
/// rename it over `config`, and maybe sync the directory.
fn replace(vfs: &dyn Vfs, contents: &[u8], sync_file: bool, sync_dir: bool) -> vfs::Result<()> {
    let tmp = vfs.open(Path::new("config.tmp"), Open::CreateNew)?;
    tmp.write_at(contents, 0)?;
    if sync_file {
        tmp.sync()?;
    }
    vfs.rename(Path::new("config.tmp"), Path::new("config"))?;
    if sync_dir {
        vfs.sync_dir(Path::new(""))?;
    }
    Ok(())
}

/// What `config` holds after a crash: a whole version, or what's wrong with it.
fn config(mem: &Mem) -> vfs::Result<String> {
    let Ok(file) = mem.open(Path::new("config"), Open::Existing) else {
        return Ok("no config at all".to_string());
    };
    let mut buf = vec![0; file.len()? as usize];
    file.read_at(&mut buf, 0)?;
    Ok(match buf.as_slice() {
        b"version 1" | b"version 2" => format!("{:?}", String::from_utf8_lossy(&buf)),
        [] => "an empty config".to_string(),
        _ => "a torn config".to_string(),
    })
}

fn main() -> vfs::Result<()> {
    let ways = [
        ("sync the file and the directory", true, true),
        ("skip the directory sync", true, false),
        ("skip the file sync", false, true),
    ];
    for (name, sync_file, sync_dir) in ways {
        let mut outcomes: BTreeMap<String, u32> = BTreeMap::new();
        for seed in 0..1000 {
            let mem = Mem::new();
            replace(&mem, b"version 1", true, true)?;
            replace(&mem, b"version 2", sync_file, sync_dir)?;
            mem.crash(seed);
            *outcomes.entry(config(&mem)?).or_default() += 1;
        }
        println!("{name}:");
        for (outcome, n) in outcomes {
            println!("  {n:4} crashes left {outcome}");
        }
    }
    Ok(())
}
