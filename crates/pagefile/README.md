# pagefile

A file of fixed-size, numbered pages.

The file is cut into pages of one size. Page 0 is a header that says what the file is. Every other page holds one kind of thing, named by its first byte, followed by the page's log sequence number (LSN). Every page ends with a checksum, so a damaged page is caught when it's read. All file I/O goes through [`vfs`](../vfs), so the tests run on a file system in memory.

This is the bottom layer of the database. Everything stored on disk in pages goes through it.

## Status

| Part | State |
|---|---|
| Pages, page numbers, page kinds and the page LSN | done |
| The header in page 0 | done |
| CRC-32C checksum on every page | done |
| Create, open, read and write pages through `vfs` | done |
| Tests for damaged, torn and foreign files | done |
| Slotted pages | done |
| Allocating and freeing pages (the freelist) | done |
| Overflow chains | not yet |
| Model test of allocate and free | done |
| A whole-file check and a fuzz test | not yet |

## Use

```rust
use pagefile::{PageFile, PageStore};
use vfs::Mem;

let mem = Mem::new();

let mut file = PageFile::create(&mem, "db.pf", 4096).unwrap();
file.set_user([7; 32]).unwrap();
drop(file);

let file = PageFile::open(&mem, "db.pf").unwrap();
assert_eq!(file.page_size(), 4096);
assert_eq!(file.page_count(), 1);
assert_eq!(file.header().user, [7; 32]);
```

## Modules

- `page`: page numbers, page kinds and the bytes of one page.
- `header`: the header in page 0.
- `crc`: the CRC-32C checksum.
- `store`: the `PageStore` trait for reading and writing pages and the header.
- `file`: `PageFile`, a page file opened through a `vfs::Vfs`, that implements `PageStore`.
- `slotted`: variable-size items with insertion, removal, replacement and compaction.
- `freelist`: `allocate` and `free`, which reuse freed pages before the file grows.

## Format

Every number is little-endian.

```text
every page except page 0
byte 0      1          9                             size - 4    size
     +------+----------+-----------------------------+-----------+
     | kind | LSN      | laid out by the page's kind | CRC-32C   |
     +------+----------+-----------------------------+-----------+

page 0, the header (no kind byte)
offset  size  field
     0     8  magic: the ASCII bytes "pagefile"
     8     2  format version, 1
    10     4  page size, in bytes
    14     4  page count, page 0 included
    18     4  first page on the freelist, 0 if the freelist is empty
    22     4  number of pages on the freelist
    26    32  user bytes, for the code using this crate
    58     .  zeros
size-4     4  CRC-32C of everything before it

a free page
byte 0      1          9                  13          size - 4    size
     +------+----------+------------------+-----------+-----------+
     | 1    | LSN      | next free page   | zeros     | CRC-32C   |
     +------+----------+------------------+-----------+-----------+
     next free page is 0 on the last page of the list

page kinds
  0  empty     allocated, never written
  1  free      on the freelist
  2  slotted   variable-size items packed into the page
  3  overflow  one piece of a value too big for a page
```

## Decisions

- **The page size is fixed when the file is created: a power of two from 512 to 32768.**
  Why: offsets inside a page are `u16`, which can't hold 65536. PostgreSQL makes the same cut.
  Cost: every page in a file has the same size, and 64 KiB pages aren't possible.
- **Page numbers are never 0 (`PageNo(NonZeroU32)`).**
  Why: page 0 is the header and nothing links to it, so 0 on disk can mean "no page", and `Option<PageNo>` stays 4 bytes.
  Cost: none so far.
- **Little-endian numbers.**
  Why: x86 and ARM are little-endian, so encoding is a plain byte copy.
  Cost: hex dumps read backwards. SQLite chose big-endian for the opposite reason.
- **A CRC-32C in the last 4 bytes of every page, the header included.**
  Why: it catches flipped bits and torn writes on every read. CRC-32C is the checksum ext4 and InnoDB use, and many CPUs have an instruction for it.
  Cost: 4 bytes per page. It can't catch a lost write, because the old page still has a valid checksum.
- **An 8-byte LSN after the kind byte of every page except page 0.**
  Why: the log above this crate needs to know which change each page last saw. This crate stores the number and never reads it.
  Cost: 8 bytes per page.
- **The checksum is a 256-entry table built at compile time, not the CPU instruction.**
  Why: std only, and one portable version that's easy to check.
  Cost: slower than the hardware instruction.
- **The header keeps 32 bytes for the code using the crate.**
  Why: the layer above needs somewhere to remember where its data starts, such as a root page, without a format change here.
  Cost: 32 bytes the crate never reads.
- **`sync` is a separate call, never automatic.**
  Why: deciding when data must be on disk belongs to the layer above, which knows what a commit is.
  Cost: a caller that forgets `sync` can lose writes on a crash.
- **All file I/O goes through `vfs`.** `create` and `open` take a `&dyn Vfs`, and `PageFile` holds a `Box<dyn vfs::File>`.
  Why: the tests run on `vfs::Mem`, which can tear a write or cut the power, so damage and crash cases are easy to test.
  Cost: one more crate below this one, and a `&dyn Vfs` in every call that opens a file.
- **`create` syncs the file and its directory before it returns.**
  Why: without the directory sync, a crash right after `create` can lose the file's name.
  Cost: two `fsync` calls on every create.
- **Reads and writes use positioned I/O (`read_at`, `write_at`).**
  Why: no seek-then-read, so a read needs only `&self`.
  Cost: none so far.
- **`PageStore` is a trait, and `PageFile` is one implementation.**
  Why: a page cache can implement the same trait and stand in for the file.
  Cost: one trait to keep stable.
- **`allocate` and `free` are functions over any `PageStore`, not methods of `PageFile`.**
  Why: with a page cache in front of the file, the pages and the header they change go through the cache too.
  Cost: each call reads the header through the trait.
- **`free` puts a page at the front of the list, so the list works as a stack.**
  Why: freeing a page writes only that page and the header. Adding at the back means walking the list or keeping a tail page to rewrite.
  Cost: the list falls out of page order, so pages allocated together can sit far apart in the file.
- **Taking a page off the list and freeing one write the page before the header.**
  Why: a crash between the two writes leaves a page that's off the list (`free`) or a header that points at a page that isn't free (`allocate`). The page is lost or the next `allocate` refuses it, but it's never handed out twice.
  Growing the file goes the other way: the header first, because a store only writes pages below the page count.
  Cost: a crash can leak a page until a check finds it.
- **`free` refuses a page that's already free, and `allocate` refuses a list page that isn't.**
  Why: a page freed twice would point at itself and be handed out forever.
  Cost: one kind check per call. A page leaked by a crash can't be freed again.
- **Errors name the rule that broke and carry the numbers involved.**
  Why: "page 3 is damaged: its checksum is 1a2b3c4d, but its bytes give 5e6f7a8b" can be acted on.
  Cost: a larger error enum.

## Known gaps

- Changes to several pages aren't atomic. A crash between two writes leaves some old pages and some new ones.
- A lost write goes unnoticed, as described under the checksum decision.
- Every read goes to the disk. Nothing is cached.
- A crash between the two writes of `allocate` or `free` leaks a page, and nothing finds it yet.
- A crash in `allocate` after the header grows but before the new page is written leaves a file that's too short, and `open` refuses it.
- The file never shrinks. Freed pages are reused, but never given back.

## Tests

```text
cargo test -p pagefile
```

The unit tests check page sizes, page numbers and the CRC check value. `tests/corrupt.rs` runs on `vfs::Mem`. It refuses foreign and cut-short files, catches a torn header, and catches every single-bit flip in a page.

`tests/model.rs` runs 20 seeds of 400 random steps: allocate and write, free, close and reopen, and read. A `BTreeMap` of pages in use and a `BTreeSet` of free pages must match the file at every read and at the end. It also checks that freed pages come back before the file grows, and that freeing a page twice is an error.

`tests/slotted.rs` checks 100,000 random operations against a `Vec` model. It also checks compaction after removals and the largest item a page can hold.

## Learn more

The design, the stages and the sources are in [papers/2-pagefile](https://github.com/PPRAMANIK62/database-papers/tree/main/2-pagefile).
