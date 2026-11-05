#!/usr/bin/env bash
# Build public silvermere at a pinned commit (cache only, never the repository) and import its
# stock game into a SQLite database. Prints the database path. Idempotent via marker files.
# The first import downloads the stock game from MBBSEmu (see silvermere's docs/import.md);
# SILVERMERE_STOCK_URL names a mirror, SILVERMERE_SRC a local clone to use as a fast path.
set -euo pipefail

SM_SHA="${SM_SHA:-672eedb14685b5e9df8cbdcc4d65fbaf5cb02afd}"
CACHE="${TOUCHSTONE_CACHE:-$HOME/.cache/touchstone}"
DIR="$CACHE/silvermere/$SM_SHA"
DB="${SILVERMERE_STOCK_DB:-$CACHE/silvermere-stock.sqlite}"
BIN="$DIR/target/release/silvermere"

if [ ! -f "$DIR/.touchstone-fetched" ]; then
  mkdir -p "$CACHE/silvermere"
  TMP="$(mktemp -d "$CACHE/silvermere/.tmp.XXXXXX")"
  trap 'rm -rf "$TMP"' EXIT
  SRC="${SILVERMERE_SRC:-}"
  if [ -n "$SRC" ] && [ "$(git -C "$SRC" rev-parse HEAD 2>/dev/null)" = "$SM_SHA" ]; then
    git clone -q "$SRC" "$TMP/out"
  else
    mkdir "$TMP/out"
    git -C "$TMP/out" init -q
    ok=0
    for delay in 0 2 4 8 16; do
      [ "$delay" -gt 0 ] && sleep "$delay"
      if git -C "$TMP/out" fetch -q --depth 1 https://github.com/jnfisher/silvermere.git "$SM_SHA"; then ok=1; break; fi
    done
    [ "$ok" = 1 ] || { echo "could not fetch silvermere $SM_SHA" >&2; exit 1; }
  fi
  git -C "$TMP/out" checkout -q "$SM_SHA"
  touch "$TMP/out/.touchstone-fetched"
  rm -rf "$DIR"
  mv "$TMP/out" "$DIR"
  trap - EXIT
  rm -rf "$TMP"
fi

if [ ! -x "$BIN" ]; then
  (cd "$DIR" && cargo build --release -p silvermere >&2) || (cd "$DIR" && cargo build --release >&2)
fi

if [ ! -s "$DB" ]; then
  # A fixed work dir keeps the downloaded game zip (silvermere caches it under ./data/cache).
  WORK="$CACHE/silvermere/work"
  mkdir -p "$WORK"
  rm -f "$DB.part"
  (cd "$WORK" && "$BIN" import --database-url "sqlite://$DB.part?mode=rwc" >&2) \
    || { rm -f "$DB.part"*; exit 1; }
  rm -f "$DB.part-wal" "$DB.part-shm" "$DB.part.lock"
  mv "$DB.part" "$DB"
fi
echo "$DB"
