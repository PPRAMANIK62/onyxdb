#!/usr/bin/env bash
# Writes the sample page files into crates/pagefile/samples/, for looking at
# with the pagefile command. Run it from anywhere:
#
#   crates/pagefile/scripts/samples.sh
#
# Needs cargo. Also writes fruit.db, the same three items stored by SQLite,
# when sqlite3 is installed.
set -euo pipefail

CRATE="$(cd "$(dirname "$0")/.." && pwd)"
SAMPLES="$CRATE/samples"
mkdir -p "$SAMPLES"
rm -f "$SAMPLES"/*

cargo run --quiet --manifest-path "$CRATE/Cargo.toml" --example samples -- "$SAMPLES"

if command -v sqlite3 >/dev/null; then
  sqlite3 "$SAMPLES/fruit.db" "PRAGMA page_size = 512;
                  CREATE TABLE fruit(name TEXT PRIMARY KEY) WITHOUT ROWID;
                  INSERT INTO fruit VALUES ('banana');
                  INSERT INTO fruit VALUES ('apple');
                  INSERT INTO fruit VALUES ('cherry');"
fi

ls "$SAMPLES"
