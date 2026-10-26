#![doc = include_str!("../README.md")]
// Every public item must say what it is, and nothing in this crate needs `unsafe`.
#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod disk;
mod error;
pub mod file;

pub use disk::Disk;
pub use error::{Error, Result};
pub use file::{File, Open, Vfs};
