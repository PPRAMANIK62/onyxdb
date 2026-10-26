# vfs

File I/O behind a trait, so a database can run on the real disk or on a file system in memory that loses power on command.

Every crate in onyxdb that keeps data in files goes through this one: `pagefile`, `wal` and `lsm`, and through them everything else. That gives one place to swap the real disk for a simulated one, and the simulated one can lose power at any moment, keep some unsynced writes and not others, and fail an `fsync`. Every layer above can then be tested against every crash it could meet, from a seed.

## Status

| Part | State |
|---|---|
| `Vfs` and `File` traits, `Open` | done |
| `Error`, with the operation and the path | done |
| `Disk`: the real file system, with an exclusive file lock | done |
| Behaviour tests on `Disk` | done |
| `Mem`: a file system in memory | not yet |
| `Rng`: the seeded random number generator | not yet |
| `Mem::crash`: what a power failure keeps | not yet |
| Faults: failed writes, short writes, failed syncs | not yet |
| `crash_test`: crash a workload after every change | not yet |

## Use

```rust
use std::path::Path;
use vfs::{Disk, Open, Vfs};

let root = std::env::temp_dir().join(format!("vfs-readme-{}", std::process::id()));
std::fs::create_dir_all(&root).unwrap();
let disk = Disk::new(&root);

// Create a file, write it, and make both its bytes and its name durable.
let file = disk.open(Path::new("notes"), Open::CreateNew).unwrap();
file.write_at(b"hello", 0).unwrap();
file.sync().unwrap();
disk.sync_dir(Path::new("")).unwrap();

let mut buf = [0; 5];
file.read_at(&mut buf, 0).unwrap();
assert_eq!(&buf, b"hello");

// One handle at a time holds a file's lock.
file.try_lock().unwrap();
let other = disk.open(Path::new("notes"), Open::Existing).unwrap();
assert!(matches!(other.try_lock(), Err(vfs::Error::Locked(_))));

std::fs::remove_dir_all(&root).unwrap();
```

Once `Mem` is built, a crash test looks like this:

```rust,ignore
use std::path::Path;
use vfs::{Mem, Open, Vfs, crash_test};

// Write a file, sync it and its directory, and count that as acknowledged.
let workload = |mem: &Mem, acked: &mut bool| {
    let file = mem.open(Path::new("f"), Open::CreateNew)?;
    file.write_at(b"abc", 0)?;
    file.sync()?;
    mem.sync_dir(Path::new(""))?;
    *acked = true;
    Ok(())
};

// After any crash, an acknowledged file must be there, whole.
let check = |mem: &Mem, acked: &bool| -> Result<(), String> {
    if !*acked {
        return Ok(());
    }
    let file = mem.open(Path::new("f"), Open::Existing).map_err(|e| e.to_string())?;
    let mut buf = [0; 3];
    file.read_at(&mut buf, 0).map_err(|e| e.to_string())?;
    if &buf == b"abc" { Ok(()) } else { Err(format!("f holds {buf:?}")) }
};

let report = crash_test(20, workload, check);
assert!(report.failures.is_empty(), "{report}");
```

## Modules

- `file`: the `Vfs` and `File` traits every caller uses, and `Open`.
- `disk`: `Disk`, the real file system.
- `mem`: `Mem`, a file system in memory that remembers what isn't synced yet.
- `rng`: `Rng`, the seeded generator crashes are chosen with.
- `crash`: `Mem::crash` and `Mem::power_off_after`.
- `fault`: `Fault` and `Mem::fail_next`.
- `harness`: `crash_test` and its `Report`.

## The interface

```text
Vfs   open · exists · remove · rename · create_dir · list · sync_dir
File  read_at · write_at · len · is_empty · set_len · sync · try_lock
Open  Existing | CreateNew
```

Paths are relative, like `data/pages.pf`, and mean the same file to `Disk` and `Mem`. The empty path is the root directory.

## The crash model

What `Mem::crash(seed)` keeps:

| What | Survives a crash when |
|---|---|
| File bytes | Synced bytes always survive. Each later write is cut into 512-byte sectors, and each sector, chosen from the seed, is kept, lost, or torn: new bytes from one end up to some point, old bytes after. |
| File length | A `set_len` after the last `sync` is kept or lost. |
| Names in a directory | Changes after the directory's last `sync_dir` survive as a prefix: the first *k*, in order. A rename is one change. |
| After a failed `sync` | The writes it was meant to save are gone for good, though reads still return them until the crash. |
| Locks | All released: the process that held them is gone. |
| Old handles | Every call returns `Error::Stale`. |

Linear sectors are SQLite's assumption, and in-order name changes are how ext4's journal behaves. A file system that breaks either breaks the databases on it too.

## Decisions

- **Trait objects, not generics.**
  Why: a generic `PageFile<F: File>` would push a type parameter into every crate above it.
  Cost: one indirect call per I/O, next to a system call that costs far more.
- **Positioned I/O only: `read_at` and `write_at`, no cursor.**
  Why: that's how a database uses a file, and it lets a `File` be shared through `&self`.
  Cost: no streaming reads; callers say where every read goes.
- **Reads are exact.**
  Why: a short read is a classic source of bugs, and a database never wants half a page.
  Cost: a caller that wants "as much as there is" asks for `len` first.
- **Paths are relative to a root, with no `.` or `..`.**
  Why: the same path means the same file to `Disk` and `Mem`, and a test can't touch files outside its own directory.
  Cost: a caller that has an absolute path splits it into a root and a name.
- **Renames stay inside one directory.**
  Why: replacing a file in place is the rename databases need, and it needs one directory sync.
  Cost: moving a file between directories isn't supported.
- **Our own `Error`, with the operation and the path.**
  Why: "write data/pages.pf: No space left on device" can be acted on; a bare `io::Error` names neither.
  Cost: a larger enum, and `from_io` to convert.
- **One exclusive lock, never waiting.**
  Why: onyxdb lets one process own a database. `try_lock` is `flock` on `Disk`, released when the handle closes or the process dies.
  Cost: no shared locks, and no waiting for a lock to free up.
- **Seeded, deterministic crashes.**
  Why: a failing crash test names its change count and seed, and those two numbers bring the same failure back.
  Cost: the model has to be written down exactly; it can't just be "whatever the disk does".

## Out of scope

Memory-mapped files, shared or waiting locks, Windows, and asynchronous I/O.

## Tests

```text
cargo test -p vfs
```

`tests/behaviour.rs` runs one walk through everything a file system must do against any `Vfs`: create, write past the end, refuse reads past the end, open with each mode, directories, rename over a file, remove an open file, lock against a second handle, and refuse bad paths. It runs on `Disk` today, and on `Mem` once it exists. Later tests compare random operations on both, and check the crash model, the faults and `crash_test`.

## Learn more

The design, the chapters and the sources are in [papers/1-vfs](https://github.com/PPRAMANIK62/database-papers/tree/main/1-vfs).
