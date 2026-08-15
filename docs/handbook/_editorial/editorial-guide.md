# Handbook Editorial Guide

This guide defines what makes a Zirk handbook chapter publishable. It is a quality contract, not a rigid page template.

## Begin with the reader's question

A chapter should first establish why its subject matters. For example, a nullability chapter should begin with the problem of representing absence safely—not with the grammar for `T?`. Introduce terminology only when the reader has a reason to use it.

## Build one mental model at a time

Order concepts by dependency. Give the smallest complete example that exposes the central rule, explain what the compiler and runtime observe, and then add constraints and interactions. Avoid presenting a dense list of unrelated syntax forms before the reader understands their common purpose.

## Choose sections to fit the subject

A feature chapter may need sections such as:

- the problem and first example;
- syntax and semantics;
- inference and type relationships;
- valid and invalid examples;
- expected compiler diagnostics and corrections;
- interaction with mutability, nullability, errors, concurrency, resources, permissions, and targets;
- guidance on when to use or avoid the feature;
- implementation status, normative sources, and related topics.

Use only the sections that improve understanding. A short orientation page and a deep concurrency chapter should not be forced to have the same size or shape.

## Use code as evidence

Examples must be compatible with the cited final specification. Prefer complete examples with meaningful names over isolated fragments. Label intentionally invalid code before the fence, explain why it fails, and show the likely diagnostic shape without inventing a stable diagnostic code that the compiler specification has not assigned.

Use `zirk` as the fenced-code language. If the current syntax highlighter does not recognize it, that is a presentation issue; the source should still identify the language correctly.

## Distinguish three kinds of truth

1. **Normative behavior** is required by the final specification.
2. **Current implementation** describes what the compiler can do today.
3. **Teaching simplification** intentionally omits detail that is introduced later.

Never present a temporary implementation gap as language semantics. Never present an old template decision as current when the final specification excludes it.

## End with useful navigation

Every ordered chapter ends with a divider and links to its previous and next chapter. The first and last chapters identify the boundary explicitly. Add “See also” links when related material is useful but not adjacent. Link to the relevant normative source either directly or through the source map.

## Definition of done

Before marking a chapter complete, verify that:

- its audience and purpose are clear in the opening;
- terminology agrees with neighboring chapters and the glossary;
- examples use normative syntax and identify implementation limitations;
- important failure modes have explanations and corrections;
- links resolve and previous/next links match `SUMMARY.md`;
- source claims can be traced to a final specification;
- the chapter is as long as necessary, but contains no filler.

---

**Handbook:** [The Zirk Handbook](../README.md) · **Sources:** [Source Map →](./source-map.md)
