#!/usr/bin/env bash
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
scratch=$(mktemp -d /tmp/task-126-preview-XXXXXX)
git -C "$repo" archive bc3a68e196290de317fa900c182e843b6b1e4ed4 | tar -x -C "$scratch"
git -C "$repo" show b240068:crates/sim/src/physics/force_evaluation.rs > "$scratch/crates/sim/src/physics/force_evaluation.rs"
cp "$repo/tools/force-measurement/measure_preview_force.rs" "$scratch/crates/sim/examples/measure_preview_force.rs"
python3 "$repo/tools/force-measurement/instrument-preview.py" "$scratch"
cd "$scratch"
if [[ ${1:-} == --timing ]]; then
  for run in 1 2 3; do
    printf '\nCandidate timing run %s\n' "$run"
    make bench-player-circuits
  done
else
  tools/run-in-budget.sh cargo run --release -p theme-park-sim --example measure_preview_force -- "$@"
fi
