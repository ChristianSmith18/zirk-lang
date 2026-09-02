## 1. Lexicon and grammar -- loops and conditional as expression

- [x] 1.1 Recognize this phase's keywords, add `in` and the tokens `..`, `..=`, `...`, and remove all of them from `phase()`
- [x] 1.2 Parse `for (init; cond; incr) { }`, `for x in expr { }`, `while cond { }`, `loop { }`
- [x] 1.3 Parse `break` and `continue`, without yet requiring them to be inside a loop (sema validates this)
- [x] 1.4 Extend `if`/`else` to admit expression position (D7)
- [x] 1.5 Remove `for`, `while`, `loop`, `match` from the "construct from a later phase" diagnostic
- [x] 1.6 Tests: one valid and one invalid case for each new rule

## 2. Grammar -- complete functions and closures

- [x] 2.1 Parse optional parameters (`name?: Type`)
- [x] 2.2 Parse default values in parameters
- [x] 2.3 Parse the variadic parameter (`...name: Type`), rejecting it if not last
- [x] 2.4 Parse named arguments in calls
- [x] 2.5 Parse expression and block lambdas (D4)
- [x] 2.6 Tests: one valid and one invalid case for each new rule

## 3. Grammar -- `match` and minimal enum

- [x] 3.1 Parse `enum Name { Constructor, ... }` without associated data (D1)
- [x] 3.2 Parse `match` as a statement and as an expression, with `pattern => body` arms
- [x] 3.3 Parse patterns: literal, enum constructor, binding, `_`
- [x] 3.4 Emit a specific diagnostic for `match with` (out of scope, depends on `Resource<E>`)
- [x] 3.5 Tests: one valid and one invalid case for each new rule

## 4. Grammar -- nullability and compound operators

- [x] 4.1 Parse `T?` in type position
- [x] 4.2 Parse `null` as a literal
- [x] 4.3 Parse `??` with correct precedence relative to the other operators
- [x] 4.4 Reject `?.` with the phase diagnostic, pointing at Phase 3 (D8)
- [x] 4.5 Expand `+=`, `-=`, `*=`, `/=`, `%=` to the equivalent assignment, in statement position (D9)
- [x] 4.6 Expand prefix and postfix `++` and `--` to the equivalent assignment, in statement position (D9)
- [x] 4.7 Reject increment and decrement in expression position, with an explicit diagnostic (D9)
- [x] 4.8 Tests: one valid and one invalid case for each new rule

## 5. Grammar -- modules

- [x] 5.1 Parse the `share` modifier on top-level declarations
- [x] 5.2 Parse `import { names } from "path"` with quoted local paths
- [x] 5.3 Parse import aliases (`name -> alias`)
- [x] 5.4 Parse `import { names } from standard.module;` without quotes
- [x] 5.5 Parse `use name;`
- [x] 5.6 Remove Phase 1's diagnostic that rejected every `import`
- [x] 5.7 Tests: one valid and one invalid case for each new rule

## 6. Module resolution (`zirk-modules`)

- [x] 6.1 Build the `import` graph across a crate's files (D6)
- [x] 6.2 Admit mutual imports, reading each file only once
- [x] 6.3 Resolve local paths relative to the importing file
- [x] 6.4 Apply the binary visibility rule (`share` vs. private to the file)
- [x] 6.5 Detect and report `share` name collisions across files
- [x] 6.6 Resolve import aliases in the scope table
- [x] 6.7 Resolve `use` over already-imported names, rejecting those that are not
- [x] 6.8 Recognize `std.io` as the only standard module for this phase
- [x] 6.9 Run this pass before the existing type checking, without changing its shape
- [x] 6.10 Tests: one valid and one invalid case for each rule, including a multi-file crate

## 7. Types and flow -- loops and `if` as expression

- [x] 7.1 Require `Boolean` with no truthiness in the condition of `for` and `while`
- [x] 7.2 Reject `break`/`continue` outside a loop
- [x] 7.3 Resolve which loop `break`/`continue` points to in nested loops
- [x] 7.4 Implement the minimal `for ... in` protocol: ranges and `String` (D3)
- [x] 7.5 Reject `for ... in` over unsupported types, with a phase diagnostic
- [x] 7.6 Type `if` as an expression: require both branches and compatible types (D7)
- [x] 7.7 Extend the returns-on-every-path analysis to consider an exhaustive `if` and infinite loops with no exiting `break`
- [x] 7.8 Tests: one valid and one invalid case for each new rule

## 8. Types and flow -- complete functions and closures

- [x] 8.1 Resolve named arguments to position against the signature (D4)
- [x] 8.2 Substitute missing parameters with their default value, evaluated at the call site
- [x] 8.3 Pack extra arguments into the variadic parameter
- [x] 8.4 Type null values for optional parameters with no argument
- [x] 8.5 Infer a lambda's function type from its parameters and body
- [x] 8.6 Record which external variables a closure captures
- [x] 8.7 Reject mutation of a captured variable (D2)
- [x] 8.8 Tests: one valid and one invalid case for each new rule

## 9. Types and flow -- `match`

- [x] 9.1 Record the set of constructors for each declared `enum`
- [x] 9.2 Verify `match` exhaustiveness over `enum`: all constructors or `_`
- [x] 9.3 Require `_` as the final arm in `match` over types without a closed set
- [x] 9.4 Verify that all arms of a `match` expression produce a common type
- [x] 9.5 Tests: one valid and one invalid case for each new rule

## 10. Types and flow -- nullability

- [x] 10.1 Represent `T?` as its own type, distinct from `T`, in the type system (D5)
- [x] 10.2 Admit `T` where `T?` is expected, and reject the opposite direction, suggesting `??`
- [x] 10.3 Type `null`, rejecting it where the type does not admit absence of value
- [x] 10.4 Type `??` requiring a common type, producing the non-nullable type when the fallback is not nullable
- [x] 10.5 Reject `??` over a non-nullable left operand
- [x] 10.6 Tests: one valid and one invalid case for each new rule

## 11. IR -- loops, `if`-expression, and `break`/`continue`

- [x] 11.1 Extend lowering to produce block graphs with cycles
- [x] 11.2 Lower `while`, `loop`, `for`, and `for ... in` to condition/body/continuation blocks
- [x] 11.3 Lower `break`/`continue` to direct jumps to the corresponding block of their enclosing loop
- [x] 11.4 Lower the `if`-expression, producing the value of the taken branch in the continuation block
- [x] 11.5 IR verifier: accept well-formed cycles, keep rejecting misplaced terminators
- [x] 11.6 Tests: expected IR for each new construct

## 12. IR -- closures, `match`, nullability

- [x] 12.1 Define the closure environment allocation operation, reusing the existing abstract allocation (D2)
- [x] 12.2 Lower closure creation to an independent function plus an environment with copied captures
- [x] 12.3 Lower a call to a closure value as an indirect call with an implicit environment
- [x] 12.4 Lower `match` to comparisons over the discriminant with a jump to each arm's block
- [x] 12.5 Lower `match`-expression with a common continuation block that receives the arm's value
- [x] 12.6 Lower `??` to an explicit null check with lazy evaluation of the fallback (D5)
- [x] 12.7 Tests: expected IR for each new construct

## 13. LLVM backend

- [x] 13.1 Translate blocks with cycles, verifying the resulting LLVM module
- [x] 13.2 Translate closures: LLVM function with the environment as its first argument, aggregate value (function, environment)
- [x] 13.3 Translate indirect calls to closures
- [x] 13.4 Translate exhaustive `match` over `enum` to an LLVM `switch`
- [x] 13.5 Translate non-exhaustive `match`, or `match` over other types, to chained comparisons
- [x] 13.6 Translate `??`'s null check, at no cost for non-nullable types
- [x] 13.7 Tests: the generated LLVM module verifies for each new construct

## 14. End-to-end verification

- [x] 14.1 Expand the corpus with valid programs: loops, closures, exhaustive `match`, nullability, multi-file modules
- [x] 14.2 Expand the corpus with invalid programs, with snapshots of their diagnostics
- [x] 14.3 Test a multi-file crate with `share`/`import`/`use`, compiled and run end to end
- [x] 14.4 Test mutual importing and importing a nonexistent file
- [x] 14.5 Confirm CI passes on all four platforms of the matrix

## 15. Closeout

- [x] 15.1 Update `docs/init/ZIRK_AGENT_PROMPT.md` with the phase's status
- [x] 15.2 Record in ADRs any architecture decision made during implementation
- [x] 15.3 Resolve, or record as pending, the design's open questions
