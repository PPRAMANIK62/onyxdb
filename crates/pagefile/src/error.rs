//! The errors every operation can return.
//!
//! Each variant names the rule that broke and carries the numbers involved,
//! and `fmt` turns it into a message that says both.

use std::{error, fmt, result};

use vfs::{self};

use crate::PageKind;

/// Everything that can go wrong opening or using a page file.
#[derive(Debug)]
pub enum Error {
    /// A page size that isn't a power of two from 512 to 32768.
    BadPageSize(u32),
    /// The file doesn't start with the magic string `pagefile`.
    NotAPageFile,
    /// The header's format version is unknown.
    UnsupportedVersion(u16),
    /// A file operation failed: the file system refused, or a simulated one
    /// lost power or injected a fault.
    Vfs(vfs::Error),
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
    /// A slotted page whose header or slots don't add up.
    BadSlottedPage(String),
    /// An item doesn't fit in a slotted page, even after compaction.
    PageFull {
        /// Bytes the item needs, its slot included.
        need: usize,
        /// Bytes the page has free, garbage included.
        room: usize,
    },
    /// The file already holds the largest page number a u32 can count.
    TooManyPages,
    /// A page has a different kind from the one the operation needs.
    WrongKind {
        /// The page number.
        page: u32,
        /// The kind the operation needs.
        expected: PageKind,
        /// The kind byte the page has.
        found: u8,
    },
    /// `free` was called on a page that's already on the freelist.
    DoubleFree(u32),
    /// An overflow chain that ends too early, runs too long, or has a page with
    /// a bad length.
    BadChain(String),
}

/// What every function in this crate returns. Functions over a `PageStore`
/// return the store's error instead.
pub type Result<T, E = Error> = result::Result<T, E>;

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
            Error::Vfs(e) => write!(f, "{e}"),
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
            Error::BadSlottedPage(why) => write!(f, "bad slotted page: {why}"),
            Error::PageFull { need, room } => {
                write!(
                    f,
                    "the item needs {need} bytes, but the page has {room} free"
                )
            }
            Error::TooManyPages => write!(f, "the file can't hold more than 4294967295 pages"),
            Error::WrongKind {
                page,
                expected,
                found,
            } => write!(
                f,
                "page {page} should be a {} page, but its kind byte is {found}",
                expected.name()
            ),
            Error::DoubleFree(page) => write!(f, "page {page} is already free"),
            Error::BadChain(why) => write!(f, "bad overflow chain: {why}"),
        }
    }
}

impl error::Error for Error {}

// `?` on a file operation turns the `vfs::Error` into an `Error` through this.
impl From<vfs::Error> for Error {
    fn from(value: vfs::Error) -> Self {
        Error::Vfs(value)
    }
}
