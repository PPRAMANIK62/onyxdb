# onyxdb

A SQL database written from scratch in Rust, standard library only. SQLite-like SQL, crash-safe storage, transactions, and many clients over its own protocol.

The references behind each layer are in [database-papers](https://github.com/PPRAMANIK62/database-papers).

## Crates

Each crate has a README with its design, its format and the decisions behind it. [ARCHITECTURE.md](ARCHITECTURE.md) shows how they fit together.

| Build | Crate | What it is | State |
|---|---|---|---|
| 1 | [`vfs`](crates/vfs) | file I/O behind a trait: a real disk, and one in memory that crashes, tears writes and fails `fsync`; file locks | in progress |
| 2 | [`pagefile`](crates/pagefile) | a file of fixed-size, checksummed pages | in progress |
| 3 | `bufpool` | a buffer pool: frames, eviction, the write-ahead rule, latches | planned |
| 4 | `wal` | a write-ahead log: group commit, replay, checkpoints | planned |
| 5 | `kv` | the ordered key-value trait, an in-memory model engine and a shared test suite | planned |
| 6 | `mvcc` | transactions: snapshots, a lock manager, deadlocks, serializable isolation with phantoms | planned |
| 7 | `sql` | parser, catalog, schema locks, prepared statements, planner and executor | planned |
| 8 | `wire` | the client-server protocol | planned |
| 9 | `server` | sessions over TCP, many clients at once | planned |
| 10 | `client` | a client library and a REPL | planned |
| 11 | `btree` | a B+tree engine, crash-safe through page images in the log | planned |
| 12 | `lsm` | a log-structured merge tree engine | planned |
| 13 | `onyxdb` | the binary: owns the data directory, recovers, serves; integrity check and backup | planned |

By build 10 the whole stack runs on the in-memory model engine. Builds 11 and 12 put real engines underneath.

## Build and test

```text
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo doc --open
```

The compiler is pinned in `rust-toolchain.toml`. CI runs the same checks on every push.

## Rules

- Standard library only. A crate depends on nothing but the crates below it.
- No `unsafe`, and all file I/O goes through `vfs`.
- Dependencies point down. `sql` can use `mvcc`; `mvcc` never uses `sql`. When a lower crate needs something from above, it defines a trait and the crate above implements it.
- Each crate's README describes the code as it is now, and changes in the same commit as the code.
