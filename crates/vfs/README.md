# vfs

File I/O behind a trait. `Disk` is what the database runs on. `Mem` is what its tests run on.

```text
                 database code
                       │
            Vfs and File (traits)
              ┌────────┴────────┐
            Disk               Mem
        the real disk     in memory, for tests:
                          cut the power, crash, inject faults
```

No crate above this one uses `std::fs`. Everything goes through these traits, so any of it can be crash-tested.

## The public API

| Item | What it is | Use it when |
|---|---|---|
| `Vfs` | a file system: `open`, `exists`, `remove`, `rename`, `create_dir`, `list`, `sync_dir` | you work with files or directories |
| `File` | an open file: `read_at`, `write_at`, `len`, `set_len`, `sync`, `try_lock` | you read or write a file |
| `Open` | `Existing` or `CreateNew` | you open a file |
| `Disk` | the real file system, below a root directory | you run the database |
| `Mem` | a file system in memory that can crash | you test code that writes files |
| `Mem::changes()` | how many changes have been made so far | you want to know where to cut the power |
| `Mem::power_off_after(n)` | lets `n` more changes happen, then fails every change | you want the power to go off at a set point |
| `Mem::crash(seed)` | throws away some unsynced data, chosen by the seed | you want to see what a crash leaves |
| `Mem::fail_next(Fault)` | makes the next write or sync fail: `WriteError` writes nothing, `ShortWrite` writes half, `SyncError` loses the unsynced writes | you test how your code handles a disk error |
| `crash_test` | crashes a workload after every change, with many seeds, and returns a `Report` | you want to check a whole workload at once |
| `Report`, `Failure` | `runs` and `failures`; each failure has `changes`, `seed` and `message` | you read a crash test's result |
| `Rng` | a small seeded random generator | your tests need repeatable random input |
| `Error`, `Result` | every error, with the operation and the path | every call returns them |

Paths are relative to the root, like `data/pages.pf`. The empty path is the root itself.

## Writing a file safely

```text
write_at   the bytes are in memory      a crash can keep, lose or tear them
sync       the bytes are on disk        safe
sync_dir   the name is on disk          safe (needed after create, rename, remove)
```

```rust
use std::path::Path;
use vfs::{Disk, Open, Vfs};

let root = std::env::temp_dir().join(format!("vfs-readme-{}", std::process::id()));
std::fs::create_dir_all(&root).unwrap();
let disk = Disk::new(&root);

let file = disk.open(Path::new("notes"), Open::CreateNew).unwrap();
file.write_at(b"hello", 0).unwrap();
file.sync().unwrap();
disk.sync_dir(Path::new("")).unwrap();

std::fs::remove_dir_all(&root).unwrap();
```

Swap `Disk::new(&root)` for `Mem::new()` and the rest is the same.

If `sync` fails, don't retry it. On Linux, a failed sync throws away the writes it was saving, and the next sync succeeds anyway. Stop, and recover from what's on disk. `Mem` behaves the same way.

## Testing against crashes

```text
crash_test(seeds, workload, check) -> Report

  workload: Fn(&Mem, &mut A) -> Result<()>          does the work, and records in A what it was told is safe
  check:    Fn(&Mem, &A) -> Result<(), String>      after a crash, checks that all of that is still there

  run the workload once, and count its changes: 1, 2, 3 ... N

  for each n from 0 to N, and each seed:
    new Mem ──> power off after n ──> workload ──> crash(seed) ──> check
                                                                     │
                                          each failure keeps (n, seed)
```

`A` is any type with a `Default`, like a `bool` or a count of saves. It lets the check tell data that was promised and lost from data that was never promised.

The `kvfile` example saves a small file two ways and crash-tests both:

```text
in place: 240 crashes, 34 failed
first: power off after 6 changes, seed 5: store holds "apple=1\nbanana", which is no version at all
by rename: 320 crashes, 0 failed
```

To replay a failure, run the workload on a new `Mem` after `power_off_after(n)`, then call `crash(seed)`.

## What a crash keeps

| What | After `crash(seed)` |
|---|---|
| synced bytes and names | always kept |
| unsynced writes | each 512-byte sector is kept, lost, or torn |
| unsynced `set_len` | kept or lost |
| unsynced name changes | kept in order, up to some point |
| writes from a failed `sync` | lost |
| locks | released |
| handles opened before the crash | return `Error::Stale` |

## Decisions

- **Trait objects, not generics.**
  Why: a generic `PageFile<F: File>` would push a type parameter into every crate above.
  Cost: one indirect call per I/O. A system call costs far more.
- **Positioned I/O only, and reads are exact.**
  Why: a database reads and writes whole pages at known offsets. It never wants half a page.
  Cost: no streaming reads. Callers ask for `len` before reading a whole file.
- **Relative paths, and renames inside one directory.**
  Why: a path means the same file to `Disk` and `Mem`. Replacing a file needs only one `sync_dir`.
  Cost: files can't move between directories.
- **Our own `Error`, with the operation and the path.**
  Why: "write data/pages.pf: No space left on device" says what to fix.
  Cost: a bigger enum than `io::Error`.
- **One exclusive lock, never waiting.**
  Why: one process owns a database.
  Cost: no shared locks.
- **Seeded crashes.**
  Why: a change count and a seed bring back the same failure every time.
  Cost: the crash model has to be written down exactly.

Not supported: memory-mapped files, shared locks, Windows, and async I/O.

## Tests and examples

```text
cargo test -p vfs                       Disk and Mem, crashes, faults, crash_test
cargo run -p vfs --example kvfile       crash-test two ways of saving a file
cargo run -p vfs --example fsyncgate    what a failed sync does
cargo run -p vfs --example replace      why sync_dir matters
cargo run -p vfs --example torn         a torn sector, seed by seed
```

The references behind this crate are in [database-papers/1-vfs](https://github.com/PPRAMANIK62/database-papers/tree/main/1-vfs).
