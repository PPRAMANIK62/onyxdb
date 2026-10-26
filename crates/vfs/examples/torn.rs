//! Writes a sector of `B`s over a synced sector of `A`s, crashes with seeds
//! 0 to 9, and prints what each crash left.
//!
//!     cargo run --example torn

use std::path::Path;

use vfs::{Mem, Open, Vfs};

fn main() -> vfs::Result<()> {
    for seed in 0..10 {
        let mem = Mem::new();
        let file = mem.open(Path::new("f"), Open::CreateNew)?;
        mem.sync_dir(Path::new(""))?;
        file.write_at(&[b'A'; 512], 0)?;
        file.sync()?;
        file.write_at(&[b'B'; 512], 0)?;
        mem.crash(seed);

        let file = mem.open(Path::new("f"), Open::Existing)?;
        let mut sector = [0; 512];
        file.read_at(&mut sector, 0)?;
        println!("seed {seed}: {}", runs(&sector));
    }
    Ok(())
}

/// Describes `bytes` as runs of the same byte, like "300 × A, 212 × B".
fn runs(bytes: &[u8]) -> String {
    let mut out: Vec<(u8, usize)> = Vec::new();
    for &b in bytes {
        match out.last_mut() {
            Some((last, n)) if *last == b => *n += 1,
            _ => out.push((b, 1)),
        }
    }
    let parts: Vec<String> = out
        .iter()
        .map(|(b, n)| format!("{n} × {}", *b as char))
        .collect();
    parts.join(", ")
}
