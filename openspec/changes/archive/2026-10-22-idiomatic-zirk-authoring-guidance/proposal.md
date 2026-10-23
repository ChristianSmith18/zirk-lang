## Why

The Zirk authoring skill and repository examples do not consistently teach the current language style. In particular, the Result demo still calls removed helpers, some temporal handbook examples call removed `Result.unwrap`, and the guidance does not clearly discourage redundant local type annotations or casts. Aligning the skill, demos, and handbook examples will make generated and human-written Zirk code more concise, idiomatic, and consistent with current contracts.

## What Changes

- Update the Zirk language skill and its references with practical rules for local type inference, binding mutability, explicit conversions, Result projection, typed error handling, and APIs whose only absence outcome is nullable.
- Audit every source file and README under `demo/` for current API validity and idiomatic Zirk style; migrate obsolete Result helpers and remove only demonstrably redundant annotations, casts, or mutable bindings.
- Audit handbook code examples for the same issues, correcting stale Result unwrap calls and other confirmed examples that contradict current language or standard-library contracts.
- Validate runnable examples with the current compiler and distinguish verified examples from normative or unsupported ones.
- Synchronize the companion `../zirk-lang-site` after committing public handbook changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This change updates authoring guidance and examples; it does not change language or library behavior. `skip_specs: true` is set in `.openspec.yaml`.

## Impact

- `.agents/skills/zirk-language/SKILL.md` and its focused references.
- All applicable `.zrk` examples and project instructions under `demo/`.
- Zirk handbook examples, including temporal and Result chapters, where the audit confirms stale or non-idiomatic code.
- `../zirk-lang-site`, whose generated handbook content must be synchronized from a clean source commit.
- No compiler, runtime, API, dependency, feature-status, limitation, or roadmap changes are intended.
