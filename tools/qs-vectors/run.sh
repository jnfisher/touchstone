#!/usr/bin/env bash
# Regenerates fixtures/quicksilver/*.json (and realm_characters.sql, card R-001) from Quicksilver's own rules (card Q-005).
#   tools/qs-vectors/run.sh            write the vectors
#   tools/qs-vectors/run.sh --check    regenerate in memory and compare with the committed files
# Environment:
#   QUICKSILVER_DEV     the Quicksilver checkout, read-only here (default /home/user/quicksilver-dev)
#   QS_VECTORS_TARGET   build directory (default ~/.cache/touchstone/qs-vectors)
# Fetches nothing and never writes into the checkout. Not part of PR CI.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
qs="$(cd "${QUICKSILVER_DEV:-/home/user/quicksilver-dev}" && pwd)"
target="${QS_VECTORS_TARGET:-$HOME/.cache/touchstone/qs-vectors}"
mkdir -p "$target/pkg"
pkg="$target/pkg"

sed "s#@QUICKSILVER_DEV@#$qs#g" "$here/Cargo.toml.in" > "$pkg/Cargo.toml"
rm -rf "$pkg/src"
cp -r "$here/src" "$pkg/src"
# Quicksilver's lock file pins every shared dependency to the versions its engine is tested with.
[ -f "$pkg/Cargo.lock" ] || cp "$qs/Cargo.lock" "$pkg/Cargo.lock"

cargo build --release --quiet --manifest-path "$pkg/Cargo.toml" --target-dir "$target/build"

commit="$(git -C "$qs" rev-parse HEAD)"
version="$(tr -d '\r\n' < "$qs/content/1.20/VERSION")"
QS_COMMIT="$commit" QS_CONTENT_VERSION="$version" \
    "$target/build/release/qs-vectors" "$root/fixtures/quicksilver" "$@"
