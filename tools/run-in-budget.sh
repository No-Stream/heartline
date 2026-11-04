#!/usr/bin/env bash
# Runs a command inside this project's shared resource budget, the user cgroup slice themeparksim.slice.
#
# The budget covers every command run through it at once, from all agents, so parallel test runs cannot take over the
# machine: past 16 GB the kernel reclaims and throttles, and at 20 GB it kills. Commands are pinned to the first 8 cores
# at low priority, which children inherit, so the rest stay free for Windows; the user systemd here does not get the cgroup
# cpu controller, so a CPUQuota would be ignored. The Makefile runs every recipe line through this script. When the
# command ends, anything it left running (preview servers, Chrome) is stopped with it.
set -euo pipefail

budget_slice="themeparksim.slice"
if grep -q "/$budget_slice/" /proc/self/cgroup; then
  exec "$@"
fi

systemctl --user set-property --runtime "$budget_slice" MemoryHigh=16G MemoryMax=20G MemorySwapMax=0
scope_unit="themeparksim-$$-$RANDOM"
trap 'systemctl --user stop "$scope_unit.scope" 2>/dev/null || true' EXIT
systemd-run --user --scope --quiet --slice="$budget_slice" --unit="$scope_unit" -- taskset --cpu-list 0-7 nice -n 10 "$@"
