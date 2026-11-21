#!/bin/sh
set -eu

BIN=${1:-./sfetch}

help_output=$("$BIN" --help)
printf '%s\n' "$help_output" | grep -q 'Usage:'
printf '%s\n' "$help_output" | grep -q -- '--update'

if "$BIN" --definitely-invalid >/dev/null 2>&1; then
    echo 'FAIL: unknown option returned success' >&2
    exit 1
fi

# System probes may report N/A in containers and restricted environments.
"$BIN" >/dev/null

echo 'SmartFetch smoke tests: PASS'
