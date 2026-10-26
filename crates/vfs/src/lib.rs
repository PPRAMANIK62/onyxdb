#![doc = include_str!("../README.md")]
// Every public item must say what it is, and nothing in this crate needs `unsafe`.
#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod crash;
pub mod disk;
mod error;
pub mod fault;
pub mod file;
pub mod mem;
pub mod rng;

pub use disk::Disk;
pub use error::{Error, Result};
pub use fault::Fault;
pub use file::{File, Open, Vfs};
pub use mem::Mem;
pub use rng::Rng;
