//! `PageFile`: a page file on disk.
//!
//! It creates or opens the file, reads and writes whole pages at
//! `page number x page size`, checks every page's checksum, and keeps the
//! header in page 0 up to date as pages are allocated and freed.
//!
//! Freed pages form the freelist: a chain of free pages, each holding the number of the next one, starting from the header.
//!
//! ```text
//! free page   0       1                  5                  size - 4     size
//!             +-------+------------------+----------------------+----------+
//!             | kind  |  next free page  |       (zeros)        | checksum |
//!             +-------+------------------+----------------------+----------+
//! ```

use std::{
    fs::{File, OpenOptions},
    os::unix::fs::FileExt,
    path::Path,
};

use crate::{
    Error, Header, Page, PageNo, Result,
    header::{PREFIX_LEN, USER_LEN, page_size_from_prefix},
    page::check_page_size,
    store::PageStore,
};

/// An open page file.
pub struct PageFile {
    file: File,
    header: Header,
}

impl PageFile {
    /// The byte offset where page `n` starts.
    fn offset(&self, n: u32) -> u64 {
        n as u64 * self.header.page_size as u64
    }

    /// Reads page `n` into `page`. No range or checksum check.
    fn read_raw(&self, n: u32, page: &mut Page) -> Result<()> {
        self.file.read_exact_at(page.bytes_mut(), self.offset(n))?;
        Ok(())
    }

    /// Writes a `page` as page `n`. No range check, and the checksum is written as it is.
    fn write_raw(&self, n: u32, page: &Page) -> Result<()> {
        self.file.write_all_at(page.bytes(), self.offset(n))?;
        Ok(())
    }

    /// Writes the header into page 0.
    fn write_header(&self) -> Result<()> {
        let mut page = Page::new(self.header.page_size as usize);
        self.header.encode(&mut page);
        page.seal();
        self.write_raw(0, &page)
    }

    /// Checks that `n` is inside the file and that `page` has the file's page size.
    fn check(&self, n: PageNo, page: &Page) -> Result<()> {
        assert_eq!(
            page.size(),
            self.page_size(),
            "the page buffer has the wrong size"
        );
        if n.get() >= self.header.page_count {
            return Err(Error::OutOfRange {
                page: n.get(),
                count: self.header.page_count,
            });
        }
        Ok(())
    }

    /// The header, as of the last change.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The number of pages in the file, page 0 included.
    pub fn page_count(&self) -> u32 {
        self.header.page_count
    }

    /// The file's length on disk, in bytes.
    pub fn file_len(&self) -> Result<u64> {
        Ok(self.file.metadata()?.len())
    }

    /// Replaces the header's user bytes and writes the header.
    pub fn set_user(&mut self, user: [u8; USER_LEN]) -> Result<()> {
        self.header.user = user;
        self.write_header()
    }

    /// Creates a page file at `path` that holds only the header page. Fails
    /// if the file is already there.
    pub fn create(path: impl AsRef<Path>, page_size: u32) -> Result<PageFile> {
        let size = check_page_size(page_size)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        let file = PageFile {
            file,
            header: Header::new(size),
        };
        file.write_header()?;
        Ok(file)
    }

    /// Opens an existing page file and checks its header.
    pub fn open(path: impl AsRef<Path>) -> Result<PageFile> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let len = file.metadata()?.len();
        if len < PREFIX_LEN as u64 {
            return Err(Error::NotAPageFile);
        }
        // The page size is stored inside page 0, so read just enough of page 0
        // to learn it before reading the whole page.
        let mut prefix = [0; PREFIX_LEN];
        file.read_exact_at(&mut prefix, 0)?;
        let size = page_size_from_prefix(&prefix)?;
        if len < size as u64 {
            return Err(Error::Truncated {
                len,
                expected: size as u64,
            });
        }
        let mut page = Page::new(size);
        file.read_exact_at(page.bytes_mut(), 0)?;
        page.verify(0)?;
        let header = Header::decode(&page)?;
        let expected = header.page_count as u64 * size as u64;
        if len < expected {
            return Err(Error::Truncated { len, expected });
        }
        Ok(PageFile { file, header })
    }

    /// Waits until the operating system has put every write so far on the disk.
    pub fn sync(&self) -> Result<()> {
        self.file.sync_all()?;
        Ok(())
    }
}

impl PageStore for PageFile {
    fn page_size(&self) -> usize {
        self.header.page_size as usize
    }

    fn read(&self, n: PageNo, page: &mut Page) -> Result<()> {
        self.check(n, page)?;
        self.read_raw(n.get(), page)?;
        page.verify(n.get())
    }

    fn write(&mut self, n: PageNo, page: &mut Page) -> Result<()> {
        self.check(n, page)?;
        page.seal();
        self.write_raw(n.get(), page)
    }

    fn allocate(&mut self) -> Result<PageNo> {
        todo!()
    }

    fn free(&mut self, n: PageNo) -> Result<()> {
        todo!()
    }
}
