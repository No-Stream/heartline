#!/usr/bin/env bash
# Runs a command inside this project's resource budget, so no test run or build can make the machine unusable.
#
# Normally the command joins the user cgroup slice themeparksim.slice, shared by every agent's commands at once: past
# 16 GB the kernel reclaims and throttles, and at 20 GB it kills. Where the user systemd bus is unreachable (the Codex
# sandbox), tools/process_tree_watchdog.py kills the command's process tree past 20 GB instead. Either way the command
# is pinned to the first 8 cores at low priority, which its children inherit, so the rest stay free for Windows (the
# user systemd here does not get the cgroup cpu controller, so a CPUQuota would be ignored), and anything it leaves
# running (preview servers, Chrome) is stopped when it ends. The Makefile runs every recipe line through this script.
set -euo pipefail

if [[ -n "${THEME_PARK_IN_BUDGET:-}" ]]; then
  exec "$@"
fi
export THEME_PARK_IN_BUDGET=1
pinned_command=(taskset --cpu-list 0-7 nice -n 10 "$@")

if ! systemctl --user show-environment > /dev/null 2>&1; then
  exec python3 "$(dirname "$0")/process_tree_watchdog.py" --memory-cap-gib 20 -- "${pinned_command[@]}"
fi

budget_slice="themeparksim.slice"
systemctl --user set-property --runtime "$budget_slice" MemoryHigh=16G MemoryMax=20G MemorySwapMax=0
scope_unit="themeparksim-$$-$RANDOM"
trap 'systemctl --user stop "$scope_unit.scope" 2>/dev/null || true' EXIT
systemd-run --user --scope --quiet --slice="$budget_slice" --unit="$scope_unit" -- "${pinned_command[@]}"
