# bufpool

A buffer pool: a fixed number of page-sized frames in memory, in front of a [`pagefile`](../pagefile).

```text
fetch(n) ──▶ page table ──▶ frame ──▶ page n, pinned and latched
                 │ miss
                 ▼
     an empty frame, or a victim the policy picks
     (a changed victim is written back first, after the log)
```

Nothing is built yet. This README grows with the code, one stage at a time.

The references behind this crate are in [database-papers/3-bufpool](https://github.com/PPRAMANIK62/database-papers/tree/main/3-bufpool).
