## 1. Authoring skill

- [x] 1.1 Update the main Zirk skill with an idiom gate for inferred local types, `mut` versus `inmut`, necessary conversions, current Result handling, and normative-versus-implemented status; verify each rule links to its detailed reference.
- [x] 1.2 Update the focused references and cross-category checklist with preferred/avoid examples; verify they reject removed Result and collection helpers without banning legitimate nullable-only APIs or test assertions.

## 2. Repository demos

- [x] 2.1 Audit every source file under `demo/` against current language and library contracts; migrate the obsolete Result API examples in `demo/13-resultados.zrk` and remove only confirmed redundant annotations, casts, or mutable bindings; verify the audit covers every demo source path.
- [x] 2.2 Update `demo/README.md` to describe the idiomatic example conventions and accurately label compiler support; verify listed demos and their status match the files and feature status.

## 3. Handbook examples

- [x] 3.1 Audit handbook examples for removed Result helpers, redundant local annotations/conversions, and mutable bindings without a demonstrated need; fix confirmed violations and verify each retained annotation or conversion expresses needed intent.
- [x] 3.2 Replace the three `Result.unwrap()` calls in the Date, Time, and DateTime handbook examples with explicit Result matching; verify no temporal handbook example still calls `Result.unwrap()`.
- [x] 3.3 Remove redundant `mut` bindings and the redundant local annotation from confirmed cast examples in the handbook, language spec, and `docs/01_plantilla_zirk.md`; verify real checked casts/conversions remain visible and their result types are inferred.

## 4. Validation and publication

- [x] 4.1 Check every demo supported by the current compiler using the checkout binary; the 29 executable demo entrypoints passed `zirk check`. The imported geometry module was validated through its project entrypoint; unsupported or normative-only examples are not claimed as runtime-tested.
- [x] 4.2 Review the complete documentation/example diff, run strict OpenSpec validation, search for removed APIs and obsolete styles, and check all supported demos; verify the changes affect guidance and examples only, not language or library behavior.
- [x] 4.3 Commit the source documentation, synchronize `../zirk-lang-site`, review the generated site diff, and validate the site content; verify both repositories are clean and the site references the committed source revision.
