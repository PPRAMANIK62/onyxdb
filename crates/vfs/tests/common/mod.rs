//! Helpers shared by the integration tests.

// Every test file compiles this module, but each uses only some of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// An empty directory in the system's temporary directory, deleted with
/// everything in it when this value is dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    /// A new directory that no other test in this process is using.
    pub fn new(name: &str) -> TempDir {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("vfs-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).expect("the temporary directory should be writable");
        TempDir(dir)
    }

    /// The directory's path.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
