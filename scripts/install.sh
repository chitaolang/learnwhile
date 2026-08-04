#!/usr/bin/env bash
#
# Install LearnWhile: build the release binary and copy it onto your PATH.
#
# Usage:
#   scripts/install.sh              # installs to ~/.local/bin
#   PREFIX=/usr/local/bin scripts/install.sh   # installs elsewhere (may need sudo)
#
set -euo pipefail

BIN="learnwhile"
PREFIX="${PREFIX:-$HOME/.local/bin}"

# Run from the repo root regardless of where the script is invoked from.
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found. Install Rust from https://rustup.rs first." >&2
  exit 1
fi

echo "Building $BIN (release)..."
cargo build --release

mkdir -p "$PREFIX"
install -m 0755 "target/release/$BIN" "$PREFIX/$BIN"
echo "Installed $PREFIX/$BIN"

# Nudge the user if the install dir is not on PATH, so the command actually runs.
case ":$PATH:" in
  *":$PREFIX:"*) ;;
  *)
    echo
    echo "note: $PREFIX is not on your PATH. Add it to your shell profile, e.g.:"
    echo "  export PATH=\"$PREFIX:\$PATH\""
    ;;
esac

echo
echo "Done. Next steps:"
echo "  1. Wire up the Claude Code hook in ~/.claude/settings.json so cards appear during waits."
echo "     See the \"Wire up the Claude Code hook\" section of the README; the binary alone does"
echo "     nothing until the hook is connected."
echo "  2. Seed a deck:  $BIN seed data/anki-jlpt/n5.tsv"
echo "  3. Run it:       $BIN"
