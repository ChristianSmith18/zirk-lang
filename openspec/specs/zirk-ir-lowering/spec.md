# zirk-ir-lowering

## Purpose

Defines the typed intermediate representation and its generation from the verified tree.

The IR is the boundary that gets distributed inside a `.zpkg`, so its form is a contract. See `docs/decisions/ADR-007-forma-de-la-ir.md`.
## Requirements
### Requirement: Typed IR

The intermediate representation SHALL be typed: every operation and every value know their type, per `ZIRK_COMPILER_SPEC.md` section 4.

#### Scenario: Type of every value
- **WHEN** any IR value is inspected
- **THEN** it exposes its type

#### Scenario: Operation with incompatible operands
- **WHEN** an operation whose operands do not match its signature is constructed
- **THEN** the construction fails and does NOT produce invalid IR

### Requirement: Basic-block form

The IR SHALL represent the body of each function as a graph of basic blocks, each terminated by exactly one terminator instruction. The graph SHALL support cycles, produced by the lowering of loops.

This is what enables the flow analysis that `ZIRK_COMPILER_SPEC.md` section 4 requires, and the form that Phase 1 (ADR-007) already anticipated with this phase's loops in mind.

#### Scenario: Block with a single terminator
- **WHEN** any basic block is inspected
- **THEN** it ends in a jump, conditional jump, or return
- **AND** it contains no terminator instructions in an intermediate position

#### Scenario: Conditional
- **WHEN** an `if` statement with both branches is lowered
- **THEN** blocks are produced for the condition, each branch, and the continuation
- **AND** the condition block ends in a conditional jump

#### Scenario: Loop
- **WHEN** a `while`, `loop`, or `for` is lowered
- **THEN** a condition or body block is produced that jumps back to itself or to an earlier block
- **AND** the resulting block graph contains a cycle

### Requirement: Independence from the memory model

The IR SHALL NOT express operations tied to a concrete memory strategy. Allocation is expressed abstractly and resolved by the runtime. This includes the allocation of a closure's capture environment.

This is a direct requirement of `docs/decisions/ADR-003-memoria.md`.

#### Scenario: Allocation operation
- **WHEN** the IR needs to express that a value is allocated
- **THEN** it uses an abstract operation that names the type
- **AND** it does NOT name `malloc`, reference counting, or garbage collection

#### Scenario: Allocation of a closure's environment
- **WHEN** a lambda that captures variables from the enclosing scope is lowered
- **THEN** the capture environment is allocated with the same abstract operation as any other type
- **AND** the IR does not name where that environment lives in memory

### Requirement: Local variables as slots

Local variables SHALL be represented as slots with load and store operations, without their own SSA form.

Promotion to registers is delegated to the backend. A dedicated SSA form does not pay off until dedicated optimizations exist.

#### Scenario: Reading and writing a local
- **WHEN** the use of a local variable is lowered
- **THEN** a load from its slot is produced
- **AND** an assignment produces a store to that slot

### Requirement: Traceability to the source

Every IR instruction SHALL retain the source location that originated it.

Without it, the faithful mapping to `.zrk` that `ZIRK_COMPILER_SPEC.md` section 11 requires of the debugger is not possible.

#### Scenario: Location of an instruction
- **WHEN** any IR instruction is inspected
- **THEN** it exposes the corresponding source location

### Requirement: Lowering from the verified tree

IR generation SHALL start from the already-resolved and verified tree, not from the parser's raw tree.

#### Scenario: Input to lowering
- **WHEN** IR is generated
- **THEN** the input has resolved names and verified types
- **AND** lowering does NOT re-check types

### Requirement: Lowering of loops and of `break`/`continue`

Lowering SHALL translate `for`, `for ... in`, `while`, and `loop` into basic blocks with the condition evaluated in its own block, and SHALL translate `break`/`continue` into a direct jump to the continuation block or to the condition block of the loop that contains them.

#### Scenario: `while`
- **WHEN** `while cond { body }` is lowered
- **THEN** a condition block, a body block, and a continuation block are produced
- **AND** the body block ends by jumping back to the condition block

#### Scenario: `break`
- **WHEN** a `break` inside a loop is lowered
- **THEN** a direct jump to that loop's continuation block is produced

#### Scenario: `continue`
- **WHEN** a `continue` inside a `for` is lowered
- **THEN** a direct jump to the `for`'s increment block is produced, not to the condition block

#### Scenario: Nested loops
- **WHEN** a `break` is inside an inner loop, nested within an outer one
- **THEN** the resulting jump targets the continuation of the inner loop, not the outer one

### Requirement: Lowering of `if` as an expression

Lowering SHALL translate an `if`/`else` in expression position into blocks whose continuation block receives the value of the branch taken, without introducing an instruction form different from the one already used by `if` as a statement.

#### Scenario: Value of the selected branch
- **WHEN** `mut r = if x > 0 { a } else { b };` is lowered
- **THEN** the continuation block produces a value that comes from the block of the executed branch

### Requirement: Lowering of closures

Lowering SHALL translate a lambda into an independent function plus an environment value with the captured values copied at the point of creation, and SHALL translate a call to a closure as an indirect call that receives the environment as an implicit argument.

#### Scenario: Creation of a closure
- **WHEN** `inmut ADD = (a: Int32, b: Int32): Int32 => a + b;` is lowered with no captures
- **THEN** an independent function and a closure value with no environment or with an empty environment are produced

#### Scenario: Closure with capture
- **WHEN** a lambda references a variable from the enclosing scope
- **THEN** the allocated environment contains a copy of that variable at the time of creation
- **AND** the body of the lowered function reads the variable from the environment, not from the original slot

### Requirement: Lowering of `match`

Lowering SHALL translate a `match` into a sequence of comparisons on the `enum`'s discriminant (or on the value, for literals), each with a conditional jump to its arm block, ending in the `_` wildcard block if one exists.

#### Scenario: `match` on an `enum`
- **WHEN** a `match` with one arm per constructor is lowered
- **THEN** one comparison block per constructor and one block per arm body are produced

#### Scenario: `match` as an expression
- **WHEN** a `match` used as an expression is lowered
- **THEN** each arm block ends by jumping to a common continuation block that receives the value of that arm

### Requirement: Lowering of null coalescing

Lowering SHALL translate `a ?? b` into an explicit nullity check with two blocks, which produces `a` when it is not null and evaluates `b` in the other block.

#### Scenario: Check with two blocks
- **WHEN** `a ?? b` is lowered
- **THEN** a nullity check on `a` is produced, with one block per outcome

#### Scenario: Null coalescing evaluates the fallback lazily
- **WHEN** `a ?? costly()` is lowered
- **THEN** `costly()` is only lowered inside the block that executes when `a` is null

### Requirement: Lowering of objects

Lowering SHALL translate the construction of an object into the abstract allocation operation followed by the initialization of its fields, and access to a field into a read by offset.

#### Scenario: Construction
- **WHEN** `User(1)` is lowered
- **THEN** the abstract allocation of the type is emitted
- **AND** the constructor's body initializes the fields

#### Scenario: Field access
- **WHEN** `u.id` is lowered
- **THEN** a read at the field's offset is emitted, with no lookup by name

### Requirement: Lowering of method calls

Lowering SHALL emit a direct call when the method is not overridable, and an indirect call through the type's table when it is.

Most calls fall into the first case, and paying for an indirection on all of them would mean paying for a generality the program does not use.

#### Scenario: Non-overridden method
- **WHEN** no subclass overrides the called method
- **THEN** a direct call is emitted

#### Scenario: Overridden method
- **WHEN** some subclass overrides it
- **THEN** an indirect call through the type's table is emitted

#### Scenario: Call through a contract
- **WHEN** the receiver has the type of an interface
- **THEN** dispatch happens through that interface's table

### Requirement: Lowering of safe access

Lowering SHALL translate `expr?.member` into an explicit nullity check with two blocks: the present case accesses the member and the absent case produces the null value.

It reuses the mechanism `??` introduced in the previous phase; what arrives here is the operator, not the machinery.

#### Scenario: Safe access
- **WHEN** `user?.name` is lowered
- **THEN** a nullity check is produced, with one block per outcome

#### Scenario: Chained
- **WHEN** `a?.b?.c` is lowered
- **THEN** each link checks before accessing

### Requirement: Generic specialization

Lowering SHALL produce one function per combination of type arguments used, and reuse it when the combination repeats.

#### Scenario: One copy per combination
- **WHEN** a generic function is used with two distinct combinations
- **THEN** the IR contains two functions

#### Scenario: No duplicates
- **WHEN** the same combination is used multiple times
- **THEN** the IR contains a single function for it

### Requirement: Integer type parameterized by width and signedness in the IR

The IR SHALL represent each integer type with a single type parameterized by width and signedness, not with a distinct variant per width, so that an existing arithmetic or conversion instruction remains valid for any width without being duplicated. Widening and promotion instructions for mixed-width and mixed-family arithmetic SHALL use the same parameterised type forms.

#### Scenario: Arithmetic instruction independent of width
- **WHEN** an addition between two `Int8` operands is lowered
- **THEN** the same instruction form as for `Int32` is emitted, with the width as data of the type, not a distinct instruction

#### Scenario: Widening instruction is parameterised by source and target widths
- **WHEN** an `Int8` operand is promoted to `Int32` before a binary operation
- **THEN** the widening instruction carries both source and target width/signedness parameters, not new opcodes

### Requirement: Overflow check per width and signedness

Lowering SHALL emit the overflow check corresponding to the width and signedness of the operation's integer type, reusing the mechanism Phase 1 established for `Int32`.

#### Scenario: Overflow checked at a narrow width
- **WHEN** an arithmetic operation on `Int8` is lowered
- **THEN** the IR includes the overflow check for that width, not the one for `Int32`

### Requirement: `NaN` as a controlled failure, not as a propagated value

Lowering SHALL include, for a `BinaryFloat` operation that can produce `NaN`
under the backend's semantics, the check that turns it into a controlled failure
before the value is used. Exact-decimal `Float` operations SHALL NOT emit a
`NaN` check
because `Float` has no `NaN`; instead they SHALL emit the coefficient-overflow
and zero-divisor checks appropriate to exact decimal arithmetic.

#### Scenario: Potentially indeterminate binary division

- **WHEN** `a / b` on `BinaryFloat64` is lowered without the checker being able
  to rule out `a == 0.0b && b == 0.0b`
- **THEN** the IR includes the corresponding check before producing the result

#### Scenario: Exact division does not emit a `NaN` check

- **WHEN** `a / b` on `Float` is lowered
- **THEN** the IR emits a zero-divisor guard and no `NaN` check

### Requirement: Desugaring of deep contextual conversion

Lowering SHALL insert the conversion of each operand of a contextual operator
tree before lowering the operation itself, instead of lowering the operation with
its original type and converting the result afterward. `Float(expr)` SHALL
convert operands to exact decimal; `BinaryFloat(expr)` SHALL convert operands to
binary.

#### Scenario: Division lowered with the exact context already applied

- **WHEN** `Float(3 / 4)` is lowered
- **THEN** the operands of the division are already exact `Float` in the
  resulting IR, and the result is `0.75`

#### Scenario: Division lowered with the binary context already applied

- **WHEN** `BinaryFloat(3 / 4)` is lowered
- **THEN** the operands of the division are already `BinaryFloat64` in the
  resulting IR

### Requirement: Desugaring of string interpolation

Lowering SHALL desugar a literal with interpolated expressions into a sequence of calls to `to_string()` concatenated with the literal text, in order of appearance.

#### Scenario: Interpolation with one expression
- **WHEN** `"Hello, {name}"` is lowered
- **THEN** the resulting IR calls `to_string()` on `name` and concatenates it with the literal text

### Requirement: String indexing lowers to a grapheme-offset instruction
A `String[index]` read expression SHALL lower to an instruction that computes the byte offset of the `index`-th grapheme and then extracts the grapheme.

#### Scenario: Valid string index
- **WHEN** the program contains `let c: Char = s[i]` with `s` of type `String`
- **THEN** the IR contains `StringGraphemeOffset` followed by `GraphemeLenAt` and `GraphemeSlice` to produce a `Char`

#### Scenario: Out-of-bounds string index
- **WHEN** the program contains `s[i]` and `i` is greater than or equal to the grapheme count
- **THEN** the IR branches to `throw_native_failure` with `IndexOutOfBoundsError`

#### Scenario: Negative string index rejected
- **WHEN** the program contains `s[i]` with `i` of a signed type and `i < 0`
- **THEN** the compiler reports a negative-index diagnostic before lowering

#### Scenario: String is not writable through index
- **WHEN** the program contains `s[i] = c`
- **THEN** the compiler reports `INDEXING_NOT_WRITABLE` or equivalent

### Requirement: IR supports dependent references and pinning
The language SHALL lower `Dependent<T>` construction to `DependentFrom` and `Pin<T>` construction to `PinObject`.

#### Scenario: Pin interior pointer
- **WHEN** `Pointer.from(o.field)` is used
- **THEN** the IR contains `PinObject(o)` before `PointerFromField`

#### Scenario: Dependent reference
- **WHEN** a `Dependent<T>` is created from a field
- **THEN** the IR contains `DependentFrom` with both the field pointer and the base object

### Requirement: IR supports pin release on all exits
The language SHALL lower an `UnpinObject` instruction on every exit path from the scope that owns the pin.

#### Scenario: Unsafe block returns normally
- **WHEN** an `unsafe` block containing a pinned object returns
- **THEN** the IR has `UnpinObject` before the return

#### Scenario: Unsafe block throws
- **WHEN** an `unsafe` block containing a pinned object throws
- **THEN** the IR unrolls the `UnpinObject` after journal rollback and before rethrow

### Requirement: Arithmetic operands are promoted to a common numeric type

When a binary arithmetic operation has operands of different numeric widths or families, lowering SHALL emit a widening conversion for each operand so that both have the same common type before the operation is performed. The common type is the smallest type that can represent every value of both operand types losslessly. For `++` and `--`, the literal `1` / `1.0` SHALL be created at the operand's exact type, so no promotion is needed.

#### Scenario: Integer operands of different widths
- **WHEN** `Int8 + Int32` is lowered
- **THEN** the `Int8` operand is widened to `Int32` before the addition

#### Scenario: Unsigned integer widened to a wider signed integer
- **WHEN** `UInt8 + Int32` is lowered
- **THEN** the `UInt8` operand is zero-extended to `Int32` before the addition

#### Scenario: Integer widened to a common float
- **WHEN** `Int32 + Float64` is lowered
- **THEN** the `Int32` operand is converted to `Float64` before the addition

#### Scenario: Unsafe integer-to-float conversion is rejected
- **WHEN** `Int32 + Float16` is lowered
- **THEN** compilation fails before lowering because `Float16` cannot losslessly represent every `Int32` value

#### Scenario: String repetition count widened
- **WHEN** `"x" * (3 as UInt8)` is lowered
- **THEN** the count is widened to `Int32` before the runtime call

#### Scenario: Increment uses the operand's own width
- **WHEN** `i++` on `Int8` is lowered
- **THEN** the constant `1` is created as `Int8` and no widening is emitted

### Requirement: Lowering of exact-decimal `Float` operations

Lowering SHALL represent an exact-decimal `Float` as a distinct IR type carrying
a 128-bit coefficient and a decimal scale, and SHALL carry a `Float` literal as
verbatim text parsed no earlier than the runtime/codegen boundary. Arithmetic,
comparison, rounding, and conversion on `Float` that requires scale alignment,
normalization, or rounding SHALL lower to dedicated runtime calls. A `Float`
division or modulo SHALL lower with a zero-divisor guard that raises
`DivisionByZeroError` before the runtime call, in the same way `Duration`
division is lowered.

#### Scenario: Exact addition lowers to a runtime call

- **WHEN** `0.1 + 0.2` on `Float` is lowered
- **THEN** the IR contains a call to the decimal-addition runtime helper with the
  two operand values, not an LLVM binary `fadd`

#### Scenario: Divide-by-zero guard precedes the call

- **WHEN** `a / b` on `Float` is lowered
- **THEN** the IR raises `DivisionByZeroError` when `b` is zero, before the
  decimal-division helper is invoked

#### Scenario: Integer operand converted to exact `Float`

- **WHEN** `Int32 + Float` is lowered
- **THEN** the `Int32` operand is converted to an exact `Float` at scale 0 before
  the addition

### Requirement: String repetition is a block-opening operation

`String * Int` SHALL be recognized as a block-opening expression in the same sense as `if`/`match`/`?.`/calls: its negative-count guard emits a `fail`/`cont` split, so every operand evaluated before the repetition — a sibling operand of an enclosing binary, a call argument, a field receiver — SHALL be held through a slot and reloaded on the continuation side rather than referenced across the branch.

#### Scenario: Repeat inside a concatenation
- **WHEN** `"x: " + "ab" * n` is lowered
- **THEN** the left operand is reloaded in the continuation block and the verifier accepts the function

#### Scenario: Repeat inside a call argument
- **WHEN** `f("a" * 3, other)` is lowered and `other` was computed first
- **THEN** `other` survives the guard branch through a slot and the verifier accepts the function

#### Scenario: Standalone repeat stays valid
- **WHEN** `stdout.println("ab" * 3)` is lowered as a whole statement
- **THEN** the verifier accepts the function and the printed value is `"ababab"`

