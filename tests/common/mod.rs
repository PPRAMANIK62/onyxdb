//! Helpers shared by the integration tests.

// Every test file compiles this module, but each uses only some of it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A path in the temporary directory that's deleted when this value is dropped.
pub struct TempPath(PathBuf);

impl TempPath {
    /// A new path that no other test in this process is using.
    pub fn new(name: &str) -> TempPath {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let file = format!("pagefile-{name}-{}-{n}", std::process::id());
        let path = std::env::temp_dir().join(file);
        let _ = std::fs::remove_file(&path);
        TempPath(path)
    }

    /// The path itself.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
