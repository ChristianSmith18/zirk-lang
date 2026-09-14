## Why

`to_string()` today is partial and asymmetric: `List(1, 2).to_string()` compiles
but `println(list)` and `"items: {list}"` are rejected; `Map`, `Set`, `Range`,
`Regex`, `Job`, `Channel`, `Pointer`, `NativeSlice`, `Dependent`, `Pin`,
nullable types and `null` have no text form at all; and the default rendering
leaks internal names (`Ejemplo$local$24`, hidden `outer` fields). The language
also lacks a multiline string literal: `'''` is specified nowhere, so
long-form text forces escaped `"…"` chains. This change makes every value
answer `to_string()`, unifies printing and interpolation behind it, and adds a
triple-quoted literal whose left edge is visible both to the compiler and to
the editor.

## What Changes

- **`to_string()` becomes universal**: every type that produces a value answers
  `to_string(): String`. Only `Void` and `Never` are excluded.
- **New renderings** (decided semantics):
  - `null` and an absent `T?` → `<Null>`; a present `T?` renders its inner
    value. `x.to_string()` on a nullable receiver is accepted.
  - `Range<T>` → mirrors its literal: `1..10`, `1..=10`, `1..10:2`,
    `1..=10:2`.
  - `Map<K,V>` → `{k: v, …}` and `Set<T>` → `{v, …}`, with elements ordered by
    the natural order of the key/element type (numeric for `IntN`/`FloatN`,
    lexicographic for `String`, rendered text otherwise) so the output is
    deterministic.
  - `Regex` → `re'pattern'` (the existing `zirk_regex_to_string` runtime
    entry point is finally wired to a member).
  - Named function → `<Function name>`; anonymous lambda → `<Function>`.
  - `Job`, `Channel`, `Pointer`, `NativeSlice`, `NativeSliceMut`, `Dependent`,
    `Pin` → `<Job>`, `<Channel>`, `<Pointer>`, `<NativeSlice>`,
    `<NativeSliceMut>`, `<Dependent>`, `<Pin>` — extending the existing
    `<Weak>`/`<Callable>` convention.
  - Algebraic enum variants render **with payload**:
    `Circulo(radio: 5)`, `Ok(5)`, `Error("x")`.
  - The default `class`/`record`/`enum` rendering uses the **surface name**:
    the `$local$N` mangling is stripped (`Ejemplo(…)`, not
    `Ejemplo$local$24(…)`).
- **Printing and interpolation are unified**: `println`, `print` and `{expr}`
  interpolation accept any non-`Void`/`Never` value and route through
  `to_string()`. The `is_printable` closed list disappears.
- **Triple-quoted string literals `'''…'''`**: literal text preserving
  newlines, tabs and spacing; `{expr}` interpolation and the existing escape
  set stay active. The column of the opening `'''` defines the literal's left
  edge: every content line must be indented at least to that column (that
  prefix is stripped), and a line starting left of the edge is a compile-time
  diagnostic.
- **Canonical formatter**: `zirk format` (ZIRK_COMPILER_SPEC.md section 10)
  lands its first rule — a `'''` literal is placed on the line after the
  `=`/`(`/`name:` that introduces it, one indent deeper, with opening and
  closing `'''` aligned on the same column.
- **VS Code extension**: the grammar scopes `'''…'''` (including nested
  interpolation), and the opening/closing `'''` get a column-level visual
  guide (bracket-pair guide and/or a decoration drawn by `extension.js`).
- **Documentation**: handbook + language/stdlib/compiler spec updates
  describing the universal `to_string` contract and the `'''` semantics.

## Capabilities

### New Capabilities

- `universal-string-rendering`: the complete `to_string()` render table — one
  owner for what every type prints, the `<TypeName>` convention for opaque
  handles, `<Null>` for absent values, deterministic collection rendering,
  and the surface-name rule for local/nested types.
- `triple-quoted-strings`: the `'''…'''` literal — lexing, the left-edge
  rule, interpolation, escapes, and the canonical formatter layout rule.
- `canonical-formatter`: the `zirk format` subcommand's first delivered slice
  (canonical, idempotent; initially covering the `'''` layout rule).

### Modified Capabilities

- `zirk-type-system`: the `to_string()` contract becomes universal
  (nullable and previously unprintable types gain a text form); string
  interpolation accepts any printable value.
- `zirk-lexical-syntax`: the string-literal and interpolation requirements
  gain the `'''` form.
- `zirk-data-types`: the universal-rendering requirement gains enum payloads
  and surface names for local/nested classes.
- `zirk-collections`: the rendering requirement extends to `Map`, `Set` and
  `Range`, and collections become printable in `println`/interpolation.
- `zirk-regex`: `Regex` gains `to_string()` returning `re'pattern'`.
- `zirk-callables`: callable rendering distinguishes named functions
  (`<Function name>`) from anonymous callables (`<Function>`).
- `zirk-cli-commands`: the command surface gains `zirk format`.
- `editor-tooling`: language configuration and grammar cover `'''`; the
  extension's provisional formatter delegates `'''` layout to `zirk format`;
  the open/close delimiter guide is specified.

## Impact

- **Code**: `zirk-lexer` (new token), `zirk-sema` (`is_printable`,
  `require_printable`, member tables, nullable receivers), `zirk-ir`
  (`lower_to_string` arms), `zirk-runtime` (Map/Set iteration, regex wire),
  `zirk-cli` (`format` subcommand), `editors/vscode` (grammar,
  language-configuration, `extension.js` decorations).
- **Docs**: `docs/ZIRK_LANGUAGE_SPEC.md`, `docs/ZIRK_STDLIB_SPEC.md` section
  3, `docs/ZIRK_COMPILER_SPEC.md` section 10, handbook pages for strings and
  conversions, plus a dedicated page explaining the `'''` left-edge rule.
- **Public surface**: yes — normative semantics, public documentation, syntax
  examples and feature-status wording change. The companion repository
  `../zirk-lang-site` must be updated in the same workstream via
  `./scripts/sync-website-content.sh` once the changes are committed.
- **Tests**: new corpus cases under `crates/zirk-cli/tests/corpus/`, unit
  tests in `zirk-lexer`/`zirk-sema`/`zirk-ir`/`zirk-runtime`, and extension
  JSON validation per AGENTS.md.
