//! Faults on demand: a write that fails, a write that stops halfway, and a
//! sync that fails.
//!
//! A failed sync is the dangerous one. On Linux, `fsync` returns the error and
//! the kernel throws away the unsaved writes. Reads still return the new
//! bytes from memory, but they never reach the disk, and the next `fsync`
//! succeeds. `Mem` does the same.

use crate::{Error, Mem, Result, mem::State};

/// A fault `Mem::fail_next` can inject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The next write fails and writes nothing.
    WriteError,
    /// The next write writes the first half of its bytes, then fails.
    ShortWrite,
    /// The next file sync fails, and the writes it was saving are lost.
    SyncError,
}

impl Fault {
    /// The fault's name, for messages.
    pub fn name(self) -> &'static str {
        match self {
            Fault::WriteError => "write error",
            Fault::ShortWrite => "short write",
            Fault::SyncError => "sync error",
        }
    }
}

/// Faults waiting to happen, in the order they were asked for.
#[derive(Default)]
pub(crate) struct Faults {
    waiting: Vec<Fault>,
}

impl Faults {
    /// Takes the first waiting fault that `applies` accepts.
    fn take(&mut self, applies: impl Fn(Fault) -> bool) -> Option<Fault> {
        let i = self.waiting.iter().position(|&f| applies(f))?;
        Some(self.waiting.remove(i))
    }
}

impl Mem {
    /// Makes the next write or sync fail with `fault`. Call it more than once
    /// to queue several faults.
    pub fn fail_next(&self, fault: Fault) {
        self.lock().faults.waiting.push(fault);
    }
}

impl State {
    /// Uses up a waiting write fault, if there is one. Returns how many of the
    /// `len` bytes get written, and what the write returns.
    pub(crate) fn write_fault(&mut self, len: usize) -> (usize, Result<()>) {
        match self.faults.take(|f| f != Fault::SyncError) {
            None => (len, Ok(())),
            Some(Fault::ShortWrite) => (len / 2, Err(Error::Injected(Fault::ShortWrite))),
            Some(fault) => (0, Err(Error::Injected(fault))),
        }
    }

    /// Fails the sync if a `SyncError` is waiting, and throws away the file's
    /// `unsynced` changes, like Linux. `data` keeps them, so reads still see
    /// them until a crash.
    pub(crate) fn sync_fault(&mut self, inode: usize) -> Result<()> {
        if self.faults.take(|f| f == Fault::SyncError).is_some() {
            self.files[inode].unsynced.clear();
            return Err(Error::Injected(Fault::SyncError));
        }
        Ok(())
    }
}
