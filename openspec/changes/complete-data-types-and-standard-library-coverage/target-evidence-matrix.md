# Target and Evidence Matrix

## Revision boundary

This evidence describes source revision
`0b6284e29041f515c1f7030310084dbfac1a292a`, executed by hosted GitHub Actions
run [41114248962](https://github.com/ChristianSmith18/zirk-lang/actions/runs/41114248962)
on 2026-10-30. The run completed successfully on all five required native
hosts. Later commits that update this matrix and synchronize documentation do
not change the tested compiler/runtime code.

Evidence stages remain distinct:

- **Check/lower**: semantic acceptance and IR lowering.
- **Interpreter/native parity**: the same fixture executes in both engines on
  the named host.
- **Object emission**: LLVM emits an object for the target triple; this does
  not prove linking or execution there.
- **Native execution**: generated Zirk executables run on the named host.
- **WASM execution**: the browser package executes under Node's WebAssembly
  runtime; this does not prove graphical-browser behavior.

## Required native hosts

The dedicated workflow `.github/workflows/advanced-data-matrix.yml` ran the same
build and focused evidence block on every row. Each host passed the target
emission, generic dispatch/body/default and Clone cases, Map/Set and collection
parity, union/intersection/Float128 parity, forced-GC cases, temporal/locale
parity, generated JSON codec behavior, and locale/zoned temporal runtime-core
tests. The JSON codec target-emission test emits objects for all nine triples
from each host. The run result is a native-host result for the checked-in
fixtures, not a proof for every possible type composition.

| Host | Hosted evidence | Status |
| --- | --- | --- |
| Linux x86_64 | Run 41114248962, job `linux-x86_64`; focused advanced data evidence passed. | Passed |
| Linux aarch64 | Run 41114248962, job `linux-aarch64`; focused advanced data evidence passed. | Passed |
| macOS x86_64 | Run 41114248962, job `macos-x86_64`; focused advanced data evidence passed. | Passed |
| macOS aarch64 | Run 41114248962, job `macos-aarch64`; focused advanced data evidence passed. | Passed |
| Windows x86_64 | Run 41114248962, job `windows-x86_64`; focused advanced data evidence passed. | Passed |

## Object-emission targets

`advanced_type_corpus_emits_for_all_nine_targets`,
`float128_runtime_helper_calls_emit_for_all_nine_targets`, and the generated
JSON codec target-emission test iterate `zirk_codegen_llvm::TARGETS`. They
establish object emission only, separately from the native-host execution rows.

| Target triple | Format | Evidence stage | Current status |
| --- | --- | --- | --- |
| `i686-pc-windows-msvc` | COFF | Advanced types, Float128, generated JSON codec object emission | Passed in run 41114248962 on each of the five host jobs |
| `x86_64-pc-windows-msvc` | COFF | Advanced types, Float128, generated JSON codec object emission | Passed in run 41114248962 on each of the five host jobs |
| `aarch64-pc-windows-msvc` | COFF | Advanced types, Float128, generated JSON codec object emission | Passed in run 41114248962 on each of the five host jobs |
| `i386-unknown-linux-gnu` | ELF | Advanced types, Float128, generated JSON codec object emission | Passed in run 41114248962 on each of the five host jobs |
| `x86_64-unknown-linux-gnu` | ELF | Advanced types, Float128, generated JSON codec object emission | Passed in run 41114248962 on each of the five host jobs |
| `armv7-unknown-linux-gnueabihf` | ELF | Advanced types, Float128, generated JSON codec object emission | Passed in run 41114248962 on each of the five host jobs |
| `aarch64-unknown-linux-gnu` | ELF | Advanced types, Float128, generated JSON codec object emission | Passed in run 41114248962 on each of the five host jobs |
| `x86_64-apple-darwin` | Mach-O | Advanced types, Float128, generated JSON codec object emission | Passed in run 41114248962 on each of the five host jobs |
| `aarch64-apple-darwin` | Mach-O | Advanced types, Float128, generated JSON codec object emission | Passed in run 41114248962 on each of the five host jobs |

## Separate web-target evidence

The release `wasm32-unknown-unknown` browser package ran under Node's WebAssembly
runtime and passed 378 behavior/corpus cases, including locale and zoned
formatting. This is separate from the nine LLVM object-emission rows and from
native host execution. Public JSON streaming is also implemented and exposed
through the Zirk JSON module: the reader accepts text or byte chunks (including
UTF-8 split across byte chunks), and the writer serializes incrementally into
bounded pull chunks with cancellation and output limits. The five-host native
and interpreter cases for that API are included in run 41114248962 above.

## Remaining limitations

The hosted matrix closes the named five-host and nine-triple evidence gates for
this change. It does not make unsupported `Map`/`Set` type combinations
admissible, prove every theoretical union/intersection composition, or convert
object emission into native execution on all nine target triples. Those
boundaries and their source diagnostics remain part of the contract.
