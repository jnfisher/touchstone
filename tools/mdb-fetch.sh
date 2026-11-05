#!/usr/bin/env bash
# Take the stock .mdb from the MME cache, verify its sha256, convert to SQLite via mdbtools.
# Prints the SQLite path.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CACHE="${TOUCHSTONE_CACHE:-$HOME/.cache/touchstone}"
WANT="eba20f06af8dc08df755eb6dcd9763b023794f12e809cbab6762236d64d7345f"
OUT="$CACHE/mdb/$WANT.sqlite"

if [ -f "$OUT" ]; then
  echo "$OUT"
  exit 0
fi

for t in mdb-tables mdb-schema mdb-export; do
  command -v "$t" >/dev/null 2>&1 || { echo "mdbtools is missing: apt install mdbtools" >&2; exit 1; }
done

MME_DIR="$("$HERE/mme-fetch.sh")"
MDB="$MME_DIR/data-v1.11p.mdb"
GOT="$(sha256sum "$MDB" | cut -d' ' -f1)"
if [ "$GOT" != "$WANT" ]; then
  echo "sha256 mismatch for $MDB: got $GOT, want $WANT" >&2
  exit 1
fi

mkdir -p "$CACHE/mdb"
python3 -I "$HERE/mdb2sqlite.py" "$MDB" "$OUT"
echo "$OUT"
