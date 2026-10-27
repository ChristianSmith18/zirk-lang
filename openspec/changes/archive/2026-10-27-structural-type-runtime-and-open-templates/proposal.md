## Why

Zirk already has the semantic foundations for unions, intersections, structural types, literal types, narrowing, and finite template-literal expansion. The remaining gaps are concentrated at two boundaries: some structural combinations accepted by the type system still lack complete portable-IR/native execution, and template types cannot express an open pattern whose generic key is inferred back from a string argument. Closing these gaps would make accepted structural programs execute consistently and let generic APIs relate event names to the property types they describe.

## What Changes

- Complete the documented behavior of structural unions and intersections from type checking through portable IR, interpreter, native code generation, package ABI, and target-specific validation. Preserve existing support for basic discriminated structural unions and extend it to the remaining supported structural and mixed-representation cases.
- Specify the full static behavior of structural unions and intersections: canonical N-ary normalization, contextual object-literal selection, common-property reads and safe writes, callable-member compatibility, intersection views, control-flow narrowing, and exhaustive matching. Diagnostics must distinguish unsupported source programs from compiler/backend failures.
- Extend template-literal types from finite expansion to open patterns over constrained generic parameters. For the `Watched<T>` contract example, infer method parameter `K = "age"` from `person.on("ageChanged", ...)`, validate `K` against `String & keyof T`, resolve `T[K]`, and contextually type the callback as the selected property type. The first version supports one uniquely recoverable interpolation with fixed surrounding text; it does not enumerate possible strings.
- Preserve method-local generic parameters and constraints on parameterized contract signatures. A call through `Watched<Person>` first substitutes the contract's `T`, then infers the method's `K`; the selected implementation is specialized at compile time for that substitution, with no runtime generic type value.
- Keep open-template inference compile-time-only and deterministic. It must not enumerate an unbounded string domain or alter runtime string behavior. Existing finite expansion and its resource limits remain intact.
- Align CLI, interpreter, native compiler, LSP diagnostics/hover/completion, handbook examples, implementation-status documents, and the companion website with the finalized behavior. Keep the accepted literal syntax unchanged, including the existing class-origin forms for date/time literal types.

The scope is limited to completing executable structural composites and adding the open event-name template inference described above. It does not add overload resolution, user-declared type predicates, mapped or conditional types, arbitrary runtime string patterns, or a general TypeScript compatibility layer. It does not introduce a new template syntax or change existing literal syntax/type operators except where needed to compose open templates with them.

The intended first-delivery Zirk form mirrors the `Watched<T>` API with a method-generic contract signature and Zirk's existing `{Type}` interpolation notation inside a type string:

```zirk
import { stdout } from std.io;

contract Watched<T> {
    fn on<K from String & keyof T>(
        event: "{K}Changed",
        callback: Fn(T[K]) => Void
    ): Void;
}

type Person = { age: Int32; name: String };

fn register(person: Watched<Person>): Void {
    person.on("ageChanged", (age): Void => {
        stdout.println(age.to_string());
    }); // K is "age"; age is Int32
}
```

The receiver supplies `T = Person`; the event singleton then infers the method-local `K` as `"age"`, and the callback parameter resolves to `Int32`. Generic contract-method implementation conformance and call-site specialization are part of this scope so this API is usable through a contract-typed receiver. The implementation must retain the existing no-overloading rule and must not require a runtime `K` descriptor.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `zirk-data-types`: specify complete N-ary structural union/intersection value semantics and contextual construction.
- `zirk-type-system`: complete structural relation solving, common-member behavior, narrowing, and intersection views.
- `zirk-ir-lowering`: ensure every semantically accepted structural composite has verified portable-IR operations and an explicit representation path.
- `zirk-native-codegen`: complete target-correct native layout, ABI, payload tracing, and dispatch for structural and mixed unions; preserve intersection identity.
- `zirk-ir-interpreter`: match native composite-value behavior, including tags, projections, managed references, and failures.
- `zirk-generics`: add open template patterns, inverse inference from literal arguments, method-local generic constraints on parameterized contract methods, and indexed-property/callback contextual typing.
- `zirk-grammar`: specify how open template types reuse Zirk's existing interpolated type-string syntax and reject malformed pattern forms.
- `zirk-lsp`: report the same inference and structural-type results as the compiler and surface focused diagnostics and type information.

## Impact

This changes compiler semantics and code generation across `zirk-sema`, `zirk-ir`, `zirk-ir-interpreter`, `zirk-codegen-llvm`, CLI/package boundaries, and `zirk-lsp`. Open templates and method-local generic substitutions add no runtime type values; contract dispatch must select or generate the implementation specialized for the checked method substitution at compile time. Structural composite support may affect internal IR layout and package ABI; generated packages must carry or validate the representation metadata needed to prevent mismatched callers.

Implementation is staged so the structural support matrix and observable rules are fixed first, followed by semantic checking and IR verification, interpreter parity, native representation/ABI support, and finally open-template inference and editor integration. Each accepted structural type must either have a valid executable path on a supported target or receive a source-located diagnostic before backend emission. Native inline storage and any required managed/boxed fallback must preserve the same source semantics and GC safety.

Validation must include semantic and lowering tests, generic contract-method conformance and dispatch tests, interpreter/native differential cases, package-boundary calls, focused platform CI for supported targets, and resource-budget tests for recursive types, normalization, template matching, and generic inference. Public handbook/status changes also affect the sibling `../zirk-lang-site`; synchronize it after the relevant repository changes are committed and reviewed, following the repository's website-sync instructions.
