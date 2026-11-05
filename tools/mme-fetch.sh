#!/usr/bin/env bash
# Fetch MME source at the pinned commit into the cache (never into the repository).
# Prints the directory. Idempotent via a marker file.
set -euo pipefail

MME_SHA="${MME_SHA:-28d23d70caf4f971418aa7ea25471cdb0ab0888a}"
CACHE="${TOUCHSTONE_CACHE:-$HOME/.cache/touchstone}"
DIR="$CACHE/mme/$MME_SHA"
MARKER="$DIR/.touchstone-fetched"

if [ -f "$MARKER" ]; then
  echo "$DIR"
  exit 0
fi

mkdir -p "$CACHE/mme"
TMP="$(mktemp -d "$CACHE/mme/.tmp.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT

URL="https://codeload.github.com/jnfisher/MMUD-Explorer/tar.gz/$MME_SHA"
ok=0
for delay in 0 2 4 8 16; do
  [ "$delay" -gt 0 ] && sleep "$delay"
  if curl -fsSL --retry 0 "$URL" -o "$TMP/src.tar.gz"; then ok=1; break; fi
done
if [ "$ok" != 1 ]; then
  # Fallback: shallow fetch of the exact commit.
  mkdir "$TMP/git"
  git -C "$TMP/git" init -q
  git -C "$TMP/git" fetch -q --depth 1 https://github.com/jnfisher/MMUD-Explorer.git "$MME_SHA"
  git -C "$TMP/git" checkout -q FETCH_HEAD
  rm -rf "$TMP/git/.git"
  mv "$TMP/git" "$TMP/out"
else
  mkdir "$TMP/out"
  tar -xzf "$TMP/src.tar.gz" -C "$TMP/out" --strip-components=1
fi

touch "$TMP/out/.touchstone-fetched"
rm -rf "$DIR"
mv "$TMP/out" "$DIR"
echo "$DIR"
