#!/usr/bin/env bash
# Runs Playwright inside the project's resource budget (tools/run-in-budget.sh), one run at a time on this machine.
#
# Every browser test worker is a Chrome doing software-rendered WebGL plus the WebAssembly simulation, several GB each.
# Overlapping runs from parallel agents once took 32 GB and froze the Windows host, so a second run waits for the first.
set -euo pipefail

lock_file="/tmp/theme-park-sim-browser-tests.lock"
exec 9>"$lock_file"
if ! flock --nonblock 9; then
  echo "Another browser test run holds $lock_file; waiting for it to finish." >&2
  flock 9
fi

if [[ "${THEME_PARK_PLAYWRIGHT_IN_CONTAINER:-0}" == "1" ]]; then
  exec "$(dirname "$0")/../../tools/run-in-budget.sh" npx playwright test "$@"
fi

if [[ "${THEME_PARK_PLAYWRIGHT_DOCKER:-0}" == "1" ]]; then
  repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
  git_entry="$repo_root/.git"
  git_common_directory="$(git -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)"
  playwright_image="mcr.microsoft.com/playwright:v1.64.1-noble@sha256:2f05f8a734509a7b3f26a912639488ee08f58a7b292d14d62271bcff9038768f"
  selected_project=""
  previous_argument=""

  for argument in "$@"; do
    if [[ "$previous_argument" == "--project" ]]; then
      selected_project="$argument"
    fi
    case "$argument" in
      --project=*) selected_project="${argument#--project=}" ;;
    esac
    previous_argument="$argument"
  done

  if [[ "$selected_project" != "feature" && "$selected_project" != "journey" ]]; then
    echo "Docker browser runs currently support only --project=feature or --project=journey." >&2
    exit 2
  fi

  docker_cpuset="$(awk -F: '
    {
      stride = NF == 2 ? $2 : 1
      bounds_count = split($1, bounds, "-")
      first_cpu = bounds[1]
      last_cpu = bounds_count == 2 ? bounds[2] : first_cpu
      for (cpu = first_cpu; cpu <= last_cpu; cpu += stride) {
        if (selected_cpu_count == 4) break
        printf "%s%d", separator, cpu
        separator = ","
        selected_cpu_count++
      }
    }
    END { if (NR != 1 || selected_cpu_count != 4) exit 1 }
  ' "$repo_root/tools/compute-cpus")"
  if [[ -z "$docker_cpuset" ]]; then
    echo "Could not select four Docker CPUs from tools/compute-cpus." >&2
    exit 2
  fi
  export THEME_PARK_COMPUTE_CPUS="$docker_cpuset"

  fixture_path="${THEME_PARK_PIECE_CATALOG_FIXTURE:-}"
  if [[ -z "$fixture_path" ]]; then
    fixture_path="$(mktemp /tmp/theme-park-piece-catalog.XXXXXX.json)"
    trap 'rm -f "$fixture_path"' EXIT
    "$repo_root/tools/run-in-budget.sh" "$HOME/.cargo/bin/cargo" run --quiet --release \
      --manifest-path "$repo_root/Cargo.toml" -p theme-park-sim --example piece_visual_fixture > "$fixture_path"
  else
    fixture_path="$(realpath "$fixture_path")"
  fi

  container_fixture_path="/tmp/piece-catalog-fixture.json"
  container_port="${PLAYWRIGHT_PORT:-4173}"
  docker_arguments=(
    run --rm --init
    --cpuset-cpus "$docker_cpuset"
    --cpus 4
    --memory 12g
    --memory-swap 12g
    --shm-size 2g
    --user "$(id -u):$(id -g)"
    --workdir /workspace
    --volume "$repo_root:/workspace"
    --volume "$git_entry:/workspace/.git:ro"
    --volume "$git_common_directory:$git_common_directory:ro"
    --volume "$fixture_path:$container_fixture_path:ro"
    --env HOME=/tmp
    --env PLAYWRIGHT_PORT="$container_port"
    --env THEME_PARK_COMPUTE_CPUS="$docker_cpuset"
    --env THEME_PARK_BROWSER_TESTS_IN_BUDGET=1
    --env THEME_PARK_PLAYWRIGHT_DOCKER=1
    --env THEME_PARK_PIECE_CATALOG_FIXTURE="$container_fixture_path"
  )
  if [[ "$selected_project" == "journey" ]]; then
    docker_arguments+=(--env THEME_PARK_LIVE_PAUSE_RENDER=1)
  fi
  docker_arguments+=("$playwright_image" npx playwright test)
  "$repo_root/tools/run-in-budget.sh" docker "${docker_arguments[@]}" "$@"
  exit 0
fi

THEME_PARK_BROWSER_TESTS_IN_BUDGET=1 exec "$(dirname "$0")/../../tools/run-in-budget.sh" npx playwright test "$@"
