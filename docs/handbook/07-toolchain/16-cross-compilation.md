# Cross-Compilation

An explicit CLI target overrides manifest targets; otherwise the manifest or host applies. LLVM, linker, SDK, runtime, stdlib, and native dependencies must all support the target. Incompatibility is diagnosed before opaque linker failure.

Canonical target selection follows: explicit `--target`, then each
`build_targets` manifest entry, then detected host. An explicit target applies
to that command and does not rewrite the manifest.

Preparation checks architecture, OS/object format, LLVM support, linker, SDK,
runtime/stdlib availability, C ABI, and every native dependency. Diagnostics
name the first blocking component and its requester path. A package containing
portable IR is specialized only after the final application target is known.

```bash
zirk prepare --target aarch64-linux
zirk build --target aarch64-linux
```

Cross-building does not emulate or execute the target binary. Build scripts
execute only under their declared build-phase permissions and cannot be assumed
to run on the target. WebAssembly and modern 32-bit macOS are outside the 1.x
target promise.

---

**Previous:** [← Incremental Compilation](15-incremental-compilation.md) · **Next:** [ ABI and IR Compatibility](17-abi-and-ir-compatibility.md)
