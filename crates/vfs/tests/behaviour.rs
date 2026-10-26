//! What every `Vfs` must do, checked on the real disk and in memory.

mod common;

use std::path::Path;

use common::TempDir;
use vfs::{Disk, Error, Mem, Open, Vfs};

/// Runs through what a file system must do, starting from an empty one.
fn behaves_like_a_file_system(vfs: &dyn Vfs) {
    let p = Path::new;

    // A new file is empty. Writing past its end leaves a gap of zeros.
    let file = vfs.open(p("a"), Open::CreateNew).unwrap();
    assert_eq!(file.len().unwrap(), 0);
    file.write_at(b"hello", 0).unwrap();
    file.write_at(b"!", 7).unwrap();
    let mut buf = [0; 8];
    file.read_at(&mut buf, 0).unwrap();
    assert_eq!(&buf, b"hello\0\0!");

    // A read that runs past the end fails instead of returning fewer bytes.
    assert!(matches!(
        file.read_at(&mut buf, 4),
        Err(Error::ShortRead { file_len: 8, .. })
    ));
    file.set_len(2).unwrap();
    assert_eq!(file.len().unwrap(), 2);
    file.sync().unwrap();

    // `open` respects what is and isn't there.
    assert!(matches!(
        vfs.open(p("a"), Open::CreateNew),
        Err(Error::AlreadyExists(_))
    ));
    assert!(matches!(
        vfs.open(p("b"), Open::Existing),
        Err(Error::NotFound(_))
    ));

    // Directories hold names, and a directory isn't a file.
    vfs.create_dir(p("d")).unwrap();
    vfs.open(p("d/x"), Open::CreateNew).unwrap();
    assert_eq!(vfs.list(p("")).unwrap(), [p("a"), p("d")]);
    assert_eq!(vfs.list(p("d")).unwrap(), [p("d/x")]);
    assert!(matches!(
        vfs.open(p("d"), Open::Existing),
        Err(Error::WrongType(_))
    ));
    vfs.sync_dir(p("d")).unwrap();
    vfs.sync_dir(p("")).unwrap();

    // A rename replaces the file that had the new name.
    let x = vfs.open(p("d/x"), Open::Existing).unwrap();
    x.write_at(b"x", 0).unwrap();
    vfs.open(p("d/y"), Open::CreateNew).unwrap();
    vfs.rename(p("d/x"), p("d/y")).unwrap();
    assert!(!vfs.exists(p("d/x")).unwrap());
    assert_eq!(
        vfs.open(p("d/y"), Open::Existing).unwrap().len().unwrap(),
        1
    );

    // Removing a name leaves open handles working.
    vfs.remove(p("d/y")).unwrap();
    assert!(!vfs.exists(p("d/y")).unwrap());
    x.write_at(b"still here", 0).unwrap();

    // One handle at a time holds a file's lock, until it's dropped.
    let first = vfs.open(p("a"), Open::Existing).unwrap();
    first.try_lock().unwrap();
    first.try_lock().unwrap();
    let second = vfs.open(p("a"), Open::Existing).unwrap();
    assert!(matches!(second.try_lock(), Err(Error::Locked(_))));
    drop(first);
    second.try_lock().unwrap();

    // Paths stay inside the file system, and renames inside one directory.
    assert!(matches!(
        vfs.rename(p("a"), p("d/a")),
        Err(Error::CrossDirectory { .. })
    ));
    assert!(matches!(
        vfs.open(p("../a"), Open::Existing),
        Err(Error::BadPath(_))
    ));
}

#[test]
fn on_disk() {
    let dir = TempDir::new("behavior");
    behaves_like_a_file_system(&Disk::new(dir.path()));
}

#[test]
fn in_memory() {
    behaves_like_a_file_system(&Mem::new());
}
