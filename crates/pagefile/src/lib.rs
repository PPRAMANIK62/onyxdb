#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod crc;
mod error;
pub mod file;
pub mod freelist;
pub mod header;
pub mod overflow;
pub mod page;
pub mod slotted;
pub mod store;

pub use error::{Error, Result};
pub use file::PageFile;
pub use header::Header;
pub use page::{Page, PageKind, PageNo};
pub use slotted::Slotted;
pub use store::PageStore;
