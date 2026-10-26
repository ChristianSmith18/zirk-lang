#!/usr/bin/env python3
"""Measure the native build path at and immediately above the inline union cap."""

from __future__ import annotations

import argparse
import math
import statistics
import subprocess
import time
from pathlib import Path


def source_for(tuple_elements: int) -> str:
    tuple_type = ", ".join(["Int32"] * tuple_elements)
    return (
        f"type BigTuple = Tuple<{tuple_type}>;\n"
        "enum Big { Data(payload: BigTuple) }\n"
        "type Huge = Big | String;\n"
        "fn take(value: Huge): Huge { return value; }\n"
        "fn main(): Void { }\n"
    )


def sample(command: list[str], cwd: Path, expect_success: bool) -> float:
    start = time.perf_counter()
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True)
    elapsed_ms = (time.perf_counter() - start) * 1000
    if expect_success and result.returncode != 0:
        raise RuntimeError(result.stderr)
    if not expect_success and (result.returncode == 0 or "E0513" not in result.stderr):
        raise RuntimeError(
            f"expected E0513 from rejected payload, exit={result.returncode}:\n{result.stderr}"
        )
    return elapsed_ms


def report(label: str, values: list[float]) -> None:
    ordered = sorted(values)
    p95 = ordered[math.ceil(0.95 * len(ordered)) - 1]
    print(
        f"{label}: n={len(values)} median_ms={statistics.median(values):.2f} "
        f"p95_ms={p95:.2f} min_ms={min(values):.2f} max_ms={max(values):.2f}"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("--zirk", type=Path, default=Path("target/debug/zirk"))
    parser.add_argument("--samples", type=int, default=12)
    args = parser.parse_args()
    if args.samples < 2:
        parser.error("--samples must be at least 2")
    compiler = args.zirk.resolve()
    args.output_dir.mkdir(parents=True, exist_ok=True)

    cases = [
        ("inline-4096", 1023, True),
        ("reject-4100", 1024, False),
    ]
    for label, tuple_elements, expect_success in cases:
        case_dir = args.output_dir / label
        case_dir.mkdir(parents=True, exist_ok=True)
        source = case_dir / "main.zrk"
        source.write_text(source_for(tuple_elements), encoding="utf-8")
        command = [str(compiler), "build", str(source.relative_to(args.output_dir))]
        for _ in range(2):
            sample(command, args.output_dir, expect_success)
        values = [
            sample(command, args.output_dir, expect_success)
            for _ in range(args.samples)
        ]
        report(label, values)


if __name__ == "__main__":
    main()
