//! The errors every operation can return.
//!
//! Each variant names the rule that broke and carries the numbers involved,
//! and `fmt` turns it into a message that says both.

use std::{error, fmt, result};

/// Everything that can go wrong opening or using a page file.
#[derive(Debug)]
pub enum Error {
    /// A page size that isn't a power of two from 512 to 32768.
    BadPageSize(u32),
}

/// What every function in this crate returns.
pub type Result<T> = result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadPageSize(size) => {
                write!(f, "page size {size} is not a power of two from 512 to 32768")
            }
        }
    }
}

impl error::Error for Error {}
