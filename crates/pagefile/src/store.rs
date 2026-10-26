//! `PageStore`: the trait for reading, writing, allocating and freeing pages.
//!
//! `PageFile` implements it by reading and writing the file. The functions in
//! `overflow` take any `PageStore`, so they work with other implementations
//! too.

use crate::{Page, PageNo, Result};

/// Somewhere numbered pages can be read, written, allocated and freed.
pub trait PageStore {
    /// The size of every page, in bytes.
    fn page_size(&self) -> usize;

    /// Reads page `n` into `page` and checks its checksum.
    fn read(&self, n: PageNo, page: &mut Page) -> Result<()>;

    /// Sets the checksum of `page`, then writes it as page `n`.
    fn write(&mut self, n: PageNo, page: &mut Page) -> Result<()>;

    /// Returns a page that's free to use, with kind `Empty`: a page from the
    /// freelist if there is one, or a new page at the end of the file.
    fn allocate(&mut self) -> Result<PageNo>;

    /// Puts page `n` on the freelist. Don't use it again until `allocate` returns it.
    fn free(&mut self, n: PageNo) -> Result<()>;

    /// A zeroed page of the right size.
    fn new_page(&self) -> Page {
        Page::new(self.page_size())
    }
}
