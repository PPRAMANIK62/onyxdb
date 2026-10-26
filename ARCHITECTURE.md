# Architecture

How the crates fit together. Each crate's README has its own design; this file is the map between them. Only `pagefile` is being built so far. The rest is the plan, and this file changes as each crate lands.

## The layers

```text
     client             REPL and client library
        |
      wire              frames and messages, shared by both sides
        |
     server             TCP, one session per connection, login
        |
       sql              parse -> bind -> plan -> execute, within a memory budget
        |
      mvcc              snapshots, conflicts, intents, locks, old versions
        |
       kv               the ordered key-value trait
   +----+-----+------------+
 model     btree          lsm      three engines behind one trait
             |             |
          bufpool          |
             |             |
          pagefile        wal      pages; the log both engines use
             |             |
             +---- vfs ----+       every file operation
```

`onyxdb` sits on top of all of them: the binary that reads the config, opens the data directory, picks the engine and starts the server.

## Who owns what

| Crate | Owns | Depends on |
|---|---|---|
| `vfs` | the file trait, the real file system, an in-memory one that can crash, injected faults | nothing |
| `pagefile` | the page format with a checksum and an LSN on every page, the header, the freelist, slotted pages, overflow chains | `vfs` |
| `bufpool` | which pages stay in memory, its own page table, pinning, eviction, the write-ahead rule | `pagefile` |
| `wal` | log records with LSNs, `fsync`, group commit, replay, fuzzy checkpoints | `vfs` |
| `kv` | the engine trait (get, put, delete, ordered scans, atomic batches), the in-memory model engine, the test suite every engine must pass | nothing |
| `mvcc` | versioned keys, snapshots, serializable checks, intents for large transactions, the lock manager and deadlock detection, cleanup of old versions | `kv` |
| `sql` | the SQL dialect, the catalog, rows and indexes as keys, statistics, plans, operators that spill to disk | `mvcc` |
| `wire` | the byte format of every message between client and server | nothing |
| `server` | listening, sessions, login, limits, timeouts, shutdown | `sql`, `wire` |
| `client` | connecting, sending queries, reading rows, retries, the REPL | `wire` |
| `btree` | the B+tree on pages, its cell format, recovery through the log | `bufpool`, `wal`, `kv` |
| `lsm` | the memtable, sorted tables, Bloom filters, compaction, the manifest | `vfs`, `wal`, `kv` |
| `onyxdb` | the binary and the end-to-end checks | all of the above |

## The life of a query

1. The REPL reads `SELECT name FROM users WHERE id = 7;` and the client library sends it as a query message (`wire`).
2. The server reads it on that connection's session and hands the text to `sql`.
3. `sql` parses it, looks up `users` in the catalog, and picks a lookup by primary key, because `id` is the key.
4. The plan runs inside the session's transaction. `mvcc` turns "row 7 of `users`" into a key and reads the newest version the transaction's snapshot can see.
5. The read goes through the `kv` trait to the engine: a path down the B-tree through `bufpool` and `pagefile`, or a search of the LSM tree's memtable and tables.
6. Rows come back up the same way, and the server sends them as row messages and a completion.

A write takes the same path down. Small transactions buffer their writes; large ones write intents as they go. At commit, `mvcc` checks for conflicts and makes the changes visible in one atomic batch, which the engine makes durable through `wal` before the server answers.

## Crash safety

- Every page carries the LSN of the last log record that changed it.
- `bufpool` never writes a dirty page until the log is on disk up to that page's LSN. It asks through a hook trait it defines; `btree` connects the hook to `wal`, so `bufpool` never depends on `wal`.
- Recovery replays the log and skips any change whose LSN a page already has.
- Checkpoints are fuzzy: they record which pages are dirty without stopping writes.

## Seams

- `vfs`: the real disk, or an in-memory one that crashes on command.
- `pagefile::PageStore`: a page file, or a buffer pool in front of one.
- `kv`: the model, the B-tree or the LSM tree. The same SQL runs on all three.
- `wire`: any client that speaks the messages can connect.

## Checks

- Every engine runs the `kv` suite: random operations against the model, then a crash at every write through `vfs`.
- Every decoder (pages, log records, tables, messages, SQL) is fuzzed; a panic is a bug.
- `mvcc` runs the Hermitage isolation tests and a deadlock test.
- `sql` compares results with `sqlite3` row for row, including sorts and joins bigger than the memory budget.
- `onyxdb` runs a slice of SQLite's sqllogictest, survives `kill -9` under load, and serves many clients at once.
- CI runs fmt, clippy with warnings as errors, and every test on each push.
