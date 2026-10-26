#!/usr/bin/env python3
"""Measure maximum resident set size with macOS /usr/bin/time -l."""
from __future__ import annotations

import argparse
import math
from pathlib import Path
import statistics
import subprocess
import re

parser = argparse.ArgumentParser()
parser.add_argument("directory", type=Path)
parser.add_argument("--samples", type=int, default=3)
parser.add_argument("--zirk", type=Path)
parser.add_argument("--lsp-server", type=Path)
args = parser.parse_args()
if args.samples < 2:
    parser.error("--samples must be at least 2")

root = Path(__file__).resolve().parents[4]
lsp_harness = root / "openspec/changes/native-type-algebra-and-narrowing/benchmarks/lsp_did_open.py"
zirk = args.zirk or (root / "target/debug/zirk")
lsp_server = args.lsp_server or (root / "target/debug/zirk-lsp")
fixtures = sorted(args.directory.glob("*.zrk"))


def peak_rss(command: list[str]) -> int:
    result = subprocess.run(
        ["/usr/bin/time", "-l", *command],
        capture_output=True,
        text=True,
        check=False,
    )
    match = re.search(r"([0-9]+)\s+maximum resident set size", result.stderr)
    if not match:
        raise RuntimeError(f"could not parse /usr/bin/time output: {result.stderr}")
    return int(match.group(1))


def p95(samples: list[int]) -> int:
    return sorted(samples)[math.ceil(0.95 * len(samples)) - 1]


print(f"samples={args.samples}; measure=/usr/bin/time -l")
print("fixture,cli_median_bytes,cli_p95_bytes,lsp_median_bytes,lsp_p95_bytes")
for fixture in fixtures:
    cli = [str(zirk), "check", str(fixture)]
    lsp = ["python3", str(lsp_harness), str(fixture), "--server", str(lsp_server)]
    peak_rss(cli)
    peak_rss(lsp)
    cli_values = [peak_rss(cli) for _ in range(args.samples)]
    lsp_values = [peak_rss(lsp) for _ in range(args.samples)]
    print(
        f"{fixture.name},{statistics.median(cli_values):.0f},{p95(cli_values)},"
        f"{statistics.median(lsp_values):.0f},{p95(lsp_values)}"
    )
