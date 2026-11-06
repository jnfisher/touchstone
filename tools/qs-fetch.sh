#!/usr/bin/env bash
# Build a Quicksilver 1.20 world as SQLite, in the cache only. Prints the database path.
# Builds Quicksilver's own binary from a local checkout (QUICKSILVER_DEV, read-only: the target
# directory is under the cache) and runs its default import, which applies the 1.20 content the
# binary carries to the stock package. The stock package is reused from the silvermere work
# directory when it is there; otherwise the import downloads it (QUICKSILVER_STOCK_URL names a
# mirror). Idempotent: an existing database is kept. Nothing here is ever committed.
set -euo pipefail

QS_SRC="${QUICKSILVER_DEV:-/home/user/quicksilver-dev}"
CACHE="${TOUCHSTONE_CACHE:-$HOME/.cache/touchstone}"
DB="${QUICKSILVER_DB:-$CACHE/quicksilver-1.20.sqlite}"

if [ -s "$DB" ]; then
  echo "$DB"
  exit 0
fi

[ -f "$QS_SRC/Cargo.toml" ] || { echo "no Quicksilver checkout at $QS_SRC (set QUICKSILVER_DEV)" >&2; exit 1; }

TARGET="${CARGO_TARGET_DIR:-$CACHE/quicksilver/target}"
BIN="$TARGET/release/quicksilver"
if [ ! -x "$BIN" ]; then
  (cd "$QS_SRC" && CARGO_TARGET_DIR="$TARGET" cargo build --release --locked -p silvermere >&2)
fi

WORK="$CACHE/quicksilver/work"
ZIP=WCCMMUD_1.11p_DOS_MBBSEmu_v2.zip
mkdir -p "$WORK/data/cache"
if [ ! -s "$WORK/data/cache/$ZIP" ] && [ -s "$CACHE/silvermere/work/data/cache/$ZIP" ]; then
  # The import checks the package's size and sha256 whatever its origin.
  cp "$CACHE/silvermere/work/data/cache/$ZIP" "$WORK/data/cache/$ZIP"
fi

rm -f "$DB.part" "$DB.part-wal" "$DB.part-shm"
(cd "$WORK" && "$BIN" import --database-url "sqlite://$DB.part?mode=rwc" >&2) \
  || { rm -f "$DB.part"*; exit 1; }
rm -f "$DB.part-wal" "$DB.part-shm" "$DB.part.lock"
mv "$DB.part" "$DB"
echo "$DB"
