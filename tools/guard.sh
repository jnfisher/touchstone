#!/usr/bin/env bash
# Fails if a tracked file has an extension that would carry MME source or data (CLAUDE.md rule 1).
set -u
cd "$(git rev-parse --show-toplevel)"
bad=$(git ls-files | grep -iE '\.(bas|frm|cls|frx|ctl|ctx|vbp|mdb|accdb|sqlite|db|c|h)$' || true)
if [ -n "$bad" ]; then
    echo "guard: forbidden file types are tracked:" >&2
    echo "$bad" >&2
    exit 1
fi
