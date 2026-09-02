# Contributing to Zirk

## The rule that overrides all others

**The specs in `docs/` are normative and are written in English.** For any doubt about syntax, semantics, or scope, they override your own judgment, memory of other languages, or "what sounds reasonable."

And its corollary, which is an explicit rule of the spec itself:

> Every ambiguity must produce a question or be documented — **it must never be resolved silently by inventing unspecified behavior.**

If something is needed to move forward and the spec does not cover it, say so explicitly before deciding.

## The second rule: don't get ahead of the current phase

The spec describes **mature** Zirk: it is the result of years of work, not the starting point. Work advances by phases per [ZIRK_ROADMAP.md](docs/init/ZIRK_ROADMAP.md).

Features from future phases are not implemented even when they are already documented, defined, and tempting because "it's all right there." If a **real** need for something from a later phase comes up along the way, it is noted as a pending item and work continues on the current phase.

A phase is considered complete when its "Output" runs with real tests, not when the code "is almost there."

## Toolchain

See [docs/TOOLCHAIN.md](docs/TOOLCHAIN.md). Summary: Rust 1.94+ (pinned by `rust-toolchain.toml`) and LLVM **20.1** with static libraries, with `LLVM_SYS_201_PREFIX` pointing at its prefix.

On Windows, **no** official LLVM distribution works with `llvm-sys`: the `.exe` installer does not ship static libraries, and the development tarball is compiled against a different CRT than the one Rust uses. See [TOOLCHAIN.md](docs/TOOLCHAIN.md) for the source that does work.

## Branching model: git flow

```
   feature/*  ──▶  develop  ──▶  release/*  ──▶  main
   hotfix/*   ──────────────────────────────▶  main + develop
```

| Branch | Purpose |
|---|---|
| `main` | releases. Only receives merges from `release/*` and `hotfix/*` |
| `develop` | integration. It is the base for every feature |
| `feature/*` | new work. Branches from `develop` and merges back into `develop` |
| `release/*` | stabilization of a version |
| `hotfix/*` | urgent fix on top of a release |

**Never commit directly to `main` or `develop`.** Everything goes through a PR.

```sh
git checkout develop
git pull
git checkout -b feature/lexer-tokens
# ... work ...
gh pr create --base develop
```

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/):

```
feat(lexer): reconocer literales de duración
fix(codegen): corregir alineación en targets de 32 bits
docs(adr): registrar la decisión de estrategia de memoria
test(diagnostics): cubrir el renderizado sin fragmento de source
chore(ci): cachear la instalación de LLVM en Windows
```

Commit messages themselves stay in Spanish (see [Language](#language) below). Common scopes: `lexer`, `parser`, `ast`, `sema`, `ir`, `codegen`, `diagnostics`, `cli`, `runtime`, `ci`, `docs`, `adr`.

## Before opening a PR

```sh
./scripts/check-local.sh
```

Runs the same thing CI runs — formatting, clippy, and tests — detecting `LLVM_SYS_201_PREFIX` and validating the LLVM version before starting.

### Where each platform is verified

CI covers `linux-x86_64`, `linux-aarch64`, `macos-aarch64`, and `windows-x86_64`.

Working on your machine is not evidence that it works on Linux or Windows: that's what CI is for.

## What comes with every feature

`ZIRK_SPEC_FINAL.md` section 8 requires every feature to ship with grammar, typing rules, observable semantics, diagnostics, valid and invalid examples, and conformance tests.

In practice, at minimum:

- **one valid test case and one invalid one** — this is an explicit spec rule: every language rule must have at least one of each;
- **diagnostics with a cause and a help** when the invalid case produces an error;
- **responsibility and boundary documentation** if you touch a new crate.

## Diagnostics

Every compiler error follows the format in `ZIRK_COMPILER_SPEC.md` section 8:

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

### Language

**In English**: the normative specs, the roadmap, the agent-startup prompt, the code, its comments, diagnostic messages, test names, the OpenSpec change artifacts (`proposal.md`, `design.md`, `specs/**/*.md`, `tasks.md`), `README.md`, this file, every ADR under `docs/decisions/`, and `docs/TOOLCHAIN.md` — even when the conversation with whoever requests a change happens in Spanish. OpenSpec artifacts become `openspec/specs/`, which is normative, so they follow the same boundary as the language specs. See `openspec/config.yaml`.

**In Spanish**: only commit messages, by convention. Past commit messages are historical and are never rewritten.

The boundary: **every document, spec, and decision record in this repository is written in English; only the log of what actually happened, commit by commit, stays in Spanish.** The reasoning, and the four times this boundary moved, are in [ADR-006](docs/decisions/ADR-006-language-of-the-codebase.md).

It is built with `zirk-diagnostics`. Codes are **stable**: a published one is never reused for a semantically different error.

In a terminal they come out with color — the code in red, the highlighted fragment in bold, the cause in blue, and the help in violet — and in redirected output they come out as plain text. The reasoning is in [ADR-008](docs/decisions/ADR-008-color-en-diagnosticos.md).

If there is no clear fix, the help is omitted. A generic, non-actionable help is worse than none.

## Architecture decisions

Decisions that cascade to the rest of the project are recorded as ADRs in [docs/decisions/](docs/decisions/). They are the **durable source**: an OpenSpec change gets archived, an ADR does not.

An ADR is written when a decision affects several layers, is expensive to revert, or someone will ask in six months "why is this built this way?" Examples already recorded: the LLVM pin, the memory strategy, the runtime boundary.

**Check in before deciding on your own** on: memory strategy, crate structure, internal IR format, or anything the roadmap hasn't already settled.

## OpenSpec

The project uses [OpenSpec](https://github.com/Fission-AI/OpenSpec) for planning. **One change per roadmap phase.**

```sh
openspec list                              # active changes
openspec status --change fase-0-bootstrap  # progress
openspec validate fase-0-bootstrap
```

Each change has a proposal, a design, capability specs, and tasks. `design.md` references the ADRs instead of duplicating them.
