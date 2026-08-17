# The Zirk Handbook

The Zirk Handbook is the public, English-language guide to learning and using Zirk. It is written for people who want more than a syntax catalog: each chapter explains the problem a feature solves, develops a mental model, demonstrates real Zirk code, and identifies the boundaries that matter in production programs.

Zirk is a general-purpose, statically typed, compiled language. It is high-level by default, offers explicit access to lower-level facilities, produces standalone native binaries, and treats structured concurrency and multicore parallelism as first-class language concerns.

> **Project status**
>
> This handbook describes the normative target language. Zirk is under active implementation, so a feature documented here may not yet be available in the current compiler. Chapters call out that distinction where it matters. See [Language Status](./00-getting-started/03-language-status.md).

## Choose a starting point

- **I am evaluating Zirk.** Start with [What Is Zirk?](./00-getting-started/01-what-is-zirk.md), then read [Why Zirk Exists](./00-getting-started/02-why-zirk-exists.md).
- **I want to learn the language.** Follow the ordered chapters in [Summary](./SUMMARY.md). The getting-started sequence comes first; the core handbook then builds concepts in dependency order.
- **I already know another language.** Use the learning paths for TypeScript, Python, Rust, or systems programmers once those chapters are published.
- **I need an exact answer.** Use the reference sections for grammar, operators, diagnostics, the CLI, project manifests, targets, and standard-library contracts.
- **I want to understand a design choice.** Read the explanations on Zirk's type model, runtime, permissions, concurrency, safety boundaries, and compiler pipeline.

## How this handbook is organized

The corpus follows a layered model:

1. **Getting started** establishes identity, scope, status, installation, and the first complete program.
2. **Learning paths** give different audiences a shorter route through the relevant chapters.
3. **The language handbook** teaches syntax and semantics progressively.
4. **Guides** cover projects, the standard library, native programming, metaprogramming, tooling, testing, and packages.
5. **Tutorials** assemble concepts into complete applications.
6. **Reference** optimizes for lookup rather than teaching order.
7. **Explanations and appendices** cover architecture, rationale, terminology, compatibility, and source material.

The canonical hierarchy and published reading order live in [SUMMARY.md](./SUMMARY.md).

## Reading code examples

Examples use the `.zrk` source syntax defined by the normative language specification. A normal example expresses valid target-language code:

```zirk
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Hello from Zirk");
}
```

When an example is intentionally invalid, the chapter labels it and explains the expected diagnostic. When syntax is normative but not yet accepted by the current compiler, the chapter labels its implementation status rather than rewriting the language around a temporary compiler limitation.

## Source authority

The handbook derives its topic inventory from [`docs/01_plantilla_zirk.md`](../01_plantilla_zirk.md), but final specifications are authoritative. Begin with the master specification, then the [core semantics checkpoint](../CORE_LANGUAGE_SEMANTICS.md), [error/resource/permission checkpoint](../ERROR_RESOURCE_PERMISSION_SEMANTICS.md), [memory/unsafe checkpoint](../MEMORY_AND_UNSAFE_SEMANTICS.md), and [structured-concurrency checkpoint](../STRUCTURED_CONCURRENCY_SEMANTICS.md) before specialized specifications. See the [source map](./_editorial/source-map.md) for complete precedence.

## Contributing to the handbook

The chapters deliberately vary in length. Orientation pages should be concise; semantic chapters should include all explanation, constraints, examples, diagnostics, and feature interactions necessary to stand on their own. The [editorial guide](./_editorial/editorial-guide.md) defines the completion standard.

---

**Previous:** Start · **Next:** [ Getting Started with Zirk](00-getting-started/README.md)
