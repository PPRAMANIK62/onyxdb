//! Slotted pages: many variable-size items in one page.
//!
//! ```text
//! 0      1     9     10      12              14        16
//! +------+-----+-----+-------+---------------+---------+--------+--
//! | kind | LSN | tag | count | content start | garbage | slot 0 | ...
//! +------+-----+-----+-------+---------------+---------+--------+--
//!
//!   --+--------+------------+--------+---------+--------+----------+
//!  ...| slot n |  free gap  | item 1 | garbage | item 0 | checksum |
//!   --+--------+------------+--------+---------+--------+----------+
//!              ^            ^                           ^
//!       16 + 4 x count  content start               size - 4
//! ```
//!
//! Each slot is an offset and a length, two little-endian `u16`s. The slots are
//! in the order the owner of the page chooses, and inserting at position `i`
//! shifts the slots after it. Items are packed from the end of the page toward
//! the slots. A removed item's bytes stay where they were, counted as garbage,
//! until `compact` packs the items again.

use std::ops::{Deref, DerefMut};

use crate::{Error, Page, PageKind, Result, page::CHECKSUM_LEN};

/// 9: a byte the page's owner can use, for example to say what the page holds.
const TAG: usize = 9;
/// 10: the number of slots.
const COUNT: usize = 10;
/// 12: where the item area starts. From here to the checksum is the items and garbage.
const CONTENT: usize = 12;
/// 14: how many bytes in the item area belong to no item.
const GARBAGE: usize = 14;

/// The size of the slotted page header, the kind byte and the LSN included.
/// The first slot starts here.
pub const HEADER_LEN: usize = 16;

/// The size of one slot: an offset and a length.
pub const SLOT_LEN: usize = 4;

/// The largest item a slotted page of `page_size` bytes can hold: the whole
/// page except the header, one slot and the checksum.
pub fn max_item(page_size: usize) -> usize {
    page_size - CHECKSUM_LEN - HEADER_LEN - SLOT_LEN
}

/// A page seen through the slotted format. `B` is `&Page` for reading or
/// `&mut Page` for changing it.
pub struct Slotted<B> {
    page: B,
}

impl<B: Deref<Target = Page>> Slotted<B> {
    /// Opens a page that's already in the slotted format, after checking that
    /// its header and every slot stay inside the page.
    pub fn open(page: B) -> Result<Slotted<B>> {
        if page.kind() != Some(PageKind::Slotted) {
            let found = page.bytes()[0];
            return Err(Error::BadSlottedPage(format!(
                "the kind byte is {found}, not 2"
            )));
        }
        let s = Slotted { page };
        let (slots_end, content, end) = (s.slots_end(), s.content_start(), s.page.usable());
        if slots_end > content || content > end {
            let why = format!(
                "slots end at {slots_end} and items start at {content}, in a page whose items must end by {end}"
            );
            return Err(Error::BadSlottedPage(why));
        }
        let mut used = 0;
        for i in 0..s.len() {
            let (offset, len) = s.slot(i);
            if offset < content || offset + len > end {
                let why = format!(
                    "slot {i} covers bytes {offset}..{}, outside the item area {content}..{end}",
                    offset + len
                );
                return Err(Error::BadSlottedPage(why));
            }
            used += len;
        }
        if used + s.garbage() != end - content {
            let why = format!(
                "items use {used} bytes and garbage {}, but the item area is {} bytes",
                s.garbage(),
                end - content
            );
            return Err(Error::BadSlottedPage(why));
        }
        Ok(s)
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.page.u16_at(COUNT) as usize
    }

    /// Whether the page holds no items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The owner's tag byte.
    pub fn tag(&self) -> u8 {
        self.page.bytes()[TAG]
    }

    /// Item `i`, or `None` if `i` is past the last item.
    pub fn get(&self, i: usize) -> Option<&[u8]> {
        if i >= self.len() {
            return None;
        }
        let (offset, len) = self.slot(i);
        Some(&self.page.bytes()[offset..offset + len])
    }

    /// Bytes free for new items and their slots. This includes the garbage,
    /// which compaction turns back into free space.
    pub fn room(&self) -> usize {
        self.gap() + self.garbage()
    }

    /// The offset and the length stored in slot `i`.
    pub fn slot(&self, i: usize) -> (usize, usize) {
        let at = HEADER_LEN + i * SLOT_LEN;
        (
            self.page.u16_at(at) as usize,
            self.page.u16_at(at + 2) as usize,
        )
    }

    /// Where te item area starts.
    pub fn content_start(&self) -> usize {
        self.page.u16_at(CONTENT) as usize
    }

    /// How many bytes of the item area belong to no item.
    pub fn garbage(&self) -> usize {
        self.page.u16_at(GARBAGE) as usize
    }

    /// The first byte after the last slot.
    fn slots_end(&self) -> usize {
        HEADER_LEN + self.len() * SLOT_LEN
    }

    /// The free bytes between the last slot and the first item.
    fn gap(&self) -> usize {
        self.content_start() - self.slots_end()
    }
}

impl<B: DerefMut<Target = Page>> Slotted<B> {
    /// Formats `page` as an empty slotted page with the given tag,
    /// erasing what was there.
    pub fn init(mut page: B, tag: u8) -> Slotted<B> {
        page.reset(PageKind::Slotted);
        page.bytes_mut()[TAG] = tag;
        let end = page.usable() as u16;
        page.put_u16(CONTENT, end);
        Slotted { page }
    }

    /// Changes the owner's tag byte.
    pub fn set_tag(&mut self, tag: u8) {
        self.page.bytes_mut()[TAG] = tag;
    }

    /// Writes slot `i`.
    fn set_slot(&mut self, i: usize, offset: usize, len: usize) {
        let at = HEADER_LEN + i * SLOT_LEN;
        self.page.put_u16(at, offset as u16);
        self.page.put_u16(at + 2, len as u16);
    }

    /// Inserts `item` as item `i`, moving items `i` and later up by one.
    /// Compacts the page first if that's the only way the item fits.
    ///
    /// Panics if `i > len()` like `Vec::insert`.
    pub fn insert(&mut self, i: usize, item: &[u8]) -> Result<()> {
        assert!(
            i <= self.len(),
            "insert at {i}, but the page has {} items",
            self.len()
        );
        let need = item.len() + SLOT_LEN;
        if need > self.room() {
            return Err(Error::PageFull {
                need,
                room: self.room(),
            });
        }
        if need > self.gap() {
            self.compact();
        }
        let offset = self.content_start() - item.len();
        self.page.bytes_mut()[offset..offset + item.len()].copy_from_slice(item);
        self.page.put_u16(CONTENT, offset as u16);
        // Move slot i.. one place to the right, which frees slot i.
        let at = HEADER_LEN + i * SLOT_LEN;
        let end = self.slots_end();
        self.page.bytes_mut().copy_within(at..end, at + SLOT_LEN);
        self.set_slot(i, offset, item.len());
        let count = self.len() + 1;
        self.page.put_u16(COUNT, count as u16);
        Ok(())
    }

    /// Removes item `i`, moving later items down by one. It's bytes stay where
    /// they are and count as garbage until the next compaction.
    ///
    /// Panics if `i >= len()`, like `Vec::remove`.
    pub fn remove(&mut self, i: usize) {
        assert!(
            i < self.len(),
            "remove {i}, but the page has {} items",
            self.len()
        );
        let (_, len) = self.slot(i);
        let at = HEADER_LEN + i * SLOT_LEN;
        let end = self.slots_end();
        self.page.bytes_mut().copy_within(at + SLOT_LEN..end, at);
        let count = self.len() - 1;
        self.page.put_u16(COUNT, count as u16);
        let garbage = self.garbage() + len;
        self.page.put_u16(GARBAGE, garbage as u16);
    }

    /// Replaces item `i` with `item`. A new item no longer than the old one is
    /// written in place.
    pub fn replace(&mut self, i: usize, item: &[u8]) -> Result<()> {
        assert!(
            i < self.len(),
            "replace {i}, but the page has {} items",
            self.len()
        );
        let (offset, len) = self.slot(i);
        if item.len() <= len {
            self.page.bytes_mut()[offset..offset + item.len()].copy_from_slice(item);
            self.set_slot(i, offset, item.len());
            let garbage = self.garbage() + len - item.len();
            self.page.put_u16(GARBAGE, garbage as u16);
            return Ok(());
        }
        // Check before changing anything. The old item's bytes and slot will be
        // reused, so they count as room
        if item.len() > self.room() + len {
            return Err(Error::PageFull {
                need: item.len(),
                room: self.room() + len,
            });
        }
        self.remove(i);
        self.insert(i, item)
    }

    /// Packs every item against the end of the page, in slot order, so the garbage
    /// becomes part of the free gap.
    pub fn compact(&mut self) {
        let old = self.page.bytes().to_vec();
        let mut end = self.page.usable();
        for i in 0..self.len() {
            let (offset, len) = self.slot(i);
            end -= len;
            self.page.bytes_mut()[end..end + len].copy_from_slice(&old[offset..offset + len]);
            self.set_slot(i, end, len);
        }
        self.page.put_u16(CONTENT, end as u16);
        self.page.put_u16(GARBAGE, 0);
    }
}
