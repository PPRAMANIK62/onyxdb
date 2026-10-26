//! The header in page 0: what the file says about itself.
//!
//! ```text
//! offset  size  field
//!      0     8  magic: the ASCII bytes "pagefile"
//!      8     2  format version, 1
//!     10     4  page size, in bytes
//!     14     4  page count, page 0 included
//!     18     4  first page on the freelist, 0 if the freelist is empty
//!     22     4  number of pages on the freelist
//!     26    32  user bytes, for the code using this crate
//!     58     .  zeros
//! size-4     4  CRC-32C of everything before it
//! ```
//!
//! Every number is little-endian.

use crate::{
    Error, Page, PageNo, Result,
    page::{check_page_size, link_to_u32},
};

/// The magic string at the start of every page file.
pub const MAGIC: [u8; 8] = *b"pagefile";

/// The format version this code reads and writes.
pub const VERSION: u16 = 1;

/// How many bytes the header keeps for the code that uses this crate. This
/// crate never reads them.
pub const USER_LEN: usize = 32;

/// How many bytes at the start of page 0 are enough to learn the page size:
/// the magic string, the version and the page size.
pub const PREFIX_LEN: usize = 14;

/// The header, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// 10: bytes per page.
    pub page_size: u32,
    /// 14: pages in the file, page 0 included.
    pub page_count: u32,
    /// 18: the first page on the freelist.
    pub free_head: Option<PageNo>,
    /// 22: how many pages are on the freelist.
    pub free_count: u32,
    /// 26: bytes this crate never reads, kept for the code that uses it, for
    /// example to remember the page its data starts at.
    pub user: [u8; USER_LEN],
}

/// Reads the page size from the first bytes of a file, after checking the
/// magic string and the version.
pub fn page_size_from_prefix(prefix: &[u8; PREFIX_LEN]) -> Result<usize> {
    if prefix[0..8] != MAGIC {
        return Err(Error::NotAPageFile);
    }
    let version = u16::from_le_bytes([prefix[8], prefix[9]]);
    if version != VERSION {
        return Err(Error::UnsupportedVersion(version));
    }
    let size = u32::from_le_bytes([prefix[10], prefix[11], prefix[12], prefix[13]]);
    check_page_size(size)
}

impl Header {
    /// The header of a new file: only page 0, and an empty freelist.
    pub fn new(page_size: usize) -> Header {
        Header {
            page_size: page_size as u32,
            page_count: 1,
            free_head: None,
            free_count: 0,
            user: [0; USER_LEN],
        }
    }

    /// Writes the header into `page`, erasing whatever was there.
    pub fn encode(&self, page: &mut Page) {
        page.bytes_mut().fill(0);
        page.bytes_mut()[0..8].copy_from_slice(&MAGIC);
        page.put_u16(8, VERSION);
        page.put_u32(10, self.page_size);
        page.put_u32(14, self.page_count);
        page.put_u32(18, link_to_u32(self.free_head));
        page.put_u32(22, self.free_count);
        page.bytes_mut()[26..26 + USER_LEN].copy_from_slice(&self.user);
    }

    /// Reads the header from page 0. Check the page's checksum first.
    pub fn decode(page: &Page) -> Result<Header> {
        let mut prefix = [0; PREFIX_LEN];
        prefix.copy_from_slice(&page.bytes()[..PREFIX_LEN]);
        let page_size = page_size_from_prefix(&prefix)?;
        let mut user = [0; USER_LEN];
        user.copy_from_slice(&page.bytes()[26..26 + USER_LEN]);
        Ok(Header {
            page_size: page_size as u32,
            page_count: page.u32_at(14),
            free_head: PageNo::new(page.u32_at(18)),
            free_count: page.u32_at(22),
            user,
        })
    }
}
