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

THEME_PARK_BROWSER_TESTS_IN_BUDGET=1 exec "$(dirname "$0")/../../tools/run-in-budget.sh" npx playwright test "$@"
