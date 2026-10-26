# pagefile

A file of fixed-size, numbered pages.

The file is cut into pages of one size. Page 0 is a header that says what the file is. Every other page holds one kind of thing, named by its first byte. Every page ends with a checksum, so a damaged page is caught when it's read.

This is the bottom layer of the database. Everything stored on disk in pages goes through it.

## Status

| Part | State |
|---|---|
| Pages, page numbers and page kinds | done |
| The header in page 0 | done |
| CRC-32C checksum on every page | done |
| Create, open, read and write pages | done |
| Tests for damaged and foreign files | done |
| Allocating and freeing pages (the freelist) | not yet: `allocate` and `free` are `todo!()` |
| Slotted pages | not yet |
| Overflow chains | not yet |

## Use

```rust
use pagefile::{PageFile, PageStore};

let path = std::env::temp_dir().join(format!("pagefile-readme-{}", std::process::id()));
let _ = std::fs::remove_file(&path);

let mut file = PageFile::create(&path, 4096).unwrap();
file.set_user([7; 32]).unwrap();
drop(file);

let file = PageFile::open(&path).unwrap();
assert_eq!(file.page_size(), 4096);
assert_eq!(file.page_count(), 1);
assert_eq!(file.header().user, [7; 32]);

std::fs::remove_file(&path).unwrap();
```

## Modules

- `page`: page numbers, page kinds and the bytes of one page.
- `header`: the header in page 0.
- `crc`: the CRC-32C checksum.
- `store`: the `PageStore` trait for reading, writing, allocating and freeing pages.
- `file`: `PageFile`, a page file on disk that implements `PageStore`.

## Format

Every number is little-endian.

```text
every page
byte 0      1                                   size - 4       size
     +------+---------------------------------------+-----------+
     | kind |      laid out by the page's kind      |  CRC-32C  |
     +------+---------------------------------------+-----------+

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
- **The checksum is a 256-entry table built at compile time, not the CPU instruction.**
  Why: std only, and one portable version that's easy to check.
  Cost: slower than the hardware instruction.
- **The header keeps 32 bytes for the code using the crate.**
  Why: the layer above needs somewhere to remember where its data starts, such as a root page, without a format change here.
  Cost: 32 bytes the crate never reads.
- **`sync` is a separate call, never automatic.**
  Why: deciding when data must be on disk belongs to the layer above, which knows what a commit is.
  Cost: a caller that forgets `sync` can lose writes on a crash.
- **Reads and writes use positioned I/O (`read_exact_at`, `write_all_at`).**
  Why: no seek-then-read, so a read needs only `&self`.
  Cost: Unix only, through `std::os::unix::fs::FileExt`.
- **`PageStore` is a trait, and `PageFile` is one implementation.**
  Why: a page cache can implement the same trait and stand in for the file.
  Cost: one trait to keep stable.
- **Errors name the rule that broke and carry the numbers involved.**
  Why: "page 3 is damaged: its checksum is 1a2b3c4d, but its bytes give 5e6f7a8b" can be acted on.
  Cost: a larger error enum.

## Known gaps

- Changes to several pages aren't atomic. A crash between two writes leaves some old pages and some new ones.
- A lost write goes unnoticed, as described under the checksum decision.
- Every read goes to the disk. Nothing is cached.

## Tests

```text
cargo test -p pagefile
```

The unit tests check page sizes, page numbers and the CRC check value. `tests/corrupt.rs` refuses foreign and cut-short files, catches a torn write, and catches every single-bit flip in a page.

## Learn more

The design, the stages and the sources are in [papers/2-pagefile](https://github.com/PPRAMANIK62/database-papers/tree/main/2-pagefile).
