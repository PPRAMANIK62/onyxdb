//! `crash_test` finds a crash bug, and the seed it reports replays it.

use std::path::Path;

use vfs::{Mem, Open, Vfs, crash_test};

/// Writes "abc" to a new file and syncs it, but never syncs the directory.
/// Counts the sync as an acknowledgement.
fn forgets_the_directory(mem: &Mem, acked: &mut bool) -> vfs::Result<()> {
    let file = mem.open(Path::new("f"), Open::CreateNew)?;
    file.write_at(b"abc", 0)?;
    file.sync()?;
    *acked = true;
    Ok(())
}

/// Once the sync was acknowledged, the file must be there with "abc" in it.
fn check(mem: &Mem, acked: &bool) -> Result<(), String> {
    if !*acked {
        return Ok(());
    }
    let file = mem
        .open(Path::new("f"), Open::Existing)
        .map_err(|e| e.to_string())?;
    let mut buf = [0; 3];
    file.read_at(&mut buf, 0).map_err(|e| e.to_string())?;
    if &buf == b"abc" {
        Ok(())
    } else {
        Err(format!("f holds {buf:?}"))
    }
}

#[test]
fn a_missing_sync_dir_is_found_and_replays() {
    let report = crash_test(20, forgets_the_directory, check);
    assert_eq!(
        report.runs,
        4 * 20,
        "three changes, so four places to cut the power"
    );
    let first = report
        .failures
        .first()
        .expect("the missing sync_dir should be found");

    // The reported change count and seed give the same failure again.
    let mem = Mem::new();
    let mut acked = false;
    mem.power_off_after(first.changes);
    let _ = forgets_the_directory(&mem, &mut acked);
    mem.crash(first.seed);
    assert_eq!(check(&mem, &acked), Err(first.message.clone()));
}
