//! A file of fixed-size, numbered pages.
//!
//! The file is cut into pages of one size. Page 0 is a header that says what
//! the file is. Every other page holds one kind of thing. Every page ends with
//! a checksum, so a damaged page is caught when it's read.
//!
//! - `page`: page numbers, page kinds and the bytes of one page.
//! - `header`: the header in page 0.

#![warn(missing_docs)]

pub mod crc;
mod error;
pub mod file;
pub mod header;
pub mod page;
pub mod store;

pub use error::{Error, Result};
pub use file::PageFile;
pub use header::Header;
pub use page::{Page, PageKind, PageNo};
pub use store::PageStore;
