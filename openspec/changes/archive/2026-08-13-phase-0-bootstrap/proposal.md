## Why

The project has five complete normative specs and zero lines of code. `ZIRK_ROADMAP.md` Phase 0 requires resolving, before writing any of the language, the decisions that cascade into everything else and assembling the skeleton that materializes them.

The decisions have already been made and recorded as ADRs, and the LLVM sanity check has already passed on `aarch64-macos`. What's missing is the verifiable skeleton: a workspace that builds and a CI that demonstrates it builds on all three platforms, not just on the author's machine.

Without this, any Phase 1 work is built on a toolchain whose portability is an assumption.

## What Changes

- **Cargo workspace with nine crates**, one per pipeline stage from `ZIRK_COMPILER_SPEC.md` section 2: `zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-diagnostics`, `zirk-cli`, and `zirk-runtime`.
- **`zirk-runtime` as a staticlib with a C ABI boundary** (ADR-002). It is not in the roadmap's original list; it is added as the ninth crate.
- **`zirk-diagnostics` operational from day one** with the format from `ZIRK_COMPILER_SPEC.md` section 8 (severity, stable code, location, cause, help), even though it has no real consumers at first.
- **LLVM sanity check incorporated into the repo as a test**, not as a disposable spike: it verifies that the inkwell → LLVM IR → object → link → execution chain works, and that valid objects are emitted for the nine targets in the spec.
- **CI over the `{windows, linux, macos} × {x86_64, aarch64}` matrix**, which is what turns portability from intention into verified fact.
- **LLVM 20.1 pin documented and applied** in the workspace and in CI (ADR-001, `docs/TOOLCHAIN.md`).

Explicitly **out of scope**: any Zirk syntax. By the end of this change, not a single `.zrk` file is parsed. The lexer, parser, and other crates exist but are empty.

## Capabilities

### New Capabilities

- `toolchain-bootstrap`: the reproducible build toolchain for the compiler — the LLVM pin, environment variable, per-platform dependencies, and the verification that the full chain produces and runs a native binary.
- `compiler-workspace`: the compiler's crate structure, their responsibility boundaries, and the dependency rule between pipeline stages.
- `diagnostics-format`: the diagnostics contract from `ZIRK_COMPILER_SPEC.md` section 8 — severity, stable code, location, cause, and help.
- `target-matrix`: the targets Zirk must be able to emit and the platforms on which the compiler must be able to build, with their verification criteria.

### Modified Capabilities

None: no previous specs exist in `openspec/specs/`.

## Impact

**New:**
- Root `Cargo.toml` (workspace) and nine `crates/*/`
- `.github/workflows/ci.yml`
- `rust-toolchain.toml` to pin the Rust version

**Already existing, referenced:**
- `docs/decisions/ADR-001..005` — durable source of the decisions; this change references them, it does not duplicate them
- `docs/TOOLCHAIN.md` — per-platform installation

**External dependencies introduced:**
- `inkwell` 0.10 (feature `llvm20-1`) → `llvm-sys` 201 → system LLVM 20.1
- Requires `LLVM_SYS_201_PREFIX` on every development machine and every CI runner

**Risks:**
- Windows is the platform with the most friction: it requires the official LLVM development tarball, not the `.exe` installer. If the Windows runner fails to build, that is a blocking finding for this change, not a detail to defer.
- Cross-**linking** (per-target sysroots) is out of scope; only object emission is covered. Phase 6 work, recorded in ADR-004.
</content>
