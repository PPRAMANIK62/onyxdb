//! A failed sync, step by step: the error, the read that still sees the new
//! bytes, the retry that succeeds, and the crash that shows nothing was saved.
//!
//!     cargo run --example fsyncgate

use std::path::Path;

use vfs::{Fault, File, Mem, Open, Vfs};

/// All of `file`'s bytes, as text.
fn text(file: &dyn File) -> vfs::Result<String> {
    let mut buf = vec![0; file.len()? as usize];
    file.read_at(&mut buf, 0)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn main() -> vfs::Result<()> {
    let mem = Mem::new();
    let file = mem.open(Path::new("balance"), Open::CreateNew)?;
    mem.sync_dir(Path::new(""))?;
    file.write_at(b"100", 0)?;
    file.sync()?;
    println!("synced:         {}", text(&*file)?);

    file.write_at(b"250", 0)?;
    mem.fail_next(Fault::SyncError);
    println!("written:        {}", text(&*file)?);
    println!(
        "sync:           {}",
        file.sync()
            .err()
            .map_or("ok".to_string(), |e| e.to_string())
    );
    println!("read again:     {}", text(&*file)?);
    println!(
        "sync again:     {}",
        file.sync()
            .err()
            .map_or("ok".to_string(), |e| e.to_string())
    );

    mem.crash(0);
    let file = mem.open(Path::new("balance"), Open::Existing)?;
    println!("after a crash:  {}", text(&*file)?);
    Ok(())
}
