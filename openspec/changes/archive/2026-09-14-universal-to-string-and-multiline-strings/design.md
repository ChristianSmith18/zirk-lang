## Context

`to_string()` is implemented as a single lowering dispatch —
`Lowerer::lower_to_string`/`lower_to_string_expr` in
`crates/zirk-ir/src/lower.rs` — gated upstream by `Checker::is_printable` and
`require_printable` in `crates/zirk-sema/src/checker.rs`. Native scalars emit
`InstKind::ToString` (dispatched by width/signedness in
`zirk-codegen-llvm/src/emit.rs` to `zirk_str_from_*` runtime functions);
`Array`/`List` lower to a string-building loop; tuples, records, classes and
enums get a compiler-generated structural rendering; `Weak` and callables
answer constant strings (`<Weak>`, `<Callable>`). Everything else —
`Map`, `Set`, `Range`, `Regex`, `Job`, `Channel`, `Pointer`, `NativeSlice`,
`NativeSliceMut`, `Dependent`, `Pin`, nullable types and `null` — is rejected
at check time.

Separately, the lexer only knows `"…"` strings and `'…'` chars/regexes. There
is no multiline literal, and `ZIRK_COMPILER_SPEC.md` section 10 promises a
canonical `zirk format` that does not exist yet (the CLI only ships `check`,
`build`, `run`).

## Goals / Non-Goals

**Goals:**

- Every value-producing type answers `to_string(): String`; `Void` and
  `Never` remain the only non-printable types.
- `println`, `print` and `{expr}` interpolation all route through
  `to_string()` — the `is_printable` closed list is removed.
- Deterministic, spec'd renderings for every type, including `<Null>` for
  absent values, `<Function name>` for named functions, enum payloads,
  surface names for local/nested classes, and literal-mirroring `Range`
  output.
- A `'''…'''` multiline literal with an enforced left-edge rule, plus the
  first slice of `zirk format` to normalize its layout.
- VS Code support: grammar scope for `'''` and a column-level visual guide
  joining the delimiters.

**Non-Goals:**

- A general-purpose formatter: `zirk format` ships only the `'''` layout rule
  in this change.
- Reflection (`type_name`, field enumeration) — deliberately separate from
  `to_string`.
- `to_debug()`/`repr` — noted as a possible follow-up, not delivered here.
- Changing `Map`/`Set` backing structures (iteration order stays arbitrary;
  rendering sorts at display time).
- Union types (`A | B` still does not compile; their rendering is specified
  for when they do).

## Decisions

### D1 — One render contract, one spec owner

The complete render table lives in a new `universal-string-rendering`
capability spec instead of scattering rows across per-type specs. Existing
specs whose *requirements* change (`zirk-type-system`, `zirk-collections`,
`zirk-data-types`, `zirk-lexical-syntax`, `zirk-regex`, `zirk-callables`,
`editor-tooling`, `zirk-cli-commands`) get deltas. This mirrors the "one
synchronized capability owner" rule already used by `zirk-standard-library`.

### D2 — `<Null>`, not `null`, for absent values

`x.to_string()` on a `T?` is accepted directly: a present value renders its
inner value, an absent one renders `<Null>`. `null.to_string()` renders
`<Null>` too. The `<…>` spelling keeps the language's convention that angle
brackets mark "not ordinary data" and makes `["null", null]` distinguishable
(`[null, <Null>]`).

*Alternative considered:* bare `null` (Kotlin/Java/JS convention). Rejected
by the language owner because a bare `null` collides visually with the
`String` `"null"` inside structural renderings, where strings are unquoted.

*Lowering:* nullable operands branch on the presence flag — `IsNull`-style
test, `const_string("<Null>")` on the absent arm, `lower_to_string` on the
inner value otherwise.

### D3 — Deterministic collection rendering via sorted output

`Map`/`Set` are `HashMap`/`HashSet`-backed, so iteration order is unstable.
Rendering iterates entries, converts each key/element (and value) through
`lower_to_string`, collects them, and emits them sorted: natural order when
the key/element type has one (`IntN`, `UIntN`, `Decimal`, `FloatN`,
`String`, `Char`, `Date`, `Time`, `DateTime`, `Duration`), rendered-text
order otherwise. The sort runs in lowered IR code (or a small runtime
helper); no backing-store change.

*Alternative considered:* insertion-ordered maps. Rejected for this change —
it is a runtime data-structure migration, not a rendering concern.

### D4 — Enum payloads render structurally

`Circulo(radio: 5)`, `Ok(5)`, `Error("boom")`. `lower_enum_to_string` extends
its discriminant switch so each arm builds `Variant(` + `field: rendered`
pairs via the same `lower_default_render_to_string` machinery records use.
Traditional enums are unchanged (`Color.Rojo` → `Rojo`).

### D5 — Surface names in structural rendering

`lower_default_render_to_string` strips the `$local$N` checker mangling
(`Ejemplo$local$24` → `Ejemplo`) and keeps `Outer.Inner` qualification, which
*is* the surface spelling for nested classes. The `inner class` hidden
`outer` field is omitted from the field list — it is representation, not
user data.

### D6 — `<Function name>` / `<Function>`

A value whose type is `Base::Function` resolves its symbol name at lowering:
named function → `const_string("<Function name>")`; anonymous lambda →
`<Function>`. This replaces `<Callable>` for callables produced through the
`Callable` representation.

### D7 — Opaque handles extend the `<…>` convention

`Job`, `Channel`, `Pointer`, `NativeSlice`, `NativeSliceMut`, `Dependent`,
`Pin` each answer a constant `<TypeName>` string — one `const_string` arm
each in `lower_to_string`, plus `to_string` entries in the checker's member
tables. `Regex` is the exception: it has content, so `re.to_string()` calls
the already-implemented `zirk_regex_to_string` (returns the pattern) wrapped
as `re'pattern'` via `Concat`.

### D8 — `'''` lexing reuses the string pipeline

The lexer gains a `'''`-delimited literal producing the same
`TokenKind::Str`/`InterpolatedStr` shapes as `"…"` (with `StrPart` parts), so
parser, checker and lowering need no new expression form — a `'''` literal
*is* a `String`. Lexer rules:

- `'` and `''` inside content are legal; the first `'''` run closes.
- Content is literal: physical newlines/tabs/spaces are preserved.
- Escapes (`\n`, `\t`, `\r`, `\0`, `\\`, `\{`, `\}`, `\'`) stay active —
  consistent with `"…"`, and the only way to write a literal `{`.
- `{expr}` interpolation reuses `Lexer::interpolation` unchanged (balanced
  braces already handled).

### D9 — The left edge is a compile-time rule, not just a style rule

The column of the opening `'''` is the literal's left edge:

- The newline immediately after the opening `'''` is not part of the value.
- Every non-empty content line must begin at or right of the edge column;
  exactly edge-width leading whitespace is stripped per line.
- A content line starting left of the edge is a lexer/parser diagnostic
  (new error code, e.g. `STRING_CONTENT_LEFT_OF_EDGE`) pointing at the
  offending line, with help naming the edge column.
- Empty lines contribute `\n` and carry no indentation requirement.
- The closing `'''` must sit on its own line, at or right of the edge; its
  line contributes nothing.
- The edge is measured in **characters**, not display cells — a tab counts
  as one character (predictable, locale-independent).
- Quote runs: `''''''` is the empty literal; inside content, `'`/`''` are
  literal and the first `'''` closes.

*Alternative considered:* relative-edge rules à la Swift (closing delimiter
sets the edge). Rejected — the *opening* delimiter is what the formatter and
the editor guide anchor to, and it reads top-down.

### D10 — `zirk format` ships minimal but canonical

New `zirk format <file>` subcommand. Its first rule: a `'''` literal is
rewritten so the opening `'''` sits on the line after the `=`/`(`/`name:`
that introduces it, one indentation level deeper, and the closing `'''`
aligns to the same column. Idempotent by construction (re-formatting is a
no-op), no configuration. `--check` reports files needing formatting without
writing (CI-friendly). Everything else in §10 remains future work.

### D11 — Editor guide via decoration, grammar via TextMate

- `syntaxes/zirk.tmLanguage.json`: a `string.quoted.triple.zirk` rule with
  `begin: '''` / `end: '''`, embedding the existing interpolation patterns so
  `{expr}` highlights inside.
- The column guide: a `TextEditorDecorationType` in `extension.js` that scans
  the visible document for `'''` pairs and paints a `border-left` on each
  covered line at the delimiter column — always-on and independent of user
  settings, like Python indent guides.
- `language-configuration.json`: optionally `["'''", "'''"]` added to
  `brackets` for built-in pair matching; evaluated during implementation
  (possible interference with the `'` char/auto-closing pair is a known
  risk — the decoration alone already delivers the required guide).
- Per AGENTS.md, the extension is validated and copied to
  `~/.devin/extensions/christiansmith.zirk-lang-0.4.2/`.

## Risks / Trade-offs

- **[Spec drift]** `zirk-type-system`'s current contract says a class without
  `to_string` in `println` is a diagnostic — implementation already renders
  structurally. The delta rewrites that requirement to the universal
  contract, keeping spec and code honest.
- **[Corpus churn]** Enum payload rendering changes existing output
  (`Shape.Circle(5)` prints `Circle` today → `Circle(radius: 5)`). Affected
  `.out` files (e.g. `tuple_enum_and_rendering.out`) must be regenerated.
- **[Recursive structures]** A class whose fields reach itself renders
  forever today; payloads make this reachable for enums too. Mitigation:
  documented limitation; a cycle/depth guard is follow-up work.
- **[Map/Set render cost]** Sorting makes rendering O(n log n) with
  allocation. Acceptable for a display operation; documented in the spec.
- **[Edge rule vs. existing code]** No existing `.zrk` file can contain
  `'''`, so no source breakage; char literals `'…'` keep maximal-munch
  priority where they appear.
- **[`zirk format` scope creep]** Shipping only the `'''` rule keeps the
  first formatter honest; §10's full formatter stays explicitly future work.
- **[Bracket-pair interference]** `["'''", "'''"]` may misfire against `'`
  auto-closing and char literals. Mitigation: the decoration guide is the
  primary mechanism; the brackets entry ships only if it proves harmless in
  validation.

## Migration Plan

No user-code migration: everything added is previously-rejected syntax or
previously-failing calls. Output-visible changes (enum payloads, local-class
surface names) are rendering fixes — documented in the handbook page and the
change summary. The website sync (`./scripts/sync-website-content.sh`) runs
after docs land on a clean tree.
