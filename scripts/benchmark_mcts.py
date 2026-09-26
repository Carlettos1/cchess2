#!/usr/bin/env python3
"""Sequential release auto-play benchmarks. Build before invoking this script."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile
import time


def engine_threads(pid):
    """Observe only cchess-engine processes in this invocation's process tree."""
    maximum = 0
    pending = [pid]
    while pending:
        current = pending.pop()
        try:
            base = Path(f"/proc/{current}")
            pending.extend(int(p) for p in (base / f"task/{current}/children").read_text().split())
            if (base / "comm").read_text().strip() == "cchess-engine":
                status = (base / "status").read_text()
                maximum = max(maximum, int(re.search(r"Threads:\s+(\d+)", status)[1]))
        except (OSError, TypeError):
            pass  # Process exited, or /proc is unavailable on this platform.
    return maximum


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--baseline-executable", type=Path)
    parser.add_argument("--repetitions", type=int, default=3)
    args = parser.parse_args()
    if args.repetitions < 1:
        parser.error("--repetitions must be positive")
    root = Path(__file__).resolve().parents[1]
    output = args.output_dir or Path(tempfile.mkdtemp(prefix="cchess-mcts-"))
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    cases = [
        ("classical", ["--max-actions", "100"]),
        ("full", ["--full-board", "--max-actions", "20"]),
        ("four", ["--four-player", "--max-actions", "20"]),
    ]
    commands = [("optimized", ["cargo", "run", "--release", "--offline", "-j", "1", "--"])]
    if args.baseline_executable:
        commands.insert(0, ("baseline", [str(args.baseline_executable.resolve())]))
    results = []
    for case, flags in cases:
        for repetition in range(args.repetitions):
            for version, prefix in commands:
                label = f"{version}-{case}-{repetition + 1}"
                log = output / f"{label}.log"
                command = prefix + ["--auto-play", str(log), "--ai", "mcts", "--ai-seed", "42"] + flags
                started = time.perf_counter()
                max_threads = 0
                with (output / f"{label}.stdout").open("x") as stdout, (output / f"{label}.stderr").open("x+") as stderr:
                    process = subprocess.Popen(command, cwd=root, stdout=stdout, stderr=stderr)
                    while process.poll() is None:
                        max_threads = max(max_threads, engine_threads(process.pid))
                        time.sleep(0.01)
                    wall = time.perf_counter() - started
                    stderr.seek(0)
                    text = stderr.read()
                timing = re.search(r"Timing: ([\d.]+)s total, ([\d.]+)s AI selection", text)
                outcome = re.search(r"(COMPLETED|UNFINISHED:[^\n]*?) · (\d+) actions · log:", text)
                if not timing or not outcome or (process.returncode != 0 and "action limit reached" not in text):
                    raise RuntimeError(f"{label} failed: {text}")
                result = dict(label=label, command=command, wall_seconds=wall,
                              game_seconds=float(timing[1]), selection_seconds=float(timing[2]),
                              status=outcome[1], actions=int(outcome[2]),
                              engine_threads_observed=max_threads or None,
                              sha256=hashlib.sha256(log.read_bytes()).hexdigest())
                results.append(result)
                (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
                print(f"{label}: {result['game_seconds']:.3f}s, {result['actions']} actions, "
                      f"{result['status']}, threads={result['engine_threads_observed']}", flush=True)
    print(f"Results: {output / 'results.json'}")


if __name__ == "__main__":
    main()
