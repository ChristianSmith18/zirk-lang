## Context

See `proposal.md` for motivation and scope. The skill has a modular reference layout and already distinguishes normative language rules from current compiler support. The new guidance must extend that structure without replacing the normative source hierarchy or treating every explicit type, conversion, or nullable API as redundant.

## Goals / Non-Goals

**Goals:**

- Give agents a short, reusable idiom checklist before they write or revise Zirk examples.
- Make examples demonstrate current Result projection and preserve typed errors when callers need them.
- Audit every maintained source example in `demo/` and handbook code blocks against current contracts and idiomatic local style.
- Keep the website's imported handbook content synchronized after the repository changes are committed.

**Non-Goals:**

- Change Zirk syntax, type inference, conversion semantics, standard-library APIs, or compiler behavior.
- Remove useful annotations from public signatures, ambiguous/empty generic constructions, or declarations whose type communicates necessary intent.
- Ban `as`, all nullable-only APIs, or test assertions such as `assert.is_ok` based on spelling alone.
- Claim unsupported examples run merely because they are documented or bundled.

## Decisions

### Put a concise style gate in the skill and detailed guidance in references

Add a short “idiomatic Zirk” checklist to the main skill so it is applied regardless of which topic reference is selected. Keep concrete rules and examples in the relevant focused references: local declarations and conversions in `foundations.md`, failure selection and Result use in `failures-resources-and-authority.md`, and the cross-category example review in `writing-zirk.md`.

The checklist will prefer inferred local types for initialized bindings when the initializer already determines the intended type, and `inmut` for stable bindings when the type permits the intended operations. `mut` remains appropriate for rebinding, definite initialization, and writes through a value-semantic place that require a mutable binding. A mutable reference-backed referent alone does not require `mut`. An explicit annotation remains preferred when it defines a public contract, resolves ambiguity, fixes a deliberately chosen scalar width, describes an empty/generic construction, or preserves `Result<T,E>` for error inspection.

Use `as` only for a real, specified conversion between distinct types. It is not a type assertion or a way to repeat the initializer's known type. Do not remove a conversion when it expresses a required width or conversion rule.

### Teach Result projection alongside lossless handling

The guide will use direct `match` or an explicitly typed Result when a caller must distinguish errors. For ordinary value consumption, it will show contextual projection to `T?` and `??` when all errors may share a fallback, with an explicit reminder that `??` treats every `Error` alike. It will not recommend removed Result inspection, extraction, fallback, or unwrap helpers, nor collection `get_or_null` aliases.

The rule will be semantic rather than name-based: keep nullable-only methods when absence is their complete outcome and there is no parallel Result operation; keep test assertions such as `assert.is_ok`; encode multiple successful states in a non-null domain payload inside Result.

### Audit maintained examples from contracts outward

Review each source file and README beneath `demo/`, then handbook code examples. For each API call, consult the owning handbook and normative semantic source. For style, remove only redundant local annotations, casts, and mutable bindings; retain deliberate type declarations and conversions. Fix stale `Result.unwrap` uses based on the API's actual return type, using `match` or contextual projection according to whether the example needs the error.

Examples that are currently supported will be checked with the checkout compiler. Normative-only or unsupported snippets will be labeled accurately rather than weakened into different behavior. The audit will include a documented inventory of files checked and any examples deliberately retained with annotations/conversions and why.

### Keep public handbook and website content aligned

After committing source documentation changes, run `./scripts/sync-website-content.sh`, review generated changes in `../zirk-lang-site`, and validate the imported content. This change does not modify project status, so an audit-date change is not expected unless the source audit reveals status evidence that also needs revision.

## Risks / Trade-offs

- [Risk] A broad style sweep could remove meaningful type intent or width conversions. → Mitigation: apply explicit removal criteria, check the declared/inferred type and use site, and record intentional exceptions in the audit.
- [Risk] A `Result` used as an ordinary value silently loses its error payload. → Mitigation: teach and validate the distinction between nullable projection and explicit matching in both the skill and examples.
- [Risk] Some demos may include features unavailable in the current compiler. → Mitigation: classify these using feature status and report normative validity separately from execution evidence.
- [Risk] Website sync requires a clean committed source revision. → Mitigation: make the source commit and generated-site review explicit completion tasks.
