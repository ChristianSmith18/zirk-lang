## Why

Writing a Zirk class today repeats every attribute three times: the field
declaration, the constructor parameter, and the `this.x = x` assignment. Two
small pieces of sugar — TypeScript-style promoted constructor parameters and
an implicit default constructor — remove that boilerplate without changing
the object model underneath.

## What Changes

- A `construct` parameter MAY carry member modifiers
  (`public`/`private`/`protected`, `mut`/`inmut`/`inmut::strict`, in
  combination). A modified parameter is *promoted*: it declares the
  equivalent attribute and assigns it at the start of the constructor body,
  so `construct(public name: String)` is the exact equivalent of declaring
  `name: String;` plus `this.name = name;`.
- Promotion works with optional parameters (`name?: Type`) and default
  values (`name: Type = expr`). A parameter without any explicit modifier
  stays an ordinary parameter — the sugar is never implicit.
- A class that declares no `construct` member gains an implicit
  `construct() {}`, so `Foo()` instantiates it. Declaring any `construct`
  removes the implicit one, matching the usual Java/TypeScript rule.
- **BREAKING**: a class without constructors that previously failed with
  "`X` has no constructor" now instantiates through the implicit
  constructor; `Foo(1)` on such a class instead reports that no constructor
  takes one argument.
- New errors: a promoted parameter colliding with a declared field;
  promotion used in a class that declares more than one `construct`
  (ambiguous which constructor owns the fields); promotion outside a class
  constructor; a promoted variadic parameter.
- New warnings: reading or writing a promoted field from a method other
  than its declaring constructor (legibility hint to declare the field
  explicitly), and the implicit `super()` call a synthesized subclass
  constructor performs (hint to write the constructor explicitly).
- Sugar stays sugar for tooling: the implicit constructor is invisible in
  symbol lists and outlines, and `zirk format` never expands either form.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `zirk-grammar`: `construct` parameter lists accept member modifiers as a
  promotion marker; the same modifiers stay rejected on every other
  parameter position.
- `zirk-classes`: construction semantics gain promoted parameters and the
  implicit default constructor, including the subclass `super()` behavior
  and the new diagnostics.

## Impact

- `zirk-parser` / `zirk-ast`: accept member modifiers in `construct`
  parameter position only; record promotion on the parameter node.
- `zirk-sema`: register promoted fields as ordinary members, emit the new
  errors and warnings, synthesize the implicit constructor.
- `zirk-ir` lowering / `zirk-codegen-llvm`: promoted assignments and the
  implicit constructor lower as ordinary code; no IR change expected.
- `zirk-interpreter` / `zirk-browser`: semantics are transparent after
  lowering; the differential fixture harness gains sugar fixtures.
- `zirk-lsp` and the VS Code extension: promoted fields appear as fields;
  the implicit constructor stays invisible.
- Public documentation: `docs/ZIRK_LANGUAGE_SPEC.md` section 7, the
  handbook constructors/instantiation chapters, feature status, and the
  roadmap — so `../zirk-lang-site` needs the usual companion sync.
