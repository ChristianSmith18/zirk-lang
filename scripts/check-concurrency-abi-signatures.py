#!/usr/bin/env python3
"""Compare target-lowered C consumer signatures with Rust runtime LLVM IR."""

import argparse
import re
import sys
from pathlib import Path


SYMBOLS = (
    "zirk_rt_init",
    "zirk_rt_shutdown",
    "zirk_rt_run_main_cli",
    "zirk_rt_run_main_with_deadline_policy",
    "zirk_rt_request_host_shutdown",
    "zirk_rt_spawn",
    "zirk_rt_job_wait_any",
    "zirk_rt_job_wait_any_with_timer",
    "zirk_rt_job_outcome_take",
    "zirk_rt_job_outcome_wait_take",
    "zirk_rt_cancel",
    "zirk_rt_cancel_with_cleanup_failure",
    "zirk_rt_scope_enter",
    "zirk_rt_scope_enter_collect_all",
    "zirk_rt_scope_exit",
    "zirk_rt_scope_exit_collect_all",
    "zirk_rt_push_frame",
    "zirk_rt_pop_frame",
    "zirk_rt_cancel_shield_enter",
    "zirk_rt_cancel_shield_exit",
    "zirk_rt_atomic_new",
    "zirk_rt_atomic_free",
    "zirk_rt_atomic_load",
    "zirk_rt_atomic_store",
    "zirk_rt_atomic_exchange",
    "zirk_rt_atomic_compare_exchange",
    "zirk_rt_atomic_fetch_add",
    "zirk_rt_atomic_fetch_sub",
    "zirk_rt_gc_mutex_alloc",
    "zirk_rt_gc_mutex_acquire",
    "zirk_rt_gc_mutex_release",
    "zirk_rt_gc_mutex_with",
)

LLVM_TYPE = re.compile(r"^(void|ptr|i[0-9]+|float|double)\b")


def split_params(params: str) -> list[str]:
    if not params.strip() or params.strip() == "void":
        return []
    parts = []
    depth = 0
    start = 0
    for i, char in enumerate(params):
        if char in "([{<":
            depth += 1
        elif char in ")]}>":
            depth -= 1
        elif char == "," and depth == 0:
            parts.append(params[start:i].strip())
            start = i + 1
    parts.append(params[start:].strip())
    return parts


def signature(text: str, symbol: str, path: Path) -> tuple[str, ...]:
    found = []
    for line in text.splitlines():
        if not (line.lstrip().startswith("define ") or line.lstrip().startswith("declare ")):
            continue
        match = re.search(rf"@{re.escape(symbol)}\((.*)\)", line)
        if not match:
            continue
        prefix = line[: match.start()].strip()
        return_types = re.findall(r"\b(?:void|ptr|i[0-9]+|float|double)\b", prefix)
        if not return_types:
            raise ValueError(f"cannot parse return type for {symbol} in {path}: {line}")
        params = []
        for param in split_params(match.group(1)):
            param_type = LLVM_TYPE.match(param)
            if not param_type:
                raise ValueError(f"cannot parse parameter type for {symbol} in {path}: {param}")
            params.append(param_type.group(1))
        found.append((return_types[-1], *params))
    if len(found) != 1:
        raise ValueError(f"expected exactly one LLVM signature for {symbol} in {path}; found {len(found)}")
    return found[0]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rust-llvm", required=True, type=Path)
    parser.add_argument("--consumer-llvm", required=True, type=Path)
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    try:
        rust_text = args.rust_llvm.read_text()
        consumer_text = args.consumer_llvm.read_text()
        for symbol in SYMBOLS:
            actual = signature(rust_text, symbol, args.rust_llvm)
            declared = signature(consumer_text, symbol, args.consumer_llvm)
            if actual != declared:
                raise ValueError(
                    f"{args.target}: ABI mismatch for {symbol}: Rust {actual}, C consumer {declared}"
                )
        print(f"{args.target}: matched lowered Rust/C signatures for {len(SYMBOLS)} ABI symbols")
        return 0
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
