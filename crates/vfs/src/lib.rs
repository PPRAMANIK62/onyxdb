//! Fille I/O behind a trait, so a database can run on the real disk or
//! on a file system in memory that loses power on command.
//!
//! - `file`: the `Vfs` and `File` traits every caller uses.
//! - `disk`: `Disk`, the real file system.

// Every public item must say what it is, and nothing in this crate needs `unsafe`.
#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod disk;
mod error;
pub mod file;

pub use disk::Disk;
pub use error::{Error, Result};
pub use file::{File, Open, Vfs};
