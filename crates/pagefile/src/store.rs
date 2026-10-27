//! `PageStore`: the trait for reading and writing pages and the header.
//!
//! `PageFile` implements it by reading and writing the file. The freelist and
//! overflow chains are functions that take any `PageStore`. So a cache in
//! front of the file can implement the trait, and they work through the cache
//! too, header and free pages included.

use crate::{Header, Page, PageNo, Result};

/// A place to read and write numbered pages, and the header that describes them.
pub trait PageStore {
    /// The size of every page, in bytes.
    fn page_size(&self) -> usize;

    /// The header in page 0.
    fn read_header(&self) -> Result<Header>;

    /// Replaces the header in page 0.
    fn write_header(&mut self, header: &Header) -> Result<()>;

    /// Reads page `n` into `page` and checks its checksum.
    fn read(&self, n: PageNo, page: &mut Page) -> Result<()>;

    /// Sets the checksum of `page`, then writes it as page `n`. `n` must be
    /// below the header's page count.
    fn write(&mut self, n: PageNo, page: &mut Page) -> Result<()>;

    /// A zeroed page of the right size.
    fn new_page(&self) -> Page {
        Page::new(self.page_size())
    }
}
