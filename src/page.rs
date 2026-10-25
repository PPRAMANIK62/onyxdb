//! Pages, page numbers and page kinds.
//!
//! A page file is cut into pages of one size. Page 0 holds the header (see
//! `header`). Every other page starts with a kind byte that says what the
//! page holds. Every page, page 0 included, ends with a 4-byte checksum
//! (see `crc`).
//!
//! ```text
//! byte 0      1                                   size - 4       size
//!      +------+---------------------------------------+-----------+
//!      | kind |      laid out by the page's kind      |  CRC-32C  |
//!      +------+---------------------------------------+-----------+
//! ```

use std::{fmt, num::NonZeroU32};

use crate::Result;

/// The smallest page size a file can use, in bytes.
pub const MIN_PAGE_SIZE: usize = 512;

/// The largest page size, in bytes. Offsets inside a page are stored as `u16`,
/// which can't hold 65536, so the largest power of two that fits is 32768.
pub const MAX_PAGE_SIZE: usize = 32768;

/// Checks that `size` is a power of two from `MIN_PAGE_SIZE` to `MAX_PAGE_SIZE`.
pub fn check_page_size(size: u32) -> Result<usize> {
    let bytes = size as usize;
    if size.is_power_of_two() && (MIN_PAGE_SIZE..=MAX_PAGE_SIZE).contains(&bytes) {
        Ok(bytes)
    } else {
        Err(crate::Error::BadPageSize(size))
    }
}

/// The number of a page, counting from 0 at the start of the file.
///
/// Page 0 is the header, and nothing ever points to it, so a `PageNo` is
/// never 0. That leaves 0 free to mean "no page" on disk, and keeps
/// `Option<PageNo>` at 4 bytes in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageNo(NonZeroU32);

impl PageNo {
    /// The page numbered `n`, or `None` when `n` is 0.
    pub fn new(n: u32) -> Option<PageNo> {
        NonZeroU32::new(n).map(PageNo)
    }

    /// The page number as a plain integer.
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl fmt::Display for PageNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}

/// A link to another page as it's stored on disk: the page number, or 0 for no page.
pub fn link_to_u32(link: Option<PageNo>) -> u32 {
    link.map_or(0, PageNo::get)
}

/// What a page holds, from its first byte. Page 0 has no kind byte,
/// it starts with the magic string instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    /// Allocated but not written yet: all zeros apart from the checksum.
    Empty = 0,
    /// On the freelist, waiting to be reused.
    Free = 1,
    /// Variable-size items in the slotted format.
    Slotted = 2,
    /// One piece of a value too big for a page.
    Overflow = 3,
}

impl PageKind {
    /// The kind stored as `byte`, or `None` if no kind uses that byte.
    pub fn from_byte(byte: u8) -> Option<PageKind> {
        match byte {
            0 => Some(PageKind::Empty),
            1 => Some(PageKind::Free),
            2 => Some(PageKind::Slotted),
            3 => Some(PageKind::Overflow),
            _ => None,
        }
    }

    /// The kind's name in lowercase, for printing.
    pub fn name(self) -> &'static str {
        match self {
            PageKind::Empty => "empty",
            PageKind::Free => "free",
            PageKind::Slotted => "slotted",
            PageKind::Overflow => "overflow",
        }
    }
}

/// The bytes of one page, in memory.
pub struct Page {
    /// `Box<[u8]>` instead of `Vec<u8>` because a page never grows.
    bytes: Box<[u8]>,
}

impl Page {
    /// A page of `size` zero bytes. `size` should have passed `check_page_size`.
    pub fn new(size: usize) -> Page {
        Page {
            bytes: vec![0; size].into_boxed_slice(),
        }
    }

    /// The page size, in bytes.
    pub fn size(&self) -> usize {
        self.bytes.len()
    }

    /// Every byte of the page.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Every byte of the page, for changing.
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    /// The kind byte, decoded. `None` if the byte matches no kind.
    pub fn kind(&self) -> Option<PageKind> {
        PageKind::from_byte(self.bytes[0])
    }

    /// Sets every byte to zero, then sets the kind byte.
    pub fn reset(&mut self, kind: PageKind) {
        self.bytes.fill(0);
        self.bytes[0] = kind as u8;
    }

    /// Reads the little-endian `u16` at byte `at`.
    pub fn u16_at(&self, at: usize) -> u16 {
        let b = &self.bytes;
        u16::from_le_bytes([b[at], b[at + 1]])
    }

    /// Writes `value` as a little-endian `u16` at byte `at`.
    pub fn put_u16(&mut self, at: usize, value: u16) {
        self.bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    /// Reads the little-endian u32 at byte `at`.
    pub fn u32_at(&self, at: usize) -> u32 {
        let b = &self.bytes;
        u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
    }

    /// Writes `value` as a little-endian u32 at byte `at`.
    pub fn put_u32(&mut self, at: usize, value: u32) {
        self.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
}

#[test]
fn page_sizes() {
    for size in [512, 1024, 4096, 32768] {
        assert_eq!(check_page_size(size).unwrap(), size as usize);
    }
    for size in [0, 256, 1000, 65536] {
        assert!(check_page_size(size).is_err(), "{size} should be rejected");
    }
}

#[test]
fn page_numbers() {
    assert_eq!(PageNo::new(0), None);
    assert_eq!(PageNo::new(7).map(PageNo::get), Some(7));
    assert_eq!(size_of::<Option<PageNo>>(), 4);
}
