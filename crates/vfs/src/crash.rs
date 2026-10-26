//! `Mem::crash`: what a power failure keeps and what it loses.
//!
//! Synced data and synced names always survive. Everything after the last
//! sync is decided by the seed:
//!
//! ```text
//! a write     cut into 512-byte sectors; each sector is kept, lost, or torn
//!             (new bytes from one end of the sector, old bytes after them)
//! a set_len   kept or lost
//! names       the first k changes since the directory's last sync_dir,
//!             in order, for some k from 0 to all of them
//! ```

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{
    Error, Mem, Result, Rng,
    fault::Faults,
    mem::{Change, Dir, Inode, Node, State, write_into},
};

/// The size of a sector, the unit a disk writes in, in bytes.
pub const SECTOR: u64 = 512;

impl Mem {
    /// How many changes have been made so far: writes, `set_len`s, syncs,
    /// and creating, removing and renaming files and directories.
    pub fn changes(&self) -> u64 {
        self.lock().changes
    }

    /// Lets `n` more changes happen, then cuts the power. After that, every
    /// change fails with `Error::PowerOff` until `crash` is called.
    pub fn power_off_after(&self, n: u64) {
        self.lock().power_left = Some(n);
    }

    /// Simulates a power failure and a restart. `seed` picks what survives, so
    /// the same seed always gives the same result. After the crash, everything
    /// left is synced, the power is on, no faults are waiting, and handles
    /// opened before the crash return `Error::Stale`.
    pub fn crash(&self, seed: u64) {
        let mut rng = Rng::new(seed);
        let mut state = self.lock();
        for inode in &mut state.files {
            inode.data = survivor(inode, &mut rng);
            inode.synced = inode.data.clone();
            inode.unsynced.clear();
        }
        for dir in state.dirs.values_mut() {
            let kept = rng.below(dir.unsynced.len() + 1);
            let mut names = dir.synced.clone();
            for change in &dir.unsynced[..kept] {
                change.apply(&mut names);
            }
            dir.names = names.clone();
            dir.synced = names;
            dir.unsynced.clear();
        }
        drop_unreachable(&mut state.dirs);
        state.generation += 1;
        state.power_left = None;
        state.faults = Faults::default();
        // The process that held them is gone.
        state.locked.clear();
    }
}

impl State {
    /// Counts one change, or refuses it if the power is off.
    pub(crate) fn change(&mut self) -> Result<()> {
        match self.power_left {
            Some(0) => return Err(Error::PowerOff),
            Some(n) => self.power_left = Some(n - 1),
            None => {}
        }
        self.changes += 1;
        Ok(())
    }
}

/// What `inode` holds after a crash. It starts from the synced bytes, and
/// `rng` decides whether each later change is kept, lost or torn.
fn survivor(inode: &Inode, rng: &mut Rng) -> Vec<u8> {
    let mut data = inode.synced.clone();
    for change in &inode.unsynced {
        match change {
            Change::SetLen(len) => {
                if rng.below(2) == 1 {
                    data.resize(*len as usize, 0);
                }
            }
            Change::Write { offset, bytes } => {
                let end = offset + bytes.len() as u64;
                let mut start = *offset;
                // One sector at a time: from `start` to the next sector boundary or the end.
                while start < end {
                    let piece_end = ((start / SECTOR + 1) * SECTOR).min(end);
                    let piece = &bytes[(start - offset) as usize..(piece_end - offset) as usize];
                    let (from, to) = tear(piece.len(), rng);
                    if from < to {
                        write_into(&mut data, start + from as u64, &piece[from..to]);
                    }
                    start = piece_end;
                }
            }
        }
    }
    data
}

/// Which of a sector's `len` new bytes reach the disk: none, all, or part.
/// A part always starts at one end, because a disk writes a sector from one
/// end to the other.
fn tear(len: usize, rng: &mut Rng) -> (usize, usize) {
    match rng.below(4) {
        0 => (0, 0),
        1 => (0, len),
        2 => (0, rng.below(len + 1)),
        _ => (rng.below(len + 1), len),
    }
}

/// Removes directories whose own name didn't survive the crash, and
/// everything below them.
fn drop_unreachable(dirs: &mut BTreeMap<PathBuf, Dir>) {
    // A directory sorts before everything inside it, so each parent is
    // handled before its children.
    let paths: Vec<PathBuf> = dirs.keys().cloned().collect();
    for path in paths {
        let Some(name) = path.file_name() else {
            continue;
        };
        let parent = path.parent().unwrap_or(Path::new(""));
        let named = dirs
            .get(parent)
            .is_some_and(|d| d.names.get(name) == Some(&Node::Dir));
        if !named {
            dirs.remove(&path);
        }
    }
}
