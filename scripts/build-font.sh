#!/usr/bin/env bash
set -euo pipefail

INPUT="fonts/JetBrainsMono-Regular.ttf"
OUTPUT="fonts/JetBrainsMono-APB.ttf"

if [[ ! -f "$INPUT" ]]; then
    echo "Missing $INPUT. See fonts/README.md for download instructions." >&2
    exit 1
fi

cargo run --release --bin apb-font-patch -- "$INPUT" -o "$OUTPUT"

cat <<EOF

Built $OUTPUT.

Install:
  macOS : cp "$OUTPUT" ~/Library/Fonts/
  Linux : mkdir -p ~/.local/share/fonts && cp "$OUTPUT" ~/.local/share/fonts/ && fc-cache -f
Then:   export APB_FONT_PATCHED=1
EOF
