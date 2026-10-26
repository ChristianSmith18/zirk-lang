#!/usr/bin/env python3
"""Collect warmed CLI and LSP latency distributions for generated fixtures."""
from __future__ import annotations

import argparse
import math
from pathlib import Path
import re
import statistics
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument("directory", type=Path)
parser.add_argument("--samples", type=int, default=10)
parser.add_argument("--change-count", type=int, default=5)
args = parser.parse_args()
if args.samples < 2:
    parser.error("--samples must be at least 2")
if args.change_count < 1:
    parser.error("--change-count must be positive")

root = Path(__file__).resolve().parents[4]
lsp_harness = root / "openspec/changes/native-type-algebra-and-narrowing/benchmarks/lsp_did_open.py"
fixtures = sorted(args.directory.glob("*.zrk"))
if not fixtures:
    parser.error(f"no .zrk fixtures in {args.directory}")


def percentile95(samples: list[float]) -> float:
    ordered = sorted(samples)
    return ordered[math.ceil(0.95 * len(ordered)) - 1]


def timed(command: list[str]) -> tuple[float, str, int]:
    started = time.perf_counter()
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    elapsed = time.perf_counter() - started
    return elapsed, result.stdout + result.stderr, result.returncode


print(f"samples={args.samples}, incremental_changes_per_sample={args.change_count}")
print("fixture,cli_median_s,cli_p95_s,cli_nonzero_runs,lsp_open_median_s,lsp_open_p95_s,lsp_change_median_s,lsp_change_p95_s,lsp_diagnostic_counts")
for fixture in fixtures:
    cli_command = [str(root / "target/debug/zirk"), "check", str(fixture)]
    lsp_command = ["python3", str(lsp_harness), str(fixture)]
    incremental_command = [
        "python3", str(lsp_harness), str(fixture), "--changes", str(args.change_count)
    ]
    timed(cli_command)  # warm both code and filesystem paths
    timed(lsp_command)
    timed(incremental_command)

    cli_times: list[float] = []
    cli_failures = 0
    lsp_times: list[float] = []
    change_times: list[float] = []
    diagnostics: set[int] = set()
    for _ in range(args.samples):
        elapsed, _, status = timed(cli_command)
        cli_times.append(elapsed)
        cli_failures += status != 0

        _, output, status = timed(lsp_command)
        if status != 0:
            raise RuntimeError(f"LSP harness failed for {fixture}: {output}")
        match = re.search(r"didOpen_to_diagnostics_seconds=([0-9.]+)", output)
        count = re.search(r"diagnostics_count=(\d+)", output)
        if not match or not count:
            raise RuntimeError(f"could not parse LSP output for {fixture}: {output}")
        lsp_times.append(float(match.group(1)))
        diagnostics.add(int(count.group(1)))

        _, output, status = timed(incremental_command)
        if status != 0:
            raise RuntimeError(f"incremental LSP harness failed for {fixture}: {output}")
        edits = re.findall(r"didChange_\d+_to_diagnostics_seconds=([0-9.]+)", output)
        if len(edits) != args.change_count:
            raise RuntimeError(f"expected {args.change_count} LSP edits for {fixture}: {output}")
        change_times.extend(float(value) for value in edits)

    print(
        f"{fixture.name},{statistics.median(cli_times):.6f},{percentile95(cli_times):.6f},"
        f"{cli_failures},{statistics.median(lsp_times):.6f},{percentile95(lsp_times):.6f},"
        f"{statistics.median(change_times):.6f},{percentile95(change_times):.6f},"
        f"{'/'.join(map(str, sorted(diagnostics)))}"
    )
