# Zirk application globals — design notes

**Status:** historical discussion checkpoint; the final contract is recorded below and in the linked OpenSpec/handbook sources.

**Date:** 2026-10-24

## Goal

Provide application-wide, named, typed state declared in `.zkinit`, with compiler-checked access through `Global`. Updates use replacement values and can be observed reactively. The first version stays small: `Global.get`, `Global.set`, and `Global.watch`.

## Manifest declarations

Only an `application` declares globals. A library/package cannot declare application globals.

Each declaration has a key and a type. The shorthand uses the type's default value:

```text
globals {
    request_count: UInt64;
}
```

An expanded object adds optional configuration:

```text
globals {
    settings: {
        type: Settings;
        from: "./src/config.zrk";
        readonly: true;
    }

    retry_limit: {
        type: UInt32;
        default: 0;
    }
}
```

Rules:

- The key is the global's name. Lowercase names are allowed; uppercase is not required.
- `type` is required in the expanded form.
- `from` is optional. When present, it identifies the Zirk source file that declares a custom type. Its path is relative to the project root. Without it, ordinary package/module type resolution applies.
- `default` is optional. When omitted, Zirk derives the value from the type's default construction.
- `readonly` is optional and defaults to `false`. When `true`, it prevents replacing the global through `Global.set`; the value remains readable through `Global.get`.
- An explicit `default` must type-check against the declared type.

### Default construction

For a custom type, default construction uses defaults declared by its constructor parameters. A parameter with its own default uses that value; otherwise Zirk uses the default value of that parameter's type. For example, a `Person(name: String)` with no explicit `name` default can be default-constructed with the default `String` value (empty string).

The compiler must reject declarations where the type cannot be resolved or a default cannot be derived. Initializer/default construction must have deterministic semantics. Whether user constructor bodies may run during global initialization, or whether only compiler-verifiable default construction is allowed, remains to be specified.

## Typed access and inference

The compiler resolves a statically known global key against `.zkinit` and infers its declared type:

```zirk
let settings = Global.get("settings"); // inferred as Settings
Global.set("settings", newSettings);   // requires Settings
```

No caller-supplied generic type may override the manifest declaration. Unknown keys, incompatible `set` values, unresolved custom types, and writes to a `readonly` global are compile-time diagnostics whenever the key is statically known.

For the first version, require literal global keys so the compiler can perform this lookup. Dynamic keys would lose this guarantee and need a separate design.

## Reactive state model

Global state follows immutable replacement semantics, inspired by Redux:

- `Global.set` replaces the stored value with a new value.
- In-place mutation must not silently alter globally stored state or bypass change notification. The value obtained from `Global.get` must therefore not expose a mutable alias that can change the stored object without a `set`.
- `readonly` controls whether the global slot may be replaced through `set`; it does not prevent reading with `get`.
- A successful replacement that is equal to the current value does not count as a change and emits no event.

The exact value equality contract must be deterministic. Watched types need an equality implementation (or compiler-derived structural equality when valid). Equality compares the values' structure and contents, not just object identity. Types that cannot be compared produce a compiler diagnostic when watched.

## `Global.watch`

`Global.watch<T>(key)` infers `T` from `.zkinit` and returns a managed subscription/stream of:

```text
GlobalChange<T> {
    previous: T;
    current: T;
    revision: UInt64;
}
```

The stream emits only after a real value change. It does not emit an initial event, because that is not a change. To avoid a race between reading the initial value and subscribing, the watch handle exposes a snapshot captured atomically when the subscription is registered; later emissions contain changes after that revision.

Delivery rules:

- Updates to each global are committed in a total, increasing revision order.
- Events are delivered in that order and outside the internal state lock; observer code never runs while the global is locked.
- A subscription is a managed resource and can be closed explicitly; leaving its owning scope closes it as well.
- Delivery is bounded so a slow observer cannot cause unbounded memory growth. A lag/overflow condition must be explicit, never a silent drop; the observer can recover by reading a fresh snapshot and resuming from its revision.
- If the implementation promises delivery of every transition, producer backpressure or an explicit lag error is required. It must not coalesce intermediate transitions while claiming lossless delivery.

The exact stream iteration syntax follows the standard-library stream design; this note does not prescribe unsupported syntax.

## Initialization and failures

Globals are initialized before `main` in a deterministic order. Failures that can be determined from declarations—unknown type, invalid `from`, incompatible default, missing parameter default, or invalid `readonly`/`set`—are reported by the compiler with a global-specific diagnostic.

If initialization can execute user code and fail dynamically, its failure cannot always be known during compilation. The design should avoid arbitrary side effects in manifest initialization or define a deterministic startup diagnostic for such runtime failures. This boundary remains open.

## Compiler/runtime/editor responsibilities

- Parse shorthand and expanded global declarations from `.zkinit`.
- Resolve custom type identity through `from` and normal package/module rules.
- Build the project global symbol table before checking source files.
- Infer and validate `Global.get`, `Global.set`, and `Global.watch` using literal keys.
- Enforce `readonly` and equality requirements statically where possible.
- Generate typed storage, deterministic initialization, snapshot/revision tracking, and managed subscriptions.
- Integrate compiler diagnostics, LSP completions/quick fixes, and VS Code manifest highlighting/actions.

## Decisions still open before a normative proposal

- Exact `.zkinit` grammar, separators, and whether object entries can use shorthand and expanded syntax in the same block.
- Whether custom constructors execute during global initialization or only pure/compiler-verifiable default construction is allowed.
- Exact structural equality contract and support for recursive/reference-containing values.
- Whether `get` returns a deep value, immutable view, or another representation that prevents unobserved in-place mutation.
- Exact public subscription API, stream ownership, cleanup syntax, and recovery after a subscriber falls behind.
- Backpressure behavior and whether `set` can suspend, fail with a typed capacity error, or publish to a shared revision log.
- Whether a global's initializer may refer to another global and, if so, how dependency ordering and cycles are diagnosed.

## Current repository status

The language and runtime specifications describe application globals and deterministic initialization, but the `.zkinit` project parser does not currently parse a `globals` block. The current semantic checker treats `use` as enabling imported names; it is not yet wired to globals declared in the manifest. This design is not implemented end to end.

## Finalized model

This discussion checkpoint is superseded by `openspec/changes/complete-zkinit-project-runtime/specs/zirk-globals/spec.md` and the [globals handbook](docs/handbook/03-projects/06-globals.md). Applications declare typed keys in `.zkinit`; packages cannot. Shorthand uses the type default. Expanded entries require `type` and may set `default`, project-relative custom-type `from`, or `readonly` (default `false`). Custom constructor parameters use their explicit defaults or their type defaults. `Global.get`, `Global.set`, and `Global.watch` infer the value type from a literal manifest key. Readonly blocks only `set`. Replacement snapshots emit only for structurally unequal values. Watch captures an atomic snapshot, emits previous/current/revision for real changes, uses bounded ordered delivery, and reports lag/closed state. Global initialization happens before imported modules and the selected entry.
