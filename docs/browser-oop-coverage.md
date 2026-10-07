# Browser object-model coverage

Evidence for what the browser interpreter runs of Zirk's object-oriented
surface, produced by the `browser-oop-conformance` OpenSpec change. Native
execution (`zirk run`) is the semantic authority; a construct is browser-supported
only when a shared fixture or a native corpus program agrees in both engines.

The machine-readable form is
[`crates/zirk-browser/oop-catalog.json`](../crates/zirk-browser/oop-catalog.json).
`cargo test -p zirk-browser --test corpus_parity` fails when an evidence file is
missing, when a "supported" construct cites a browser exclusion, or when an
"excluded" construct cites a program that now runs. The package manifest
republishes the verdicts under `objectModel`.

## How the audit was run

1. Every valid native end-to-end program (`crates/zirk-cli/tests/corpus/valid`,
   218 programs with a recorded `.out`) runs through the browser `run`
   operation. 184 reproduce the native output exactly; the other 34 fail
   preflight with a capability diagnostic and are listed in
   `EXCLUDED` in `corpus_parity.rs`: concurrency and timers, `parallel`,
   channels, pointers and native slices, `unsafe` journals, `Pin`/`Dependent`,
   resource transfer, and deep `clone()` of records and callables. None of the
   34 is an interpreter defect.
2. The handbook chapters on classes, interfaces and traits were turned into
   probe programs for the constructs the corpus does not exercise. The ones that
   compile natively became the 14 `oop-*` fixtures under
   `crates/zirk-browser/fixtures/`, with expected output taken from native
   execution.

## Construct catalogue

| Construct | Handbook source | Verdict | Evidence |
| --- | --- | --- | --- |
| classes | `docs/handbook/02-handbook/08-classes-and-objects/01-declaring-classes` | supported | fixture `oop-constructors`; corpus `classes` |
| fields-and-initializers | `docs/handbook/02-handbook/08-classes-and-objects/02-fields-and-properties` | supported | fixture `oop-constructors`; corpus `field_default_honored` |
| constructors | `docs/handbook/02-handbook/08-classes-and-objects/03-constructors` | supported | fixture `oop-constructors`; corpus `named_constructor_overload`; corpus `class_construction_sugar`; corpus `constructor_override_default` |
| this | `docs/handbook/02-handbook/08-classes-and-objects/05-this` | supported | fixture `oop-constructors`; corpus `closure_captures_this` |
| visibility | `docs/handbook/02-handbook/08-classes-and-objects/06-visibility` | supported | fixture `oop-visibility-inheritance` |
| inheritance | `docs/handbook/02-handbook/08-classes-and-objects/07-inheritance` | supported | fixture `oop-inheritance-chain`; fixture `oop-subclass-adds-method`; corpus `inheritance`; corpus `subclass_adds_method` |
| final | `docs/handbook/02-handbook/08-classes-and-objects/07-inheritance` | supported | fixture `oop-final-class` |
| abstract-classes | `docs/handbook/02-handbook/08-classes-and-objects/08-abstract-classes` | supported | fixture `oop-polymorphic-collection`; corpus `abstract_class_dynamic_dispatch`; corpus `abstract_class_and_interface` |
| static-members | `docs/handbook/02-handbook/08-classes-and-objects/09-static-members` | supported | fixture `oop-static-mutable`; corpus `static_field`; corpus `static_method_call` |
| object-identity | `docs/handbook/02-handbook/08-classes-and-objects/10-object-identity` | supported | fixture `oop-identity-aliasing`; corpus `identity_and_equality` |
| equality-contract | `docs/handbook/02-handbook/08-classes-and-objects/10-object-identity` | supported | fixture `oop-equality-contract`; corpus `structural_equality` |
| deep-clone | `docs/handbook/02-handbook/08-classes-and-objects/11-cloning` | excluded | corpus `clone_record`; corpus `callable_clone` |
| nested-inner-local-classes | `docs/handbook/02-handbook/08-classes-and-objects/12-nested-classes` | supported | fixture `oop-nested-inner-local` |
| interfaces | `docs/handbook/02-handbook/09-interfaces-and-traits/01-interfaces` | supported | fixture `oop-interface-trait-default`; corpus `contracts`; corpus `diamond_contracts` |
| contract-composition | `docs/handbook/02-handbook/09-interfaces-and-traits/02-implementing-contracts` | supported | corpus `transitive_contracts`; corpus `generic_contract_composition` |
| traits-and-defaults | `docs/handbook/02-handbook/09-interfaces-and-traits/03-traits` | supported | fixture `oop-interface-trait-default`; corpus `inferred_trait_default`; corpus `trait_super_method` |
| generic-classes | `docs/handbook/02-handbook/11-generics/02-generic-types` | supported | fixture `oop-generics-two-params`; corpus `generic_class_dispatch`; corpus `generic_class_specialization`; corpus `generic_contract_dispatch` |
| records-and-enums-with-classes | `docs/handbook/02-handbook/10-data-types/01-records` | supported | fixture `oop-records-enums-in-classes`; corpus `enum_class_payload`; corpus `record_contract_dispatch` |
| polymorphic-collections | `docs/handbook/02-handbook/12-collections` | supported | fixture `oop-polymorphic-collection` |
| exceptions-from-methods | `docs/handbook/02-handbook/15-errors/04-exceptions` | supported | fixture `oop-exceptions-in-methods`; corpus `exceptions_throw_try_catch_finally`; corpus `exceptions_finally_on_return_break` |
| operator-contracts | `docs/handbook/02-handbook/05-operators-and-expressions/11-operator-overloading` | supported | corpus `operator_contracts`; corpus `user_comparison`; corpus `user_iterable` |
| to-string-contract | `docs/handbook/02-handbook/08-classes-and-objects/01-declaring-classes` | supported | corpus `to_string_contract`; corpus `class_without_to_string` |
| weak-references | `docs/handbook/02-handbook/17-memory-and-safety` | excluded | — |

## Result for the interpreter

No divergence between the interpreter and the native pipeline was found in the
supported constructs. The only object-model gap is deep `clone()`, which is
excluded as `InstKind::Clone` and tracked by the `browser-deep-clone` change.
Excluded features now report the Zirk feature name, for example
``deep `clone()` is unavailable in the browser (instruction Clone)``, instead
of an IR debug dump.

## Findings outside the browser interpreter

The audit also exposed defects and gaps that live in the shared frontend or the
native runtime, so both engines are affected or the browser is the correct one.
They are recorded here rather than fixed by this change.

- **Fixed:** a subclass method whose declaration position equalled the position
  of an inherited method was lowered with the inherited method's signature,
  producing `compiler produced invalid IR` for any class that adds a method to
  a base with methods (`subclass_adds_method` corpus program).
- **Native runtime defect (browser is correct):** `Map<String, V>` and
  `Set<String>` compare string keys by handle identity natively, so
  `m.contains_key("k")` is `false` for a key inserted through another `String`
  value with the same content. The interpreter compares by content, as the
  language specifies.
- **Shared defect:** `finally` does not run when `return` executes inside a
  `catch` handler (both engines skip it); the corpus covers `return` inside the
  `try` body only.
- **Shared gap:** a user exception class cannot appear in a class method's
  `throws` clause (`... cannot be thrown`), although the same declaration works
  on a top-level function.
- **Shared gap:** an instance that implements `B`, where `B implements A`, is not
  accepted where an `A` is expected, although the handbook says conformance is
  transitive. `List<Shape>` literals of different subclasses also fail to infer
  (`[Sq(2), Rect(2, 5)]`).
- **Language gap:** a `static` method cannot refer to its own class by name, so a
  `static mut` field cannot be written from inside the class.
