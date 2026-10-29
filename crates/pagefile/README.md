# pagefile

A file of fixed-size, numbered pages.

```text
 page 0     page 1     page 2     page 3     page 4
┌────────┬──────────┬──────────┬──────────┬──────────┐
│ header │ slotted  │ overflow │ overflow │ free     │ ...
└────────┴──────────┴──────────┴──────────┴──────────┘
  what the   small      one value too big      reused before
  file is    items      for a page            the file grows
```

Page 0 is a header that says what the file is. Every other page starts with a byte that names its kind, then the page's log sequence number (LSN). Every page ends with a checksum, so a damaged page is caught when it's read. All file I/O goes through [`vfs`](../vfs), so the tests run on a file system in memory that can crash.

## The public API

| Item | What it is | Use it when |
|---|---|---|
| `PageFile` | a page file: `create`, `open`, `header`, `page_count`, `set_user`, `sync` | you open or create a file |
| `PageStore` | a trait: `page_size`, `read`, `write`, `read_header`, `write_header`, `new_page` | you read or write pages, through a file or anything that stands in for one |
| `Page` | the bytes of one page: `kind`, `lsn`, `set_lsn`, and number readers and writers | you look at or change a page's bytes |
| `PageNo`, `PageKind` | a page number, never 0, and a page's kind | you name a page or check what it holds |
| `Header` | the fields of page 0, with 32 bytes for your own use | you need the page size, the page count or the user bytes |
| `Slotted` | many variable-size items in one page: `insert`, `get`, `remove`, `replace`, `compact` | you store small items in order |
| `freelist::allocate`, `freelist::free` | get a page, reusing a freed one first, and give one back | you need a new page or are done with one |
| `overflow::write_chain`, `read_chain`, `free_chain` | store, read and free a value across a chain of pages | a value is bigger than `slotted::max_item` |
| `check::check` | reads every page and walks the freelist, and returns a `Report` of problems | you want to know if a file is healthy |
| `crc::crc32c` | the CRC-32C checksum | you checksum bytes the same way pages are |
| `Error`, `Result` | every error, with the rule that broke and the numbers involved | every call returns them |

## Use

```rust
use pagefile::{PageFile, PageStore, Slotted, freelist, overflow};
use vfs::Mem;

let mem = Mem::new();
let mut file = PageFile::create(&mem, "db.pf", 4096).unwrap();

// A page of small items, kept in the order we insert them.
let n = freelist::allocate(&mut file).unwrap();
let mut page = file.new_page();
let mut items = Slotted::init(&mut page, 0);
items.insert(0, b"banana").unwrap();
items.insert(0, b"apple").unwrap();
file.write(n, &mut page).unwrap();

// A value too big for one page goes in a chain of overflow pages.
let big = vec![7u8; 10_000];
let first = overflow::write_chain(&mut file, &big).unwrap();
file.sync().unwrap();
drop(file);

let file = PageFile::open(&mem, "db.pf").unwrap();
let mut page = file.new_page();
file.read(n, &mut page).unwrap();
assert_eq!(Slotted::open(&page).unwrap().get(0), Some(&b"apple"[..]));
assert_eq!(overflow::read_chain(&file, first, big.len()).unwrap(), big);
```

`write` seals the page with its checksum. `read` checks it, so a damaged page comes back as an error, never as bytes.

## Format

Every number is little-endian.

```text
every page except page 0
byte 0      1          9                             size - 4    size
     +------+----------+-----------------------------+-----------+
     | kind | LSN      | laid out by the page's kind | CRC-32C   |
     +------+----------+-----------------------------+-----------+

page kinds
  0  empty     allocated, never written
  1  free      on the freelist
  2  slotted   variable-size items packed into the page
  3  overflow  one piece of a value too big for a page
```

**Page 0, the header** (no kind byte):

```text
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
```

**A slotted page.** Slots grow from the front, items from the back, and the free space is the gap between them. Each slot is an offset and a length, two `u16`s.

```text
0      1     9     10      12              14        16
+------+-----+-----+-------+---------------+---------+--------+-----+--------+----------+------+-----+----------+
| kind | LSN | tag | count | content start | garbage | slot 0 | ... | slot n | free gap | item | ... | CRC-32C  |
+------+-----+-----+-------+---------------+---------+--------+-----+--------+----------+------+-----+----------+
                                                                                ^ content start
```

**An overflow page.** The chain doesn't store the value's length. Whoever points at the first page keeps it.

```text
0      1     9                 13           15                   size - 4    size
+------+-----+-----------------+------------+--------------------+-----------+
| 3    | LSN | next page, or 0 | bytes used | data               | CRC-32C   |
+------+-----+-----------------+------------+--------------------+-----------+
```

**A free page.**

```text
0      1     9                      13                       size - 4    size
+------+-----+----------------------+------------------------+-----------+
| 1    | LSN | next free page, or 0 | zeros                  | CRC-32C   |
+------+-----+----------------------+------------------------+-----------+
```

With 512-byte pages, the largest slotted item is 488 bytes and an overflow page holds 493.

## Look inside a file

Make the sample files first. They go in `crates/pagefile/samples/`, which git ignores:

```text
crates/pagefile/scripts/samples.sh
```

Build the `pagefile` command and put it on your `PATH`, from the workspace root:

```text
cargo build --release -p pagefile
export PATH="$PWD/target/release:$PATH"
```

Then look at the samples with it:

```text
cd crates/pagefile/samples
pagefile header long.pf
pagefile pages  long.pf
pagefile page   fruit.pf 1
pagefile check  leaked.pf
```

```text
$ pagefile pages long.pf
    0  header
    1  slotted   1 item, 480 bytes of room
    2  overflow  next 3, 493 bytes
    3  overflow  next 4, 493 bytes
    4  overflow  next 0, 214 bytes

$ pagefile check leaked.pf
pages: 2 free, 1 header, 3 slotted
problem: page 4 is free but not on the freelist
```

`check` exits with 1 when it finds a problem. Items that are all printable text print in quotes, and anything else prints as hex.

| Sample | What's in it |
|---|---|
| `empty.pf` | only the header |
| `fruit.pf` | one slotted page holding apple, banana and cherry |
| `holes.pf` | `fruit.pf` with banana removed, which leaves a gap |
| `packed.pf` | `holes.pf` after `compact` closes the gap |
| `free.pf` | five pages, with pages 2 and 4 freed |
| `long.pf` | a 1200-byte value in a chain of three overflow pages, and a slotted page holding its first page and length |
| `flipped.pf` | `fruit.pf` with one bit flipped, so its checksum fails |
| `leaked.pf` | a free page that isn't on the freelist |
| `fruit.db` | the same three fruits stored by SQLite, if `sqlite3` is installed |

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
- **The freelist and overflow chains are functions over any `PageStore`, not methods of `PageFile`.**
  Why: with a page cache in front of the file, the pages and the header they change go through the cache too.
  Cost: each call reads the header through the trait.
- **Slot order is the items' order, and inserting or removing shifts the slots after it.**
  Why: the code using the page keeps its items sorted by moving 4-byte slots, never the item bytes.
  Cost: a slot number isn't a stable name for an item, so nothing outside the page can hold on to one. PostgreSQL keeps slot numbers fixed instead, because its indexes point at rows by (page, slot).
- **A removed item's bytes stay as garbage until `compact`.**
  Why: removing only touches the slots.
  Cost: `insert` compacts the page when the gap is too small but the garbage would make room.
- **`free` puts a page at the front of the list, so the list works as a stack.**
  Why: freeing a page writes only that page and the header. Adding at the back means walking the list or keeping a tail page to rewrite.
  Cost: the list falls out of page order, so pages allocated together can sit far apart in the file.
- **Taking a page off the list and freeing one write the page before the header.**
  Why: a crash between the two writes leaves a page that's off the list (`free`) or a header that points at a page that isn't free (`allocate`). The page is lost or the next `allocate` refuses it, but it's never handed out twice.
  Growing the file goes the other way: the header first, because a store only writes pages below the page count.
  Cost: a crash can leak a page. `check` finds it.
- **`free` refuses a page that's already free, and `allocate` refuses a list page that isn't.**
  Why: a page freed twice would point at itself and be handed out forever.
  Cost: one kind check per call. A page leaked by a crash can't be freed again.
- **An overflow chain doesn't store the value's length. The caller keeps it.**
  Why: the caller already stores a pointer to the first page, and 4 more bytes there let `read_chain` catch a chain that ends too early or runs too long.
  Cost: a chain can't be read without its length, and `check` can't check chains.
- **Every overflow page holds at least 1 byte.**
  Why: a chain that loops back on itself still ends after at most `len` pages, so `read_chain` never runs forever on a damaged file.
  Cost: none.
- **`write_chain` allocates every page of the chain before it writes any.**
  Why: each page's next page is known when it's written, so every page is written once.
  Cost: a crash in between leaves allocated pages that hold nothing.
- **`check` reports every problem it finds, not just the first.**
  Why: one run shows all the damage. Only an I/O error stops it.
  Cost: a `Report` to read instead of a single error.
- **Errors name the rule that broke and carry the numbers involved.**
  Why: "page 3 is damaged: its checksum is 1a2b3c4d, but its bytes give 5e6f7a8b" can be acted on.
  Cost: a larger error enum.

## Known gaps

- Changes to several pages aren't atomic. A crash between two writes leaves some old pages and some new ones, and a crash can tear the header, which `open` then refuses. The crash example shows how often.
- A lost write goes unnoticed, as described under the checksum decision.
- Every read goes to the disk. Nothing is cached.
- `check` finds a leaked free page, but nothing gives it back. It can't find a leaked slotted or overflow page, because only the code using the crate knows which pages it still points to.
- The file never shrinks. Freed pages are reused, but never given back.

## Tests and examples

```text
cargo test -p pagefile
cargo run --release -p pagefile --example crashes
```

| Test | What it checks |
|---|---|
| unit tests | page sizes, page numbers and the CRC check value |
| `tests/corrupt.rs` | refuses foreign and cut-short files, catches a torn header, and catches every single-bit flip in a page |
| `tests/slotted.rs` | 50 seeds of 2000 random operations against a `Vec`, compaction after removals, and the largest item |
| `tests/model.rs` | 20 seeds of 400 random allocates, frees, reopens and reads against a `BTreeMap` and a `BTreeSet`, then `check` finds nothing wrong. Freed pages come back before the file grows, and freeing twice is an error |
| `tests/overflow.rs` | values come back unchanged, freeing a chain returns every page, and a wrong length is caught |
| `tests/fuzz.rs` | damaged slotted pages, headers and whole files. Every reader returns an error and never panics |

The crash example creates a file, writes three pages, frees one and syncs, then cuts the power after every change with 20 seeds:

```text
it opens:            340 crashes, 179 failed
it opens and checks: 340 crashes, 204 failed
```

The failures are expected. Each change writes two pages, a crash can keep either one, and nothing here makes them change together. That's the write-ahead log's job, so the same workload should pass once a log sits underneath.

The references behind this crate are in [database-papers/2-pagefile](https://github.com/PPRAMANIK62/database-papers/tree/main/2-pagefile).
