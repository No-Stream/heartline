#!/usr/bin/env bash
# Refuses to start a build or test when the disk holding this project is nearly full. On 2026-11-05 about thirty
# worktrees' cargo target folders (1-8 GB each) filled the disk to 100%, and agents' gates failed in confusing ways.
# Free space with `cargo clean` inside merged worktrees (see docs/status.md); set HEARTLINE_MIN_FREE_GIB to change the floor.
set -euo pipefail
minimum_free_gib="${HEARTLINE_MIN_FREE_GIB:-25}"
project_directory="$(realpath "$(dirname "$0")/..")"
free_gib=$(df --block-size=1G --output=avail "$project_directory" | tail -1 | tr -d ' ')
if (( free_gib < minimum_free_gib )); then
  echo "error: only ${free_gib} GB free on the disk holding $project_directory; builds and tests need at least ${minimum_free_gib} GB." >&2
  echo "Run 'cargo clean' in merged worktrees under ../theme-park-sim-worktrees/ before continuing." >&2
  exit 1
fi
