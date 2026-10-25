# Roadmap

The handbook targets normative Zirk 1.x; the detailed construction and status
source is [`docs/init/ZIRK_ROADMAP.md`](../../init/ZIRK_ROADMAP.md). Phase 0,
1, 2, 3, 3b, and 4a–4e have completed scoped delivery changes, and the
`array-list-tuple-duration-regex` change delivers the everyday data surface
(`Tuple`, `Array<T>`, `List<T>`, `Duration`, `Regex`, the `String`/`Char`
method surface, and `type` alias lowering) while removing `value class`.
“Scoped” matters: `Range<T>` and `Regex.split`/`String.split` remain
explicitly assigned to the same or later changes. The Phase 3 OOP closing
slice delivered `static` members on `class`, field initializers, the `as?`
nullable cast, `TraitName.super.method()`, contract composition via
`implements` on contracts, type-parameter defaults on generic types,
comparison operators as contracts, generic contract dispatch for classes
and records, and positional variance verification. Still assigned to later
changes: trait defaults under generic substitution, nested contract
arguments (`Container<Box<T>>`), type-parameter defaults and bodies for
user-defined generic functions, and `static` on `enum`.

Phase 5 scoped delivery is complete: alongside the structured executor,
`parallel`, and typed channels, the compiler supports bounded completion and
collection combinators, public background and service supervision, managed
`Thread.run`, scalar `Atomic<T>`, and `Mutex<T>.with`. The Linux
x86_64/aarch64, macOS aarch64, and Windows x86_64 CI matrix passed, including
actual Windows execution of GC/thread checks and the workspace suite.
`Thread<T>`, `RwLock`, `Semaphore`, `Barrier`, and `Once<T>` remain explicitly
gated source APIs. These scoped results do not establish general data-race
freedom or complete `Transfer`/`Share` enforcement. Cancellation remains
cooperative, and runtime shutdown does not forcibly stop arbitrary native
work. After Phase 5, the remaining sequence moves through
projects/permissions, the rest of the standard library, functional style,
packaging, developer tooling, decorators, hardening, and eventual
self-hosting. Every feature requires one owning phase, end-to-end tests,
diagnostics, and a documentation/status update.

Phase 6 has delivered a scoped project-system slice: typed `.zkinit`,
application `entry` and package `root`, top-level entry execution without an
implicit source `fn main`, globals, logical-name dependencies, unified
`requires` declarations, exact runtime consent for exposed network and
environment APIs, `.zkinit` editor support, and `zirk init` with interactive
and `-y` defaults. Registry/Git acquisition, package publication, and
filesystem/process APIs remain incomplete. The [feature status](../11-reference/12-feature-status.md)
and [current limitations](07-current-limitations.md) pages define the current
boundary.

Future features require an explicit specification change and compatibility
analysis. Excluded 1.x ideas are not automatically roadmap commitments, and a
completed documentation design is not an implementation-complete claim.

An optional browser-hosted verified-IR interpreter is positioned as tooling
alongside compiler validation, not as a roadmap target backend. Its browser
package may grow only through reviewed compatibility policy, conformance
fixtures, and resource-boundary tests; a Zirk-to-WebAssembly target would need
a separate language, ABI, and code-generation proposal.

---

**Previous:** [← Current Limitations](07-current-limitations.md) · **Next:** [ Normative Sources](09-normative-sources.md)
