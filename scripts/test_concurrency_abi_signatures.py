#!/usr/bin/env python3
import importlib.util
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("check-concurrency-abi-signatures.py")
SPEC = importlib.util.spec_from_file_location("concurrency_abi_signatures", MODULE_PATH)
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


class SignatureParserTests(unittest.TestCase):
    def parse(self, ir: str) -> tuple[str, ...]:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.ll"
            path.write_text(ir)
            return CHECKER.signature(ir, "zirk_rt_probe", path)

    def test_parses_lowered_return_and_argument_types(self):
        signature = self.parse("define dso_local zeroext i1 @zirk_rt_probe(ptr %p, i64 %n) { ret i1 0 }")
        self.assertEqual(signature, ("i1", "ptr", "i64"))

    def test_return_width_drift_is_distinguishable(self):
        rust = self.parse("define i64 @zirk_rt_probe(i32 %value) { ret i64 0 }")
        consumer = self.parse("declare i32 @zirk_rt_probe(i32 %value)")
        self.assertNotEqual(rust, consumer)

    def test_parameter_width_drift_is_distinguishable(self):
        rust = self.parse("define i32 @zirk_rt_probe(i64 %value) { ret i32 0 }")
        consumer = self.parse("declare i32 @zirk_rt_probe(i32 %value)")
        self.assertNotEqual(rust, consumer)

    def test_missing_or_duplicate_symbol_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.ll"
            with self.assertRaises(ValueError):
                CHECKER.signature("", "zirk_rt_probe", path)
            duplicate = "declare i32 @zirk_rt_probe()\ndeclare i32 @zirk_rt_probe()"
            with self.assertRaises(ValueError):
                CHECKER.signature(duplicate, "zirk_rt_probe", path)


class NestedCallbackAbiTests(unittest.TestCase):
    def test_consumer_pins_spawn_callback_prototype(self):
        source = Path(__file__).with_name("concurrency-abi-consumer.c").read_text()
        self.assertIn("typedef size_t (*ZirkSpawnCallback)(void *);", source)
        self.assertIn("__typeof__(&zirk_rt_spawn)", source)
        self.assertIn("uint64_t (*)(ZirkSpawnCallback, void *)", source)

    @unittest.skipUnless(shutil.which("clang"), "clang required for C ABI negative test")
    def test_callback_return_width_drift_fails_compile_time_guard(self):
        source = r"""
#include <stddef.h>
#include <stdint.h>
typedef size_t (*ZirkSpawnCallback)(void *);
extern uint64_t zirk_rt_spawn(void (*)(void *), void *);
_Static_assert(__builtin_types_compatible_p(__typeof__(&zirk_rt_spawn),
                                           uint64_t (*)(ZirkSpawnCallback, void *)),
               "zirk_rt_spawn callback ABI drift");
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "callback-drift.c"
            path.write_text(source)
            result = subprocess.run(
                [shutil.which("clang"), "-std=c11", "-fsyntax-only", str(path)],
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("callback ABI drift", result.stderr)


if __name__ == "__main__":
    unittest.main()
