## 1. Canonical Semantic Checkpoint

- [x] 1.1 Update the authorial checkpoint with the accepted callable, projection, object, generic, algebraic-data, collection, and matching decisions
- [x] 1.2 Rewrite affected normative language sections and preserve explicit implementation-status boundaries
- [x] 1.3 Expand standard-library contracts for `Fn`, Clone-dependent projections, collections, iteration, generators, and external enum behavior
- [x] 1.4 Correct superseded answers and examples in the authorial inventory without treating historical questions as normative
- [x] 1.5 Align affected active Phase 3 proposal/design/spec/task artifacts while keeping later-phase implementation out of Phase 3

## 2. Functions and Callables

- [x] 2.1 Document `Function(...) => R`, preferred `Fn`, labels, optionals, variadics, defaults, variance, and compatibility
- [x] 2.2 Document contextual lambda inference, explicit recursion, whole-reference versus projected capture, mutable lifted cells, and escape behavior
- [x] 2.3 Document callable assignment, identity, deep cloning, bound/unbound methods, callable objects, and absence of structural equality
- [x] 2.4 Document generator laziness, `Iteration<T>`, cleanup, Result-shaped iteration failure, and pipeline first-argument lowering
- [x] 2.5 Replace obsolete callable syntax and Phase 3-final-semantics claims throughout handbook/reference material

## 3. Projection, Object, and Contract Semantics

- [x] 3.1 Rewrite value/reference semantics around whole-reference aliasing, deep projection copies, and place expressions
- [x] 3.2 Replace property language with attributes plus conventional `get_`/`set_` methods and automatic type defaults
- [x] 3.3 Document construction, `super`, `override fn`, virtual dispatch, visibility, identity, cloning, and strict aliases
- [x] 3.4 Document abstract classes as requirement sets adopted through `implements`, concrete `extends`, interfaces, and state-free traits
- [x] 3.5 Document composition, trait conflict selection, exact override parameters, covariant results, and explicit derivation
- [x] 3.6 Document strict/optional casts and update all object examples and decision guides

## 4. Generics

- [x] 4.1 Document declarations, explicit application, `from A & B`, valid constraint kinds, and call-site diagnostics
- [x] 4.2 Document inference sources/boundaries, trailing defaults, methods, constructors, and factories
- [x] 4.3 Document invariance, `in`/`out`, callable variance interaction, and invalid position examples
- [x] 4.4 Document recursive constraints, `Box<T>`, finite layout, complete runtime identity, and casts
- [x] 4.5 Document single-check generic bodies, portable IR, monomorphization, safe code sharing, and excluded associated/HKT features
- [x] 4.6 Apply projection-Clone requirements to generic extraction APIs and examples

## 5. Algebraic Data and Matching

- [x] 5.1 Add the Tuple chapter with `Tuple(...)`, literal/destructuring forms, constant `[]` access, negative indexes, and capability derivation
- [x] 5.2 Expand records with defaults, named construction, immutable methods, structural equality/hash, cloning, and invalid mutation/construction
- [x] 5.3 Expand traditional enums with `.name`, `.value`, conversions, mapping uniqueness, absence of implicit index/order, and external behavior
- [x] 5.4 Expand algebraic enums with generic payload construction, equality/clone derivation, no user methods, and match-only extraction
- [x] 5.5 Expand union normalization, common capability access, narrowing, and enum-versus-union guidance
- [x] 5.6 Rewrite matching for exhaustiveness in statements/expressions, no guards, alternatives, regex, nesting, reachability, Never, and projection-copy bindings
- [x] 5.7 Document direct destructuring only for records/tuples and reject enum/rest destructuring forms

## 6. Collections and Iteration

- [x] 6.1 Rewrite collection overview, family selection, literals, storage/alias categories, strictness, and capacity safety
- [x] 6.2 Expand Array and fixed-array APIs, equality, indexing, mutation, conversion, complexity, and invalid structural operations
- [x] 6.3 Expand List API, capacity, structural mutation, equality, cloning, and controlled empty/allocation errors
- [x] 6.4 Expand Map API, key constraints, access, entries, defaults/factories, equality, ordering, cloning, and mutation
- [x] 6.5 Expand Set API, equality constraints, mathematical operations, ordering, cloning, and mutation
- [x] 6.6 Finalize negative indexing and direct versus Result/nullable safe access across supported families
- [x] 6.7 Finalize Python-style omitted slice defaults, negative step, strict explicit bounds, deep-copy results, and equal-length replacement
- [x] 6.8 Document `Iterable<out T>`, `Iterator<T>`, `Iteration<T>`, copy-yielding loops, generators, and Range behavior
- [x] 6.9 Document deterministic iterator invalidation, explicit read-only views, lifetimes/status boundary, and future mutable iteration
- [x] 6.10 Publish per-collection API/complexity/error/contract tables and valid/invalid examples

## 7. Reference and Information Architecture

- [x] 7.1 Update built-in types, operators, literals, grammar, keywords, type member, and feature-status references
- [x] 7.2 Add callable, object/contract, generic, algebraic-data, collection, iteration, and matching reference matrices
- [x] 7.3 Resolve duplicate unlinked type placeholders and preserve genuinely future planned sections
- [x] 7.4 Update glossary, language comparisons, valid/invalid examples, learning paths, and source ownership map
- [x] 7.5 Regenerate previous/next navigation for the final SUMMARY order

## 8. Verification

- [x] 8.1 Audit every relative link, anchor, Markdown fence, heading, table, and code example
- [x] 8.2 Audit contradictions for old function syntax, nonescaping closures, nested reference aliases, properties, abstract extends, permissive overrides, weak generics, enum methods, match guards, nullable iterator completion, and clamping slices
- [x] 8.3 Verify complex chapters satisfy editorial depth while compact pages omit only inapplicable material
- [x] 8.4 Validate this change, active affected changes, and all OpenSpec artifacts in strict mode
- [x] 8.5 Confirm the final diff is documentation/OpenSpec-only and report exact source-of-truth reading paths
