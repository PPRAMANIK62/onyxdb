//! Overflow chains: values too big for one page.
//!
//! A long value is just cut into pieces, one per workflow page, and each page holds
//! the number of the next.
//!
//! ```text
//! 0      1     9                 13           15             size - 4      size
//! +------+-----+-----------------+------------+------------------+----------+
//! | kind | LSN | next page       | bytes used | data             | checksum |
//! +------+-----+-----------------+------------+------------------+----------+
//! ```
//!
//! The chain doesn't store the value's total length: whoever points to the first
//! page keeps it. Every function here takes any `PageStore`.

use crate::{
    Error, PageKind, PageNo, PageStore, Result, freelist,
    page::{CHECKSUM_LEN, link_to_u32},
};

/// 9: the next page in the chain, or 0 on the last page.
pub const NEXT: usize = 9;
/// 13: how many data bytes this page holds.
pub const USED: usize = 13;
/// 15: where the data starts.
pub const DATA: usize = 15;

/// How many data bytes one overflow page holds.
pub fn capacity(page_size: usize) -> usize {
    page_size - CHECKSUM_LEN - DATA
}

/// Stores `data` in a new chain of overflow pages and returns the first page.
///
/// Panics if `data` is empty: a chain holds at least one byte.
pub fn write_chain<S: PageStore + ?Sized>(store: &mut S, data: &[u8]) -> Result<PageNo> {
    assert!(
        !data.is_empty(),
        "an overflow chain needs at least one byte"
    );
    let pieces: Vec<&[u8]> = data.chunks(capacity(store.page_size())).collect();
    let mut pages = Vec::with_capacity(pieces.len());
    for _ in &pieces {
        pages.push(freelist::allocate(store)?);
    }
    let mut page = store.new_page();
    for (i, piece) in pieces.iter().enumerate() {
        page.reset(PageKind::Overflow);
        page.put_u32(NEXT, link_to_u32(pages.get(i + 1).copied()));
        page.put_u16(USED, piece.len() as u16);
        page.bytes_mut()[DATA..DATA + piece.len()].copy_from_slice(piece);
        store.write(pages[i], &mut page)?;
    }
    Ok(pages[0])
}

/// Read the `len` bytes stored in the chain that starts at `first`.
pub fn read_chain<S: PageStore + ?Sized>(store: &S, first: PageNo, len: usize) -> Result<Vec<u8>> {
    let cap = capacity(store.page_size());
    let mut data = Vec::with_capacity(len);
    let mut page = store.new_page();
    let mut next = Some(first);
    while data.len() < len {
        let Some(n) = next else {
            let why = format!(
                "the chain from page {first} ends after {} of {len} bytes",
                data.len()
            );
            return Err(Error::BadChain(why));
        };
        store.read(n, &mut page)?;
        if page.kind() != Some(PageKind::Overflow) {
            let found = page.bytes()[0];
            return Err(Error::WrongKind {
                page: n.get(),
                expected: PageKind::Overflow,
                found,
            });
        }
        // Every page must hold at least one byte, so a chain that loops back on
        // itself still ends after at most `len` pages.
        let used = page.u16_at(USED) as usize;
        let left = len - data.len();
        if used == 0 || used > cap || used > left {
            let why = format!(
                "page {n} says it holds {used} bytes, but it can hold 1 to {cap} and {left} are left"
            );
            return Err(Error::BadChain(why));
        }
        data.extend_from_slice(&page.bytes()[DATA..DATA + used]);
        next = PageNo::new(page.u32_at(NEXT));
    }
    if let Some(n) = next {
        let why =
            format!("the chain from page {first} holds all {len} bytes but goes on to page {n}");
        return Err(Error::BadChain(why));
    }
    Ok(data)
}

/// Frees every page of the chain that starts at `first`.
pub fn free_chain<S: PageStore + ?Sized>(store: &mut S, first: PageNo) -> Result<()> {
    let mut page = store.new_page();
    let mut next = Some(first);
    while let Some(n) = next {
        store.read(n, &mut page)?;
        // If the chain loops, it comes back to a page this loop already freed.
        // That page's kind is not free, so the loop stops here.
        if page.kind() != Some(PageKind::Overflow) {
            let found = page.bytes()[0];
            return Err(Error::WrongKind {
                page: n.get(),
                expected: PageKind::Overflow,
                found,
            });
        }
        next = PageNo::new(page.u32_at(NEXT));
        freelist::free(store, n)?;
    }
    Ok(())
}
