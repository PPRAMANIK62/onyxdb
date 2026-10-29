//! `PageFile`: a page file on disk.
//!
//! It creates or opens the file through a `vfs::Vfs`, reads and writes whole
//! pages at `page number x page size`, checks every page's checksum, and keeps
//! a copy of the header in page 0 in memory.
//!
//! Freed pages form the freelist: a chain of free pages, each holding the number of the next one, starting from the header.
//!
//! ```text
//! free page   0       1                  5                  size - 4     size
//!             +-------+------------------+----------------------+----------+
//!             | kind  |  next free page  |       (zeros)        | checksum |
//!             +-------+------------------+----------------------+----------+
//! ```

use std::path::Path;

use vfs::{File, Open, Vfs};

use crate::{
    Error, Header, Page, PageNo, Result,
    header::{PREFIX_LEN, USER_LEN, page_size_from_prefix},
    page::check_page_size,
    store::PageStore,
};

/// An open page file.
pub struct PageFile {
    file: Box<dyn File>,
    header: Header,
}

impl PageFile {
    /// The byte offset where page `n` starts.
    fn offset(&self, n: u32) -> u64 {
        n as u64 * self.header.page_size as u64
    }

    /// Reads page `n` into `page` and checks its checksum. It doesn't check
    /// `n` against the page count: `read` does, and a cache in front of the
    /// file checks it against its own header.
    pub fn read_page(&self, n: u32, page: &mut Page) -> Result<()> {
        assert_eq!(
            page.size(),
            self.header.page_size as usize,
            "the page buffer has the wrong size"
        );
        self.file.read_at(page.bytes_mut(), self.offset(n))?;
        page.verify(n)
    }

    /// Sets the checksum of `page` and writes it as page `n`. Like
    /// `read_page`, it doesn't check `n` against the page count.
    pub fn write_page(&self, n: u32, page: &mut Page) -> Result<()> {
        assert_eq!(
            page.size(),
            self.header.page_size as usize,
            "the page buffer has the wrong size"
        );
        page.seal();
        self.file.write_at(page.bytes(), self.offset(n))?;
        Ok(())
    }

    /// Writes the header into page 0.
    fn save_header(&self) -> Result<()> {
        let mut page = Page::new(self.header.page_size as usize);
        self.header.encode(&mut page);
        self.write_page(0, &mut page)
    }

    /// Checks that page `n` is inside the file.
    fn check(&self, n: PageNo) -> Result<()> {
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
        Ok(self.file.len()?)
    }

    /// Replaces the header's user bytes and writes the header.
    pub fn set_user(&mut self, user: [u8; USER_LEN]) -> Result<()> {
        self.header.user = user;
        self.save_header()
    }

    /// Creates a page file at `path` in `vfs` that holds only the header
    /// page, and makes it durable: the file and its name are both synced.
    /// Fails if a file is already there.
    pub fn create(vfs: &dyn Vfs, path: impl AsRef<Path>, page_size: u32) -> Result<PageFile> {
        let size = check_page_size(page_size)?;
        let path = path.as_ref();
        let file = vfs.open(path, Open::CreateNew)?;
        let file = PageFile {
            file,
            header: Header::new(size),
        };
        file.save_header()?;
        file.sync()?;
        vfs.sync_dir(path.parent().unwrap_or(Path::new("")))?;
        Ok(file)
    }

    /// Opens an existing page file at `path` in `vfs` and checks its header.
    pub fn open(vfs: &dyn Vfs, path: impl AsRef<Path>) -> Result<PageFile> {
        let file = vfs.open(path.as_ref(), Open::Existing)?;
        let len = file.len()?;
        if len < PREFIX_LEN as u64 {
            return Err(Error::NotAPageFile);
        }
        // The page size is stored inside page 0, so read just enough of page 0
        // to learn it before reading the whole page.
        let mut prefix = [0; PREFIX_LEN];
        file.read_at(&mut prefix, 0)?;
        let size = page_size_from_prefix(&prefix)?;
        if len < size as u64 {
            return Err(Error::Truncated {
                len,
                expected: size as u64,
            });
        }
        let mut page = Page::new(size);
        file.read_at(page.bytes_mut(), 0)?;
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
        self.file.sync()?;
        Ok(())
    }
}

impl PageStore for PageFile {
    fn page_size(&self) -> usize {
        self.header.page_size as usize
    }

    fn read_header(&self) -> Result<Header> {
        Ok(self.header.clone())
    }

    fn write_header(&mut self, header: &Header) -> Result<()> {
        self.header = header.clone();
        self.save_header()
    }

    fn read(&self, n: PageNo, page: &mut Page) -> Result<()> {
        self.check(n)?;
        self.read_page(n.get(), page)
    }

    fn write(&mut self, n: PageNo, page: &mut Page) -> Result<()> {
        self.check(n)?;
        self.write_page(n.get(), page)
    }
}
