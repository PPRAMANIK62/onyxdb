//! Injected faults: failed writes, short writes and failed syncs.

use std::path::Path;

use vfs::{Error, Fault, File, Mem, Open, Vfs};

/// A file `f` whose name is synced, holding `old` on the disk.
fn file_holding(mem: &Mem, old: &[u8]) -> Box<dyn File> {
    let file = mem.open(Path::new("f"), Open::CreateNew).unwrap();
    mem.sync_dir(Path::new("")).unwrap();
    file.write_at(old, 0).unwrap();
    file.sync().unwrap();
    file
}

/// All of `file`'s bytes.
fn contents(file: &dyn File) -> Vec<u8> {
    let mut buf = vec![0; file.len().unwrap() as usize];
    file.read_at(&mut buf, 0).unwrap();
    buf
}

#[test]
fn a_failed_write_writes_nothing() {
    let mem = Mem::new();
    let file = file_holding(&mem, b"old old old");
    mem.fail_next(Fault::WriteError);
    assert!(matches!(
        file.write_at(b"NEW NEW NEW", 0),
        Err(Error::Injected(Fault::WriteError))
    ));
    assert_eq!(contents(&*file), b"old old old");
}

#[test]
fn a_short_write_writes_the_first_half() {
    let mem = Mem::new();
    let file = file_holding(&mem, b"old old old!");
    mem.fail_next(Fault::ShortWrite);
    assert!(matches!(
        file.write_at(b"NEW NEW NEW!", 0),
        Err(Error::Injected(Fault::ShortWrite))
    ));
    assert_eq!(contents(&*file), b"NEW NEd old!");
}

#[test]
fn a_failed_sync_loses_the_write_for_good() {
    for seed in 0..100 {
        let mem = Mem::new();
        let file = file_holding(&mem, b"version 1");
        file.write_at(b"version 2", 0).unwrap();
        mem.fail_next(Fault::SyncError);
        assert!(matches!(
            file.sync(),
            Err(Error::Injected(Fault::SyncError))
        ));

        // The new bytes are still there to read, and the next sync succeeds...
        assert_eq!(contents(&*file), b"version 2");
        file.sync().unwrap();

        // ...but they were never saved.
        mem.crash(seed);
        let file = mem.open(Path::new("f"), Open::Existing).unwrap();
        assert_eq!(contents(&*file), b"version 1", "seed {seed}");
    }
}

#[test]
fn faults_wait_for_the_call_they_apply_to() {
    let mem = Mem::new();
    let file = file_holding(&mem, b"x");
    mem.fail_next(Fault::SyncError);
    file.write_at(b"y", 0).unwrap();
    assert!(file.sync().is_err());
    assert!(file.sync().is_ok());
}
