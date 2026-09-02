## 1. Lexicon and grammar

- [x] 1.1 New `throw`/`throws` keywords; graduate `try`/`catch`/`finally` from reserved to implemented
- [x] 1.2 `throw expr;` / `throw;` (rethrow, valid only inside a `catch` — validated by the checker, not the parser)
- [x] 1.3 `try { } catch Type(name) { } ... finally { }`, requiring at least one `catch` or one `finally` (`E0313`)
- [x] 1.4 `throws Type (| Type)*` on a function or method signature, reusing `parse_type`'s own union syntax
- [x] 1.5 Tests: one valid and one invalid case per new rule

## 2. `Error`/`Throwable`/`RuntimeError`/`StackTrace` hierarchy

- [x] 2.1 Register the three as `abstract class`, the same "inject the tables directly" mechanism as `Result`/`Iteration<T>` (D4)
- [x] 2.2 `StackTrace` as a minimal concrete class, built structurally (with no real `program.classes` entry, D4)
- [x] 2.3 Fix `declare_class` to seed `methods` from `self.native_exceptions` before its own, aligning indices for virtual dispatch (D4)
- [x] 2.4 Fix `CallVirtual` in `zirk-codegen-llvm` to build the indirect call's signature from the instruction's own return type, not from `self.functions[&layout.methods[index]]` (D4)
- [x] 2.5 Extend an `ObjectLayout`'s `ancestors` to include `abstract_bases` transitively (roadmap Phase 4b), needed so `catch Throwable(e)` recognizes at runtime a class that only implements `RuntimeError`

## 3. Checker — validation and effect analysis

- [x] 3.1 `Checker::implements_abstract_class`: a transitive walk over `abstract_bases`, separate from `is_subclass_of`'s own non-transitive check (see Risks in `design.md`)
- [x] 3.2 `is_throwable_type`/`resolve_throws_clause`: validates that `throw`/`throws`/`catch` name a type that implements `Throwable`
- [x] 3.3 `pending_throws`/`current_throws`: "catch or declare" analysis — every `throw`, rethrow, or call to a `throws` function is accumulated and checked against the enclosing function's own `throws` (`E0434`)
- [x] 3.4 `catch` ordering: a `catch` already covered by an earlier one is unreachable (`E0436`)
- [x] 3.5 `throw;` outside a `catch` is an error (`E0437`)
- [x] 3.6 `return`/`break`/`continue`/`throw` directly inside a `finally` is an error (`E0435`) — a stricter rule than the spec's own ("only when it would replace an active outcome"), deliberately (D3)
- [x] 3.7 `check_try` participates in "every path returns" analysis (the body and every `catch` must return)
- [x] 3.8 Tests: one valid and one invalid case per new rule

## 4. IR and real execution (D1, D2, D3)

- [x] 4.1 Runtime (`zirk-runtime`): `zirk_rt_throw`/`zirk_rt_has_pending_exception`/`zirk_rt_take_pending_exception` over a `thread_local` slot, plus `zirk_rt_is_instance` (boolean variant of `zirk_rt_check_cast`) and `zirk_rt_uncaught_exception`
- [x] 4.2 IR: `InstKind::Throw`/`HasPendingException`/`TakePendingException`/`IsInstance`/`Undefined`, with their verification in `verify.rs` and codegen in `emit.rs`
- [x] 4.3 `Lowering::try_stack`: each `catch` builds its handler block and binding slot before the `try`'s body is lowered (D2)
- [x] 4.4 `lower_throw`/`lower_pending_exception_dispatch`: takes the pending exception, tests each active `catch` from innermost to outermost, jumps to the first match or re-propagates after running every `finally` along the way (D1/D3)
- [x] 4.5 `lower_throws_check`: after every call whose target declares `throws`, checks for a pending exception — wired at the four call sites (direct function, method, contract method, and their statement-position counterparts)
- [x] 4.6 `opens_blocks` did not need extending for `throws` calls in statement position (already covered by `lower_expr_for_effect`'s own wiring), but the `lower_throw`/`lower_throws_check` checkpoint itself did surface and require fixing a real bug: `lower_throw` was unconditionally returning from the function instead of consulting `try_stack` first
- [x] 4.7 `emit_c_entrypoint`: after `main`, checks for a pending exception and aborts via `zirk_rt_uncaught_exception` if it is still active (an uncaught exception in `main` exits with a nonzero code)
- [x] 4.8 Tests: manually verified scenarios (catch by concrete type, catch by `Throwable`, rethrow through a `throws` function, uncaught propagation), valid and invalid corpus

## 5. Closeout

- [x] 5.1 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --check` all green
- [x] 5.2 `design.md`: `## Decisions` section completed (D1–D4) after replacing the original propagation mechanism (invisible return value) with the actually implemented `thread_local` slot
- [x] 5.3 Fixed regressions in `zirk-ir/tests/lowering.rs`: tests that assumed `objects[0]`/`Object(0)` for the first user class now use `Module::object_id` by name, since the four native classes are registered first
- [x] 5.4 Fixed regression in `zirk-parser/tests/grammar.rs`: `try`/`catch`/`finally` are no longer "later phase"; the `UNEXPECTED_TOKEN` code that the empty `try` itself used was replaced with a dedicated `E0313` (`EMPTY_TRY`)
