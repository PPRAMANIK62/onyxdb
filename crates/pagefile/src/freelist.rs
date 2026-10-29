//! The freelist: freed pages, reused before the file grows.
//!
//! Free pages form a chain that starts at the header. Each free page holds the
//! number of the next one. `allocate` takes the first free page, or adds a page
//! at the end of the file if the chain is empty. `free` puts a page at the front.
//! Both work through any `PageStore`, so with a cache in front of the file,
//! every page they change goes through the cache, the header included.
//!
//! ```text
//! free page   0      1     9                13            size - 4     size
//!             +------+-----+----------------+-----------------+----------+
//!             | kind | LSN | next free page | (zeros)         | checksum |
//!             +------+-----+----------------+-----------------+----------+
//! ```

use crate::{Error, Header, PageKind, PageNo, PageStore, Result, page::link_to_u32};

/// 9: a free page, the number of the next free page, or 0 for the last one.
pub const FREE_NEXT: usize = 9;

/// Returns a page that's free to use.
// "`S` can be any type that implements `PageStore`, including types whose size
// isn't known at compile time."
// `S` is any type that implements `PageStore`. The `?Sized` lets `S` also be
// a trait object, so code that holds `&mut dyn PageStore<Error = E>` can call
// these too. Without it Rust only accepts types whose size is known at compile
// time, and a trait object isn't one of them.
pub fn allocate<S: PageStore + ?Sized>(store: &mut S) -> Result<PageNo, S::Error> {
    let header = store.read_header()?;
    match header.free_head {
        Some(head) => pop(store, header, head),
        None => append(store, header),
    }
}

/// Puts page `n` on the freelist. Don't use it again until `allocate` returns it.
pub fn free<S: PageStore + ?Sized>(store: &mut S, n: PageNo) -> Result<(), S::Error> {
    let mut header = store.read_header()?;
    let mut page = store.new_page();
    store.read(n, &mut page)?;
    if page.kind() == Some(PageKind::Free) {
        return Err(Error::DoubleFree(n.get()).into());
    }
    page.reset(PageKind::Free);
    page.put_u32(FREE_NEXT, link_to_u32(header.free_head));
    store.write(n, &mut page)?;
    header.free_head = Some(n);
    header.free_count += 1;
    store.write_header(&header)
}

/// Takes `head`, the first page on the freelist, off the list and empties it.
fn pop<S: PageStore + ?Sized>(
    store: &mut S,
    mut header: Header,
    head: PageNo,
) -> Result<PageNo, S::Error> {
    let mut page = store.new_page();
    store.read(head, &mut page)?;
    page.expect_kind(head, PageKind::Free)?;
    let next = PageNo::new(page.u32_at(FREE_NEXT));
    page.reset(PageKind::Empty);
    store.write(head, &mut page)?;
    header.free_head = next;
    // A count of 0 here is wrong. Leave it at 0 for `check` to report, instead of
    // wrapping around.
    header.free_count = header.free_count.saturating_sub(1);
    store.write_header(&header)?;
    Ok(head)
}

/// Adds an empty page at the end of the file. The header counts it first,
/// because a store only writes pages below the page count.
fn append<S: PageStore + ?Sized>(store: &mut S, mut header: Header) -> Result<PageNo, S::Error> {
    let n = header.page_count;
    header.page_count = n.checked_add(1).ok_or(Error::TooManyPages)?;
    store.write_header(&header)?;
    let n = PageNo::new(n).expect("page 0 always exists, so a new page is never page 0");
    let mut page = store.new_page();
    page.reset(PageKind::Empty);
    store.write(n, &mut page)?;
    Ok(n)
}
