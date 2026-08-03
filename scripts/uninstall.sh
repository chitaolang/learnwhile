#!/usr/bin/env bash
#
# Uninstall LearnWhile: remove the installed binary. By default your data is left
# untouched; pass --purge to also delete the database, logs, and socket.
#
# Usage:
#   scripts/uninstall.sh            # remove the binary from ~/.local/bin
#   scripts/uninstall.sh --purge    # also delete cards, review history, logs, socket
#   PREFIX=/usr/local/bin scripts/uninstall.sh   # match a custom install location
#
set -euo pipefail

BIN="learnwhile"
PREFIX="${PREFIX:-$HOME/.local/bin}"
purge=0
[ "${1:-}" = "--purge" ] && purge=1

target="$PREFIX/$BIN"
if [ -e "$target" ]; then
  rm -f "$target"
  echo "Removed $target"
else
  echo "No binary at $target (nothing to remove)"
fi

if [ "$purge" -eq 1 ]; then
  # XDG-aware paths, matching where the host writes them (see README).
  data="${XDG_DATA_HOME:-$HOME/.local/share}/learnwhile"
  state="${XDG_STATE_HOME:-$HOME/.local/state}/learnwhile"
  sock="${XDG_RUNTIME_DIR:-/tmp}/learnwhile.sock"

  echo
  echo "--purge will permanently delete your cards, review history, logs, and socket:"
  echo "  $data"
  echo "  $state"
  echo "  $sock"
  echo "Stop the host first if it is running."
  printf "Delete these? [y/N] "
  read -r reply
  case "$reply" in
    [yY] | [yY][eE][sS])
      rm -rf "$data" "$state"
      rm -f "$sock"
      echo "Purged."
      ;;
    *)
      echo "Kept your data. Binary removal is done."
      ;;
  esac
fi
