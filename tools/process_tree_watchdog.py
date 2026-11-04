"""Runs a command in its own session and kills the whole session if its memory goes over a cap.

This is the fallback for tools/run-in-budget.sh where the user systemd bus is unreachable (the Codex sandbox), so no
cgroup can be created. Memory is the sum of proportional set sizes (shared pages split between their users) over every
process in the command's session, sampled every two seconds. Anything still running when the command ends is killed.
"""

import argparse
import logging
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

logger: logging.Logger = logging.getLogger("process_tree_watchdog")
SAMPLE_INTERVAL_SECONDS = 2.0


def session_processes(session_id: int) -> list[int]:
    process_ids = []
    for process_dir in Path("/proc").iterdir():
        if not process_dir.name.isdigit():
            continue
        try:
            if os.getsid(int(process_dir.name)) == session_id:
                process_ids.append(int(process_dir.name))
        except ProcessLookupError:
            continue
    return process_ids


def proportional_set_size_bytes(process_id: int) -> int:
    try:
        rollup = Path(f"/proc/{process_id}/smaps_rollup").read_text()
    except (FileNotFoundError, ProcessLookupError):
        return 0
    for line in rollup.splitlines():
        if line.startswith("Pss:"):
            return int(line.split()[1]) * 1024
    raise ValueError(f"No Pss line in /proc/{process_id}/smaps_rollup")


def kill_session(session_id: int) -> None:
    try:
        os.killpg(session_id, signal.SIGKILL)
    except ProcessLookupError:
        pass


def run_with_memory_cap(command: list[str], memory_cap_bytes: int) -> int:
    child = subprocess.Popen(command, start_new_session=True)
    for forwarded_signal in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(forwarded_signal, lambda signum, _frame: os.killpg(child.pid, signum))
    peak_bytes = 0
    try:
        while child.poll() is None:
            used_bytes = sum(proportional_set_size_bytes(process_id) for process_id in session_processes(child.pid))
            peak_bytes = max(peak_bytes, used_bytes)
            if used_bytes > memory_cap_bytes:
                logger.error("Killed %s: it used %.1f GiB, over the %.1f GiB cap.", command[0], used_bytes / 2**30, memory_cap_bytes / 2**30)
                kill_session(child.pid)
                child.wait()
                return 137
            time.sleep(SAMPLE_INTERVAL_SECONDS)
        return child.returncode
    finally:
        kill_session(child.pid)
        if peak_bytes >= 2**30:
            logger.info("%s peaked at %.1f GiB.", command[0], peak_bytes / 2**30)


def main() -> None:
    logging.basicConfig(level=logging.INFO, format="%(name)s: %(message)s")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--memory-cap-gib", type=float, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command[1:] if arguments.command[:1] == ["--"] else arguments.command
    if not command:
        parser.error("no command given")
    sys.exit(run_with_memory_cap(command, int(arguments.memory_cap_gib * 2**30)))


if __name__ == "__main__":
    main()
