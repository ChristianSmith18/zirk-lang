## Context

The compiler's own native exception hierarchy (`Error`/`Throwable`/`RuntimeError` and their descendants) is registered directly as `ClassKind::Abstract` classes (`checker.rs:1270`+, bypassing ordinary user-declaration checking) and already dispatches dynamically through `InstKind::CallVirtual` (`lower.rs:5370-5389`) whenever a method is `overridden`. `catch Throwable(e) { e.message() }` is real, working, production dynamic dispatch through an abstract-class-typed value today. A user-declared `abstract class` hits a hard `not_lowered` gate (`checker.rs:2474-2483`) purely because nobody has yet confirmed the same mechanism generalizes past the compiler's own hand-built hierarchy.

## Goals / Non-Goals

**Goals:**
- A user-declared `abstract class` can be named as a type, and a value of that type (backed by some concrete adopter) dispatches to the adopter's own override — identically in mechanism to `Throwable`.
- No change to the native exception hierarchy's own behavior.

**Non-Goals:** value-type contract dispatch (a `record`/`value class` implementing a contract) — a different representation problem, tracked separately.

## Decisions

### D1: Reuse the existing method-table/virtual-index machinery unmodified; the fix is in how a user abstract class's method list is built and indexed, not in `CallVirtual` itself

`CallVirtual`'s lowering (`lower.rs:5370-5389`) already dispatches purely off `method.overridden`/`method.index` — it has no special knowledge of *which* class declared the method, abstract or concrete. The native exception hierarchy proves this: `Throwable.message()`'s `overridden: true` flag and its `index` in the flattened method list are set at native-registration time (`checker.rs:1289-1300`), and `CallVirtual` reads them the same way it would for any user class's overridden method. The fix is making the ordinary user-declaration path (`check_class` and whatever builds a class's flattened method list, `overridden` flags, and `index` assignment for concrete classes) do the same thing for a user's `ClassKind::Abstract` that it already does for `ClassKind::Class` — verified by reading that actual code path during implementation, not assumed from this description alone, since the native hierarchy was hand-assembled and may have taken shortcuts (e.g., manually setting `overridden: true` on every method, `checker.rs:1289`+) that the general declaration path does not already do automatically for an abstract class specifically.

### D2: An abstract class still produces no layout, no constructor, no allocation — only its method-table entries are needed

Consistent with the existing, unchanged spec text (`zirk-classes`: "no constructor, body, state allocation, or layout contribution"). This change adds exactly one new thing to what a user abstract class produces at lowering: a place in the flattened-method-list/virtual-index scheme its adopters share, so `CallVirtual` has something to index into when the *static* type of a call's receiver is the abstract class. Nothing else about an abstract class's own (non-existent) instance representation changes.

### D3: Multiple adopters of the same abstract class must share method indices consistently

For `CallVirtual`'s `index` to mean the same method across every adopter (the whole point of a shared virtual index), every concrete class implementing a given user abstract class must assign that abstract class's methods the same index in their own flattened method list — this is presumably already how `implements`-based conformance checking works for any abstract-class/contract requirement today (an adopter's own override already needs to line up with what dispatch expects); confirm this holds for a *user* abstract class specifically, the same rigor D1 already calls for, rather than assuming it transfers automatically from the class-inheritance case (`extends`) that `CallVirtual` was originally built to serve.

## Risks / Trade-offs

- **[Risk] The native hierarchy's method-index assignment was hand-written directly into `self.classes` (`checker.rs:1270`+), bypassing whatever general index-assignment logic ordinary `check_class` uses — the two code paths may have silently diverged in some assumption** (e.g., method-index computation order, how multiple abstract-class `implements` targets get reconciled into one flattened list). → Mitigation: this change must trace and test the *general* declaration path independently, not assume it already matches the native hierarchy's hand-built shape just because both use `CallVirtual`.
- **[Risk] Multiple `implements` targets** (an abstract class plus one or more interfaces/traits) each contributing methods to one adopter's flattened list could collide on index assignment if the general path was never exercised with an abstract class specifically in that mix. → Mitigation: a test with a concrete class implementing both a user abstract class and a plain interface, confirming both dispatch correctly.
