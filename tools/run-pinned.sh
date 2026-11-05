#!/usr/bin/env bash
# Runs a command on the compute CPUs (tools/compute-cpus) at low priority, leaving the remaining WSL CPUs free for the
# Claude Code UI and agent processes. Cargo uses it as the runner for native test and run binaries (.cargo/config.toml),
# so bare `cargo test` from any agent is pinned too.
set -euo pipefail
exec taskset --cpu-list "$(cat "$(dirname "$0")/compute-cpus")" nice -n 10 "$@"
