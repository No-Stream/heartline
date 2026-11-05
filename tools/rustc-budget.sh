#!/usr/bin/env bash
# Cargo runs every compiler invocation through this script (build.rustc-wrapper in .cargo/config.toml), including bare
# `cargo` calls that bypass tools/run-in-budget.sh. A machine-wide pool of lock files limits how many rustc processes
# run at once across all worktrees and agents, so several parallel builds cannot exhaust memory. Each compile holds one
# slot until it exits; set HEARTLINE_RUSTC_SLOTS to change the pool size.
set -euo pipefail
slot_count="${HEARTLINE_RUSTC_SLOTS:-6}"
slot_directory="/tmp/heartline-rustc-slots"
mkdir -p "$slot_directory"
while true; do
  for slot in $(seq 1 "$slot_count"); do
    exec 9>"$slot_directory/$slot"
    if flock --nonblock 9; then
      exec "$(dirname "$0")/run-pinned.sh" "$@"
    fi
    exec 9>&-
  done
  sleep 0.2
done
