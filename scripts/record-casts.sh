#!/usr/bin/env bash
set -euo pipefail

if ! command -v asciinema &>/dev/null; then
    echo "asciinema not installed. brew install asciinema (macOS) or apt install asciinema (Linux)." >&2
    exit 1
fi

mkdir -p casts
asciinema rec -c "cargo run -q --example basic"          casts/01-overview.cast
asciinema rec -c "cargo run -q --example narrow"         casts/02-precision.cast
asciinema rec -c "cargo run -q --example degrade_ladder" casts/03-degrade.cast
echo "Casts written to casts/"
