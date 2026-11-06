#!/usr/bin/env bash
# Runs a command on the configured compute CPUs at low priority, leaving the remaining WSL CPUs free for the Claude Code
# UI and agent processes. CI containers can override the local CPU list with their assigned CPU set.
set -euo pipefail

compute_cpus="${THEME_PARK_COMPUTE_CPUS:-$(cat "$(dirname "$0")/compute-cpus")}"
exec taskset --cpu-list "$compute_cpus" nice -n 10 "$@"
