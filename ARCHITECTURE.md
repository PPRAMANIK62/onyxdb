# Architecture

How the crates fit together. Each crate's README has its own design. This file is the map between them. Only `pagefile` is being built so far; everything else here is the plan, and this file changes as each crate lands.

## The layers

```text
           client           REPL and client library
              |
            wire            frames and messages, shared by both sides
              |
           server           TCP, one session per connection, login
              |
             sql            parse -> bind to the catalog -> plan -> execute
              |
            mvcc            transactions: snapshots, conflicts, old versions
              |
             kv             the ordered key-value trait
         +----+----+
       btree      lsm       two engines behind the same trait
         |         |
      bufpool      |
         |         |
      pagefile    wal       pages on disk; the log both engines use
```

`onyxdb` sits on top of all of them. It is the binary that reads the config, opens the data directory, picks the engine and starts the server.

## Who owns what

| Crate | Owns | Depends on |
|---|---|---|
| `pagefile` | the page format, the header, checksums, the freelist, slotted pages, overflow chains | nothing |
| `bufpool` | which pages stay in memory, pinning, eviction, writing dirty pages back | `pagefile` |
| `wal` | log records, `fsync`, group commit, replay after a crash, checkpoints | nothing |
| `kv` | the trait every engine implements: get, put, delete, ordered scans, atomic batches; an in-memory model; the test suite every engine must pass | nothing |
| `btree` | the B+tree on pages, its cell format, crash safety through the log | `bufpool`, `wal`, `kv` |
| `lsm` | the memtable, sorted tables, Bloom filters, compaction, the manifest | `wal`, `kv` |
| `mvcc` | versioned keys, snapshots, write conflicts, serializable checks, cleanup of old versions | `kv` |
| `sql` | the SQL dialect, the catalog of tables and indexes, how rows and index entries become keys, plans, operators | `mvcc` |
| `wire` | the byte format of every message between client and server | nothing |
| `server` | listening, sessions, one transaction per session at a time, login, shutdown | `sql`, `wire` |
| `client` | connecting, sending queries, reading rows, the REPL | `wire` |
| `onyxdb` | the binary and the end-to-end checks | all of the above |

## The life of a query

1. The REPL reads `SELECT name FROM users WHERE id = 7;` and the client library sends it as a query message (`wire`).
2. The server reads the message on that connection's session and hands the text to `sql`.
3. `sql` parses it, looks up `users` in the catalog, and picks a plan: a lookup by primary key, because `id` is the key.
4. The plan runs inside the session's transaction. `mvcc` turns "row 7 of `users`" into a key, and reads the newest version the transaction's snapshot can see.
5. The read goes through the `kv` trait to the engine: a path down the B-tree through `bufpool` and `pagefile`, or a search of the LSM tree's memtable and tables.
6. Rows go back up the same way, and the server sends them as row messages followed by a completion message.

A write takes the same path down. At commit, `mvcc` checks for conflicts and writes the transaction's changes as one atomic batch, which the engine makes durable through `wal` before the server answers.

## Seams

The places where one layer can be swapped without the others noticing:

- `pagefile::PageStore`: a page file on disk, or a buffer pool in front of one.
- the `kv` trait: the B-tree or the LSM tree. The same SQL runs on either, which makes comparing them a matter of changing one setting.
- the `wire` messages: any client that speaks them can connect.

## Checks

- Every engine runs the `kv` test suite: random operations compared with an in-memory model, then crashes at every write.
- `mvcc` runs the Hermitage isolation tests.
- `sql` compares query results with `sqlite3` row for row.
- `onyxdb` runs a slice of SQLite's sqllogictest, survives `kill -9` mid-write, and serves many clients at once.
