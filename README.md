# onyxdb

A SQL database written from scratch in Rust, standard library only. SQLite-like SQL, crash-safe storage, transactions, and many clients over its own protocol.

The references behind each layer are in [database-papers](https://github.com/PPRAMANIK62/database-papers).

## Crates

Each crate has a README with its design, its format and the decisions behind it. [ARCHITECTURE.md](ARCHITECTURE.md) shows how they fit together.

| Build | Crate | What it is | State |
|---|---|---|---|
| 1 | [`pagefile`](crates/pagefile) | a file of fixed-size, checksummed pages | in progress |
| 2 | `bufpool` | a buffer pool that keeps hot pages in memory | planned |
| 3 | `wal` | a write-ahead log and crash recovery | planned |
| 4 | `kv` | the ordered key-value trait every engine implements, with a model and a shared test suite | planned |
| 5 | `btree` | a B+tree engine | planned |
| 6 | `lsm` | a log-structured merge tree engine | planned |
| 7 | `mvcc` | transactions with snapshots, on any engine | planned |
| 8 | `sql` | parser, catalog, planner and executor | planned |
| 9 | `wire` | the client-server protocol | planned |
| 10 | `server` | sessions over TCP, many clients at once | planned |
| 11 | `client` | a client library and a REPL | planned |
| 12 | `onyxdb` | the database binary, wiring every layer together | planned |

## Build and test

```text
cargo test
cargo clippy --all-targets
cargo doc --open
```

The compiler is pinned in `rust-toolchain.toml`.

## Rules

- Standard library only. A crate depends on nothing but the crates below it.
- Dependencies point down. `sql` can use `mvcc`; `mvcc` never uses `sql`. When a lower crate needs something from above, it defines a trait and the crate above implements it.
- Each crate's README describes the code as it is now, and changes in the same commit as the code.
