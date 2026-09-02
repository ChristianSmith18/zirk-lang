## 1. Grammar

- [x] 1.1 `MatchExpr` gains `with_binding: Option<Ident>` (`zirk-ast`)
- [x] 1.2 `parse_match` restructured: scrutinee first, `with binding`
      optional after it, before `{` — replaces the guard that emitted
      `NOT_IMPLEMENTED` in the wrong position
- [x] 1.3 Tests: one valid case (`match ... with binding { ... }` parses with
      `with_binding` populated) and confirmation that the absence of `with`
      still yields `None`

## 2. `Resource<E>` as a native contract

- [x] 2.1 `Checker::register_native_resource_contract`: `interface
      Resource<E from Error> { fn close(): Result<Void,E>; fn is_closed():
      Boolean; }`, registered after `register_native_exception_hierarchy`
      (needs `Error` for `E`'s constraint) and after
      `register_native_result_enum` (needs `Result` for `close()`'s return type)
- [x] 2.2 `is_native_contract_name` includes `"Resource"`
- [x] 2.3 `resolve_implements_args`'s own `NOT_LOWERED` gate exempts
      `Resource<E>`: it does not need a specialized dispatch table, since
      `close()`/`is_closed()` are always reached by static dispatch on the
      concrete class, never through a contract-typed reference

## 3. Checker — `match ... with` validation

- [x] 3.1 `Checker::check_resource_match_scrutinee`: the scrutinee must be a
      non-nullable `Result<R,Err>` (`E0438`)
- [x] 3.2 `Checker::check_resource_binding_type`: the arm that destructures
      `with_binding`'s name in its own pattern must have a type that
      implements `Resource<E>` (`E0438`)
- [x] 3.3 No arm destructures `with_binding`'s name: error
      (`E0438`)
- [x] 3.4 New code `codes::INVALID_RESOURCE_MATCH` (`E0438`, next free code
      after `E0437`)

## 4. IR — real automatic close (D1, D2)

- [x] 4.1 `FunctionLowering::pattern_binds`: whether an arm's pattern
      (bare or inside `Variant.bindings`) names `with_binding`
- [x] 4.2 `FunctionLowering::resource_close_block`: synthesizes
      `binding.close();` as a one-statement `ast::Block` — an
      `ast::Expr::Call` over an `ast::Expr::Field`, without going through the
      checker
- [x] 4.3 `lower_match`: the arm that acquires the resource pushes a
      `TryFrame { catches: vec![], finally: Some(<synthetic block>) }`
      before lowering its body and pops it afterward — reuses
      `fase-4b-excepciones`'s `run_finally_through`/`lower_pending_exception_dispatch`
      unchanged for `return`/`break`/`continue`/a propagated exception
- [x] 4.4 Normal completion (the arm does not exit early): runs the synthetic
      `finally` before storing the result and jumping to the continuation
      block, the same pattern `lower_try`'s own post-body check uses

## 5. Pre-existing gap discovered: `Result<Void,E>` in codegen (D4)

- [x] 5.1 `enum_struct`: a `Void` field occupies a zero-sized LLVM struct
      (`{}`) instead of panicking, preserving `EnumLayout.variants`'s
      index numbering
- [x] 5.2 `InstKind::BuildEnum`: skips inserting a value for a `Void`
      field — there is no operand to insert
- [x] 5.3 `InstKind::LoadField`: returns `None` when the result's type
      is `Void`, the same as any other instruction that produces no value
- [x] 5.4 `value_struct`/`object_struct`/closure captures deliberately
      left untouched: the checker never admits a `Void` field or capture
      (`VOID_VARIABLE`), so no program exercises those paths

## 6. Corpus and tests

- [x] 6.1 `crates/zirk-cli/tests/corpus/valid/resources_match_with_closes_on_every_exit.zrk`
      + `.out`: a class implements `Resource<OpenError>`, `match ... with`
      closes on normal completion and on an early `return`
- [x] 6.2 `crates/zirk-cli/tests/corpus/valid/resources_match_with_closes_on_thrown_exception.zrk`
      + `.out`: the same mechanism closes the resource when an exception
      propagates through the arm that acquired it
- [x] 6.3 `crates/zirk-sema/tests/typing.rs`: seven new tests — a class that
      implements `Resource`, `match ... with` accepted (normal completion
      and early `return`), a class that does not implement the full contract
      (`MISSING_IMPLEMENTATION`), and three shapes of `INVALID_RESOURCE_MATCH`
      (no arm destructures the binding, scrutinee is not `Result`,
      binding does not implement `Resource<E>`)
- [x] 6.4 `crates/zirk-parser/tests/grammar.rs`: the `match with` entry
      is removed from the list of "later-phase constructs" (it is now
      implemented); `valid_match_with_states_its_phase` is replaced
      by two real grammar tests

## 7. Closeout

- [x] 7.1 `cargo test --workspace` (799 tests), `cargo clippy --workspace
      --all-targets`, `cargo fmt --check` all green
- [x] 7.2 Manual verification with `zirk run` on both corpus programs,
      confirming the exact output order (including the exception
      caught after the close)
- [x] 7.3 `openspec/specs/zirk-resources/spec.md`: `MODIFIED
      Requirement` delta narrowing the first of its five sections to what this
      change actually implements
