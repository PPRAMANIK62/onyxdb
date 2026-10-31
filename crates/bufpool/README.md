# bufpool

A buffer pool: a fixed number of page-sized frames in memory, in front of a [`pagefile`](../pagefile).

```text
fetch(n) ──▶ page table ──▶ frame ──▶ page n, pinned and latched
                 │ miss
                 ▼
     an empty frame, or a victim the policy picks
     (a changed victim is written back first, after the log)
```

So far it has the page table. The frames, the write-back and the eviction policy come next.

## The public API

| Item | What it is | Use it when |
|---|---|---|
| `PageTable<V>` | a fixed-size hash table from `PageNo` to `V`: `new`, `get`, `insert`, `remove`, `iter`, `len` | you need to find which frame holds a page |

## Use

```rust
use bufpool::PageTable;
use pagefile::PageNo;

let mut table = PageTable::new(4);
let page = PageNo::new(7).unwrap();
table.insert(page, 2);
assert_eq!(table.get(page), Some(2));
assert_eq!(table.remove(page), Some(2));
assert_eq!(table.get(page), None);
```

## How the page table works

```text
 capacity 4 -> 8 slots
 ┌───┬───┬───┬───┬───┬───┬───┬───┐
 │   │ 7 │ 3 │ 9 │   │   │ 5 │   │
 └───┴───┴───┴───┴───┴───┴───┴───┘
       └─ a run: a search walks forward until an empty slot
```

A page's home slot comes from Fibonacci hashing: multiply by 2^64 / golden ratio and keep the top bits. If the home slot is taken, the entry goes in the next free slot.

## Decisions

- **Open addressing with linear probing, in a flat array.**
  Why: one lookup reads neighboring slots, with no pointers to follow and no allocation after `new`.
  Cost: removing an entry has to move later entries of its run back, or a search would stop early at the hole.
- **Twice as many slots as entries, rounded up to a power of two.**
  Why: the table is never more than half full, so runs stay short, and wrapping around is a bit mask instead of a division.
  Cost: half the slots are always empty.
- **A fixed size, set at `new`.**
  Why: a buffer pool has a fixed number of frames, so the table never needs to grow.
  Cost: inserting more than `capacity` entries panics.

The references behind this crate are in [database-papers/3-bufpool](https://github.com/PPRAMANIK62/database-papers/tree/main/3-bufpool).
