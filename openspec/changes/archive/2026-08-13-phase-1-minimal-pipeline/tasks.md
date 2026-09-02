## 1. Lexicon

- [x] 1.1 Define the subset's token type, including the keywords of the full language (D6)
- [x] 1.2 Implement scanning of identifiers, keywords, and delimiters, with a span per token
- [x] 1.3 Implement integer literals with `_` as a separator, rejecting invalid positions
- [x] 1.4 Implement string literals with `\n`, `\t`, `\"`, and `\\` escapes
- [x] 1.5 Implement boolean literals and line and block comments
- [x] 1.6 Emit lexical diagnostics: unterminated string, unterminated comment, unknown escape, unrecognized character
- [x] 1.7 Tests: one valid case and one invalid case per lexical rule

## 2. Syntax tree

- [x] 2.1 Define the subset's nodes in `zirk-ast`, each with its span (D1)
- [x] 2.2 Define the representation of syntactic types (`Void`, `Int32`, `Boolean`, `String`)
- [x] 2.3 Document in `lib.rs` that the AST is private and that the public Syntax API is Phase 10

## 3. Grammar

- [x] 3.1 Implement parsing of function declarations with parameters and return type
- [x] 3.2 Implement `mut` and `inmut` declarations, with explicit or inferred type
- [x] 3.3 Implement expressions with the precedence and associativity from the spec
- [x] 3.4 Implement `if`/`else` as a statement, with bodies always in braces
- [x] 3.5 Implement function calls, assignment, and `return`
- [x] 3.6 Allow omitting the semicolon when there is no ambiguity
- [x] 3.7 Emit specific diagnostics for constructs from later phases (D6)
- [x] 3.8 Emit a dedicated diagnostic for `import`, indicating that modules arrive later
- [x] 3.9 Tests: one valid case and one invalid case per grammar rule

## 4. Names, types, and flow

- [x] 4.1 Build the scope table with block nesting and shadowing
- [x] 4.2 Resolve identifiers to their declaration, with a diagnostic if none exists
- [x] 4.3 Implement the subset's type checking, with no implicit conversions
- [x] 4.4 Reject truthiness: require `Boolean` in every condition
- [x] 4.5 Restrict `&&`, `||`, and `!` to boolean operands
- [x] 4.6 Implement inference from the initializer when unambiguous
- [x] 4.7 Verify mutability: reject reassignment of `inmut`
- [x] 4.8 Verify arity and types of arguments against the signature
- [x] 4.9 Verify return coherence and that every path of a non-`Void` function returns
- [x] 4.10 Flow analysis for use before availability
- [x] 4.11 Reject integer literals outside the range of their type
- [x] 4.12 Verify the existence and signature of `main`
- [x] 4.13 Tests: one valid case and one invalid case per type rule

## 5. Intermediate representation

- [x] 5.1 Define the IR's types and its representation of typed values
- [x] 5.2 Define basic blocks with a single terminator (D2)
- [x] 5.3 Define the instruction set: arithmetic, comparison, logical, call, load, store, jump, conditional jump, return
- [x] 5.4 Define the abstract allocation operation, without naming a memory strategy (D3, ADR-003)
- [x] 5.5 Preserve the source location in every instruction
- [x] 5.6 Implement lowering from the verified tree, with locals as slots (D2)
- [x] 5.7 Lowering of `if`/`else` to blocks with conditional jump
- [x] 5.8 Well-formed IR verifier, used in tests
- [x] 5.9 Tests: expected IR for each construct in the subset

## 6. Backend

- [x] 6.1 Translate IR types to LLVM types
- [x] 6.2 Translate basic blocks and instructions to LLVM IR
- [x] 6.3 Implement arithmetic with overflow detection via LLVM intrinsics
- [x] 6.4 Implement division with a zero-divisor check
- [x] 6.5 Generate the entrypoint that invokes `zirk_rt_init` and `zirk_rt_shutdown` around `main`
- [x] 6.6 Materialize string literals as global constants plus a runtime call (D5)
- [x] 6.7 Tests: the generated LLVM module verifies for each construct in the subset

## 7. Runtime

- [x] 7.1 Define the internal representation of `String`, private to the runtime (ADR-005)
- [x] 7.2 Implement `zirk_str_from_utf8` as `extern "C"`
- [x] 7.3 Implement `zirk_io_println` as `extern "C"`, with a flush before finishing
- [x] 7.4 Tests: non-ASCII content is written correctly in UTF-8
- [x] 7.5 Tests: the new symbols appear without mangling in the static library

## 8. CLI

- [x] 8.1 Implement the compile subcommand over a single file
- [x] 8.2 Implement the compile-and-run subcommand, propagating the exit code
- [x] 8.3 Invoke the linker from the CLI, using the pinned LLVM toolchain (D7)
- [x] 8.4 Present diagnostics on stderr, in both human-readable and structured format
- [x] 8.5 Diagnostic for multiple files, indicating that projects arrive later
- [x] 8.6 Resolve where the executable ends up when running (design's open question)

## 9. End-to-end verification

- [x] 9.1 Create the corpus of valid `.zrk` programs, one per construct in the subset
- [x] 9.2 Create the corpus of invalid programs, with snapshots of their diagnostics
- [x] 9.3 End-to-end test: compile, link, run, and compare output and exit code
- [x] 9.4 Verify the roadmap's reference program: `fn main(): Void { stdout.println("Hello from Zirk"); }`
- [x] 9.5 Confirm CI passes on all four platforms in the matrix

## 10. Closure

- [x] 10.1 Update `docs/init/ZIRK_AGENT_PROMPT.md` with the phase's status
- [x] 10.2 Record in ADRs any architectural decision made during implementation
- [x] 10.3 Resolve or record as pending the design's open questions
</content>
