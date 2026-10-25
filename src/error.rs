//! The errors every operation can return.
//!
//! Each variant names the rule that broke and carries the numbers involved,
//! and `fmt` turns it into a message that says both.

use std::{error, fmt, io, result};

/// Everything that can go wrong opening or using a page file.
#[derive(Debug)]
pub enum Error {
    /// A page size that isn't a power of two from 512 to 32768.
    BadPageSize(u32),
    /// The file doesn't start with the magic string `pagefile`.
    NotAPageFile,
    /// The header's format version is unknown.
    UnsupportedVersion(u16),
    /// The operating system failed a read or a write.
    Io(io::Error),
    /// The file is shorter than its header says it should be.
    Truncated {
        /// The file's length, in bytes.
        len: u64,
        /// The length the header implies, in bytes.
        expected: u64,
    },
    /// A page's stored checksum doesn't match its bytes.
    Checksum {
        /// The page number.
        page: u32,
        /// The checksum stored at the end of the page.
        stored: u32,
        /// The checksum of the bytes as they are now.
        computed: u32,
    },
    /// A page number at or past the end of the file.
    OutOfRange {
        /// The page number asked for.
        page: u32,
        /// The number of pages in the file.
        count: u32,
    },
}

/// What every function in this crate returns.
pub type Result<T> = result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadPageSize(size) => {
                write!(
                    f,
                    "page size {size} is not a power of two from 512 to 32768"
                )
            }
            Error::NotAPageFile => {
                write!(f, "not a page file: the first 8 bytes aren't \"pagefile\"")
            }
            Error::UnsupportedVersion(v) => {
                write!(
                    f,
                    "format version {v} is not supported (this code reads version 1)"
                )
            }
            Error::Io(e) => write!(f, "{e}"),
            Error::Truncated { len, expected } => {
                write!(
                    f,
                    "the file is {len} bytes, but its header says it holds {expected}"
                )
            }
            Error::Checksum {
                page,
                stored,
                computed,
            } => {
                write!(
                    f,
                    "page {page} is damaged: its checksum is {stored:08x}, but its bytes give {computed:08x}"
                )
            }
            Error::OutOfRange { page, count } => {
                write!(
                    f,
                    "page {page} is past the end of the file, which has {count} pages"
                )
            }
        }
    }
}

impl error::Error for Error {}

// `?` on an I/O call turns the `io::Error` into an `Error` through this.
impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error::Io(value)
    }
}
