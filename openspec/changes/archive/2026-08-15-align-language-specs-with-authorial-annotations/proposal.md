## Why

The current normative specifications predate the language author's 37-point review of the public handbook, leaving the handbook and the compiler-facing contracts inconsistent. The decisions must be consolidated now, on the latest `develop`, before Phase 3 implementation treats older omissions or contradictory examples as authoritative.

## What Changes

- **BREAKING** Prohibit ordinary local shadowing while defining explicit lambda-capture qualification through `this`.
- Add exponentiation, complete range and slicing behavior, classic `for`, single-statement `if`, `do ... while`, regex literals and regex patterns, and comma-grouped match alternatives.
- Define optional and variadic parameter rules, optional `fn` on lambdas, callable method cloning, and direct standard-library convenience member resolution.
- Define `public mut` as the class-field default, multiple constructor signatures, named constructor arguments, reserved operator methods, and safe customization rules.
- Complete traditional and algebraic enum behavior, records, value classes, fixed arrays, strings as iterables, generators, and pure-function pipelines.
- Update the consolidated and specialized language documents plus the active Phase 3 artifacts so all current planning uses the same contract.
- Keep compiler/runtime implementation out of this change; unsupported syntax remains documented target behavior until its implementation phase.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `zirk-lexical-syntax`: Add the lexical surface for exponentiation, regex literals, and newly confirmed control/generator words.
- `zirk-grammar`: Define the corrected control-flow, range, slicing, pattern, lambda, constructor, enum, and generator productions.
- `zirk-type-system`: Define shadowing, parameters, captures, operator methods, object defaults, enum values, arrays, iteration, and callable-value semantics.
- `zirk-modules`: Define direct convenience-member resolution for imported standard-library objects and ambiguity qualification.

## Impact

- Updates `docs/ZIRK_SPEC_FINAL.md`, `docs/ZIRK_LANGUAGE_SPEC.md`, `docs/ZIRK_STDLIB_SPEC.md`, and the historical decision inventory where its recorded answer is superseded.
- Updates main OpenSpec capabilities and the active `fase-3-objects-and-type-system` proposal, design, delta specs, and tasks where Phase 3 owns implementation.
- Creates no compiler, runtime, standard-library implementation, website, or package-format changes.
