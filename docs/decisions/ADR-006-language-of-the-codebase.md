# ADR-006 — The project is written in English

- **Status:** accepted
- **Date:** August 13, 2026
- **Phase:** 1

## Context

None of the five normative documents defines **what language** the compiler's messages are emitted in. The ambiguity went unnoticed until Phase 1 started producing real diagnostics.

The project was documented in Spanish, and the first diagnostics were written in Spanish for continuity. But the language of the internal documentation and the language of the compiler's output are different decisions: the first affects whoever develops Zirk, the second affects whoever uses it.

### The complication

`ZIRK_COMPILER_SPEC.md` section 8 fixes the minimal format with this example:

```text
error[E1234]: descripción precisa
  src/users.zrk:18:12
   |
18 |     expresión problemática
   |            ^ explicación localizada
   |
   = causa: motivo semántico
   = ayuda: acción concreta
```

The labels appear in Spanish. Two readings are possible:

1. **`causa:` and `ayuda:` are normative strings** that the implementation must emit literally.
2. **The example is illustrative prose** describing the *structure* — severity, code, location, cause, help — using the same language as the rest of the document.

## Decision

**The project is written in English.** This covers:

- the **five normative specs** in `docs/`;
- the **roadmap** and the **agent-startup prompt** in `docs/init/`;
- **code identifiers**: functions, variables, types, constants, and test names;
- **comments and code documentation** (`//`, `///`, `//!`);
- messages, causes, help text, format labels, and CLI output;
- the repository's scripts;
- **every OpenSpec change artifact** (`proposal.md`, `design.md`, `specs/**/*.md`, `tasks.md`), regardless of the language the change was requested in: they become `openspec/specs/`, which is normative;
- **`README.md` and `CONTRIBUTING.md`**: they are the project's first public surface, the one anyone reads before deciding whether to contribute, and they follow the same logic that already motivated the rest of this decision — see "Rationale" below;
- **every ADR under `docs/decisions/`, including this one, and `docs/TOOLCHAIN.md`**: once `README.md`, `CONTRIBUTING.md`, and the OpenSpec artifacts moved to English, keeping the decision record itself in Spanish was the last piece of the boundary still drawn in the wrong place — see the fourth correction under "How we got here."

The **second reading** of the spec is adopted: the example is illustrative. What is normative is that a cause and a help exist, not the words used to label them. The rest of that document is in Spanish because the document itself is in Spanish, not because the compiler must speak Spanish.

The labels become:

```text
error[E0308]: incompatible types
  src/main.zrk:4:24
  |
4 |     mut total: Int32 = "cuarenta";
  |                        ^^^^^^^^^^ expected Int32, found String
  |
  = cause: there is no implicit conversion from String to Int32
  = help: use Int32.parse("cuarenta") to convert at runtime
```

### What does **not** change

Only the record of *what actually happened, in the order it happened* stays in Spanish: git commit messages. Rewriting past commit messages would mean rewriting git history, which this ADR does not do and does not ask for; new commits keep following the Spanish convention in `CONTRIBUTING.md` unless a future decision says otherwise.

The boundary is now as simple as it can be: **everything that is a document, a spec, a decision record, or an artifact of this repository is written in English. Only the historical, unchangeable record of past commits stays in Spanish.**

### How we got here

This ADR has been corrected four times, and it's worth keeping all four corrections written down because they show where the boundary was misplaced:

1. **First version:** only the diagnostics in English, with the boundary drawn at "what a Zirk user sees versus what a Zirk builder sees." It failed because an external contributor reads the code before any document: `sincronizar()` is as real an entry barrier as a translated error message.

2. **Second version:** the code too, with the boundary at "code versus document." It failed because the normative specs **are not documentation of the project: they are the definition of the language**. Anyone who wants to understand Zirk reads them before the code, and they are the most public artifact the project has.

3. **Third version:** `README.md`, `CONTRIBUTING.md`, and the OpenSpec artifacts too. It failed for the same reason as the first two: it still treated as "internal Spanish documentation" the two documents an external contributor reads **first**, before any spec, and the artifacts that become specs (`openspec/specs/`) the moment a change is archived. Keeping them in Spanish protected nothing Phase 1 cared about protecting — it just repeated, at the project's own front door, the same mistake the translated diagnostic had already made.

4. **Fourth version (this one):** the ADRs themselves, `docs/decisions/README.md`, and `docs/TOOLCHAIN.md`. Keeping the decision record in Spanish while everything it decides on is in English produced the same asymmetry one level up: a contributor who reads a translated spec, a translated README, and a translated OpenSpec proposal, and then opens the ADR that explains *why*, hit a language switch at exactly the document meant to be the most durable and most referenced of all. There is no longer a "documentation about the project" category that is exempt: the only thing that stays in Spanish is what cannot be rewritten without rewriting history itself.

## Rationale

It is the ecosystem's de facto standard. Rust, Go, Zig, Swift, TypeScript, and Clang emit diagnostics in English, and their users — including Spanish speakers — look those messages up in English when something fails. A diagnostic in Spanish has no results on Stack Overflow or in anyone's documentation.

It also affects the project's future:

- `ZIRK_COMPILER_SPEC.md` section 9 requires structured output (`--json`) for tooling. An LSP, a linter, or a CI that consumes those messages expects English.
- A compiler that speaks Spanish narrows its base of contributors and users without gaining anything in return.

## Alternatives considered

**Spanish, for consistency with the documentation.** Internally consistent, externally hostile. It confuses two audiences that are not the same.

**Translatable diagnostics with language selection.** This is what `rustc` does with `--error-format` and partial translations, and it is real work: a message catalog, parametrization, and the risk that translations drift out of sync with the codes. It does not belong in Phase 1, and this ADR does not block it: the stable codes are exactly the hook that would make it possible later.

## Consequences

- Every message, identifier, comment, and test name in the workspace is translated.
- `zirk-diagnostics`'s labels become `cause:` and `help:`.
- Diagnostic codes are renamed to English: `TOKEN_INESPERADO` → `UNEXPECTED_TOKEN`, `CADENA_SIN_CERRAR` → `UNTERMINATED_STRING`, etc. The **stable codes** (`E0301`, `E0202`) do not change: they are the contract with tools and documentation.
- Test files are renamed: `lexico.rs` → `lexical.rs`, `gramatica.rs` → `grammar.rs`.
- `ZIRK_COMPILER_SPEC.md` section 8 already uses the real `cause:` and `help:` labels, so the ambiguity that originated this ADR is gone.
- `docs/init/ZIRK_AGENT_PROMPT.md` opens by stating this rule, so any agent picking up the project reads it before anything else.
- `README.md` and `CONTRIBUTING.md` were fully translated into English; `docs/init/ZIRK_AGENT_PROMPT.md` and the "Language" section of `CONTRIBUTING.md` were updated to no longer list them as a Spanish exception.
- Every OpenSpec change's artifacts (`proposal.md`, `design.md`, `specs/**/*.md`, `tasks.md`) are written in English from creation; `openspec/config.yaml` codifies this rule so it applies even when the change was requested in Spanish.
- Every ADR under `docs/decisions/` (including this one), `docs/decisions/README.md`, `docs/decisions/proximos-pasos-fase-4.md`, `docs/decisions/2026-08-20-auditoria-documentacion-y-specs.md`, and `docs/TOOLCHAIN.md` are translated into English.
- Every archived OpenSpec change under `openspec/changes/archive/` is translated into English, and every archived change folder whose name was in Spanish is renamed to its English equivalent (e.g. `2026-08-19-fase-3-objects-and-type-system` → `2026-08-19-phase-3-objects-and-type-system`), with every in-repo cross-reference to the old name updated in the same pass.
- Commit messages remain the only Spanish-language artifact in the repository going forward, by convention in `CONTRIBUTING.md`; past commit messages are not and cannot be rewritten.
