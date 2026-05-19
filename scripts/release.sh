#!/usr/bin/env bash
#
# Interactive release picker for pixbar.
#
# Reads the current version from Cargo.toml, computes the next
# patch/minor/major target, and lets you pick a release type via fzf.
# Hands off to `cargo release` which bumps Cargo.toml, commits, tags,
# and pushes. CI publishes via OIDC.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

require() {
  command -v "$1" &>/dev/null || {
    echo "missing: $1" >&2
    case "$1" in
      fzf)           echo "  brew install fzf  (macOS) | apt install fzf (Linux)" >&2 ;;
      cargo-release) echo "  cargo install cargo-release" >&2 ;;
    esac
    exit 1
  }
}
require fzf
require cargo-release

# Parse current version from Cargo.toml ([package] table only — first match).
CUR="$(awk '
  /^\[package\]/ { in_pkg = 1; next }
  /^\[/          { in_pkg = 0 }
  in_pkg && /^version *= */ {
    if (match($0, /"[^"]*"/)) print substr($0, RSTART + 1, RLENGTH - 2)
    exit
  }
' Cargo.toml)"

if [[ -z "$CUR" ]]; then
  echo "could not read version from Cargo.toml" >&2
  exit 1
fi

# Split semver: 1.2.3-rc.4+build.5 → MAJ MIN PAT PRE
CORE="${CUR%%[-+]*}"                          # 1.2.3
PRE="$(echo "$CUR" | sed -nE 's/[^-]*-([^+]+).*/\1/p')"  # rc.4 (empty if none)
IFS='.' read -r MAJ MIN PAT <<<"$CORE"

if [[ -z "$PRE" ]]; then
  NEXT_PATCH="${MAJ}.${MIN}.$((PAT + 1))"
  NEXT_MINOR="${MAJ}.$((MIN + 1)).0"
  NEXT_MAJOR="$((MAJ + 1)).0.0"
  RELEASE_HINT="(no prerelease to finalize)"
else
  # Currently on prerelease → "release" finalizes to the same X.Y.Z.
  NEXT_PATCH="${MAJ}.${MIN}.$((PAT + 1))"
  NEXT_MINOR="${MAJ}.$((MIN + 1)).0"
  NEXT_MAJOR="$((MAJ + 1)).0.0"
  RELEASE_HINT="→ ${CORE}  (drop -${PRE})"
fi

# Pre-release hints: next patch with -alpha.0 / -beta.0 / -rc.0 if currently stable;
# otherwise bump the prerelease counter (best-effort; cargo-release computes final).
if [[ -z "$PRE" ]]; then
  ALPHA_HINT="→ ${NEXT_PATCH}-alpha.0"
  BETA_HINT="→ ${NEXT_PATCH}-beta.0"
  RC_HINT="→ ${NEXT_PATCH}-rc.0"
else
  ALPHA_HINT="→ bump -${PRE} → next alpha"
  BETA_HINT="→ bump -${PRE} → next beta"
  RC_HINT="→ bump -${PRE} → next rc"
fi

# Build menu. Columns padded so fzf shows clean alignment.
printf -v MENU '%s\n' \
  "patch     ${CUR} → ${NEXT_PATCH}" \
  "minor     ${CUR} → ${NEXT_MINOR}" \
  "major     ${CUR} → ${NEXT_MAJOR}" \
  "rc        ${CUR} ${RC_HINT}" \
  "beta      ${CUR} ${BETA_HINT}" \
  "alpha     ${CUR} ${ALPHA_HINT}" \
  "release   ${CUR} ${RELEASE_HINT}" \
  "custom    enter exact version manually" \
  "cancel    abort"

CHOICE="$(printf '%s' "$MENU" | fzf \
  --prompt="release pixbar  current=${CUR}  > " \
  --height=12 \
  --no-sort \
  --header='↑↓ to pick · Enter to run · Esc to cancel')" || exit 1

KIND="$(awk '{print $1}' <<<"$CHOICE")"

case "$KIND" in
  cancel) echo "aborted"; exit 0 ;;
  custom)
    read -rp "exact version (e.g. 1.0.0-rc.1): " VER
    [[ -n "$VER" ]] || { echo "no version given" >&2; exit 1; }
    exec cargo release "$VER" --execute --no-publish --no-confirm
    ;;
  patch|minor|major|alpha|beta|rc|release)
    exec cargo release "$KIND" --execute --no-publish --no-confirm
    ;;
  *)
    echo "unknown choice: $KIND" >&2; exit 1 ;;
esac
