#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: generate-shaped-element-shards.sh SHARD_DIRECTORY" >&2
  exit 2
fi

shard_directory="$1"
workers="${SHAPED_ELEMENT_SHARD_WORKERS:-6}"
binary="target/release/examples/generate_shaped_elements"
mkdir -p "$shard_directory"

models=(
  "wooden_twister:4:12"
  "velocity_viper:3:6"
  "hybrid:4:8"
  "family_coaster:4:10"
  "hyper:4:8"
)

active_pids=()
active_labels=()
failed_shards=()
shard_paths=()
for model_range in "${models[@]}"; do
  IFS=: read -r model_key first_car last_car <<< "$model_range"
  for ((cars = first_car; cars <= last_car; cars++)); do
    shard_path="$shard_directory/$model_key-$cars.rs"
    log_path="$shard_directory/$model_key-$cars.log"
    shard_paths+=("$shard_path")
    if [[ -s "$shard_path" ]]; then
      echo "keeping completed checkpoint $shard_path"
      continue
    fi
    echo "started shard $model_key $cars -> $shard_path"
    "$binary" --shard "$model_key" "$cars" "$shard_path" > "$log_path" 2>&1 &
    active_pids+=("$!")
    active_labels+=("$model_key $cars")
    if (( ${#active_pids[@]} == workers )); then
      for index in "${!active_pids[@]}"; do
        if wait "${active_pids[$index]}"; then
          echo "completed shard ${active_labels[$index]}"
        else
          echo "failed shard ${active_labels[$index]}"
          failed_shards+=("${active_labels[$index]}")
        fi
      done
      active_pids=()
      active_labels=()
    fi
  done
done

for index in "${!active_pids[@]}"; do
  if wait "${active_pids[$index]}"; then
    echo "completed shard ${active_labels[$index]}"
  else
    echo "failed shard ${active_labels[$index]}"
    failed_shards+=("${active_labels[$index]}")
  fi
done

if (( ${#failed_shards[@]} > 0 )); then
  printf 'failed shaped-element shards: %s\n' "${failed_shards[*]}" >&2
  exit 1
fi

"$binary" --merge crates/sim/src/track/shaped/generated_grid.rs "${shard_paths[@]}"
