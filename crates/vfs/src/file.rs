//! `Vfs` and `File`: everything a database does with files and directories.
//!
//! A `Vfs` is a file system: it opens files and changes directories.
//!
//! A `File` is one open file: it reads and writes bytes at an offset, changes its
//! length, and syncs.
//!
//! Paths are relative to the root of the file system, so the same path means
//! the same file to every implementation.

use std::path::{Path, PathBuf};

use crate::Result;

/// How `Vfs::open` treats a file that is or isn't there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Open {
    /// Open a file that exists. Fails with `Error::NotFound` if it doesn't.
    Existing,
    /// Create a new, empty file. Fails with `Error::AlreadyExists` if one is there.
    CreateNew,
}

/// The file system: somewhere files can be opened and directories can be changed.
pub trait Vfs: Send + Sync {
    /// Opens or creates the file at `path`.
    fn open(&self, path: &Path, mode: Open) -> Result<Box<dyn File>>;

    /// Whether a file or directory exists at `path`.
    fn exists(&self, path: &Path) -> Result<bool>;

    /// Removes the file at `path`. Handles already open keep working.
    fn remove(&self, path: &Path) -> Result<()>;

    /// Gives the file at `from` the name `to`, replacing any file already
    /// called `to`. Both must be in the same directory.
    fn rename(&self, from: &Path, to: &Path) -> Result<()>;

    /// Creates an empty directory at `path`. Its parent must exist.
    fn create_dir(&self, path: &Path) -> Result<()>;

    /// The paths of everything in the directory `dir`, sorted.
    fn list(&self, dir: &Path) -> Result<Vec<PathBuf>>;

    /// Waits until the files created, removed and renamed in `dir` since the
    /// last `sync_dir` are on the disk.
    fn sync_dir(&self, dir: &Path) -> Result<()>;
}

/// One open file.
pub trait File: Send + Sync {
    /// Fills `buf` with the bytes starting at `offset`. Fails with
    /// `Error::ShortRead` if the file ends first.
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<()>;

    /// Writes all of `buf` starting at `offset`. Writing past the end makes
    /// the file longer, and any gap reads as zeros.
    fn write_at(&self, buf: &[u8], offset: u64) -> Result<()>;

    /// The file's length, in bytes.
    fn len(&self) -> Result<u64>;

    /// Whether the file is empty or not.
    fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Cuts the file to `len` bytes, or makes it longer with zeros.
    fn set_len(&self, len: u64) -> Result<()>;

    /// Waits until every write to this file so far is on the disk.
    fn sync(&self) -> Result<()>;

    /// Locks the file, or fails with `Error::Locked` if another handle has the
    /// lock. The lock is released when this handle is dropped. Calling it again
    /// on the same handle succeeds.
    fn try_lock(&self) -> Result<()>;
}
