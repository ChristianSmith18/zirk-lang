## MODIFIED Requirements

### Requirement: Loops

The parser SHALL recognize `for` with initialization/condition/increment, `for ... in` over an iterable expression, `while`, `do ... while`, and `loop`, together with `break` and `continue`, per `ZIRK_LANGUAGE_SPEC.md` section 5.

Parentheses around the header SHALL be optional in all of these forms. The canonical form omits them, and both SHALL produce the same tree.

#### Scenario: `for` with all three clauses
- **WHEN** `for mut i = 0; i < 10; i++ { }` is parsed
- **THEN** a loop with initialization, condition, and increment is produced

#### Scenario: `for` with parentheses
- **WHEN** `for (mut i = 0; i < 10; i++) { }` is parsed
- **THEN** the same tree as without parentheses is produced

#### Scenario: `for ... in`
- **WHEN** `for x in 0..10 { }` is parsed
- **THEN** a loop that iterates the variable `x` over the range is produced

#### Scenario: `while`
- **WHEN** `while x > 0 { }` or `while (x > 0) { }` is parsed
- **THEN** a conditional loop is produced in both cases

#### Scenario: `do ... while`
- **WHEN** `do { poll(); } while pending;` is parsed
- **THEN** a post-condition loop whose body precedes its condition is produced

#### Scenario: `loop`
- **WHEN** `loop { break; }` is parsed
- **THEN** an unconditional loop whose body contains `break` is produced

#### Scenario: `break` and `continue` outside a loop
- **WHEN** `break;` or `continue;` is parsed outside any loop
- **THEN** a diagnostic stating that they are only valid inside a loop is emitted

### Requirement: Conditional statement

The parser SHALL recognize `if` and `else` as a statement, with bodies in braces except in the effect form that governs a single statement. When both branches are present, `if`/`else` SHALL also be admitted as an expression, per `ZIRK_LANGUAGE_SPEC.md` section 5.

Whether it is a statement or an expression is not decided by the parser: it is decided by type checking, based on whether the usage requires a value and both branches are type-compatible.

Parentheses around the condition SHALL be optional.

#### Scenario: Simple conditional
- **WHEN** `if x > 0 { }` is parsed
- **THEN** a conditional statement with no alternative branch is produced

#### Scenario: Conditional with an alternative
- **WHEN** `if x > 0 { } else { }` is parsed
- **THEN** a conditional statement with both branches is produced

#### Scenario: Chaining
- **WHEN** `if a { } else if b { } else { }` is parsed
- **THEN** a conditional whose alternative branch is another conditional is produced

#### Scenario: Body without braces
- **WHEN** `if closed return;` is parsed
- **THEN** a conditional whose single branch is that statement is produced

#### Scenario: Alternative branch over a brace-free body
- **WHEN** a brace-free `if` is followed by `else`
- **THEN** a diagnostic stating that the brace-free form governs a single statement is emitted

#### Scenario: Condition in parentheses
- **WHEN** `if (x > 0) { }` is parsed
- **THEN** the same tree as without parentheses is produced

#### Scenario: Use as an expression
- **WHEN** `mut resultado = if x > 0 { "positivo" } else { "no positivo" };` is parsed
- **THEN** a declaration whose initializer is the conditional is produced

### Requirement: Compound assignment and increment

The parser SHALL recognize `+=`, `-=`, `*=`, `/=`, `%=`, `**=`, `++`, and `--` in statement position, expanding them to the tree of the equivalent assignment.

`++` and `--` SHALL also be admitted in expression position, preserving the conventional prefix and postfix semantics from `ZIRK_LANGUAGE_SPEC.md` section 4: the postfix form evaluates to the previous value and the prefix form to the already-incremented value. Both SHALL require an assignable and mutable place.

#### Scenario: Compound assignment
- **WHEN** `total += 5;` is parsed
- **THEN** the produced tree is equivalent to that of `total = total + 5;`

#### Scenario: Increment
- **WHEN** `i++;` or `++i;` is parsed
- **THEN** the produced tree is equivalent to that of `i = i + 1;`

#### Scenario: Postfix increment in expression position
- **WHEN** `mut x = i++;` is parsed
- **THEN** `x` receives the value of `i` prior to the increment

#### Scenario: Prefix increment in expression position
- **WHEN** `mut x = ++i;` is parsed
- **THEN** `x` receives the already-incremented value of `i`

#### Scenario: Increment over a non-assignable place
- **WHEN** `++`'s operand is not an assignable and mutable place
- **THEN** a diagnostic pointing at the operand is emitted

## ADDED Requirements

### Requirement: Optional parentheses in control headers

The parser SHALL admit optional parentheses around the header of every control structure -- `if`, `while`, `for`, `for ... in`, `do ... while`, and `match` -- producing the same tree with and without them.

The parenthesis-free form is the canonical one. No structure SHALL require them.

This rule was not written down in any normative source, and that absence is what led to requiring them in the traditional `for`.

#### Scenario: Header without parentheses
- **WHEN** `while pending { }` is parsed
- **THEN** parsing succeeds

#### Scenario: Header with parentheses
- **WHEN** `while (pending) { }` is parsed
- **THEN** parsing succeeds and produces the same tree

#### Scenario: `for ... in` with parentheses
- **WHEN** `for (x in 0..10) { }` is parsed
- **THEN** the same tree as `for x in 0..10 { }` is produced

#### Scenario: `match` with parentheses
- **WHEN** `match (valor) { }` is parsed
- **THEN** the same tree as `match valor { }` is produced

### Requirement: Ternary expression

The parser SHALL recognize the right-associative ternary expression `condition ? when_true : when_false`, per level 16 of the precedence table.

The ternary is the preferred compact form for choosing a short value, and it SHALL NOT replace `if` in expression position: both coexist.

#### Scenario: Simple ternary
- **WHEN** `mut label = active ? "yes" : "no";` is parsed
- **THEN** the initializer is a ternary expression with its three parts

#### Scenario: Nested ternary
- **WHEN** `a ? b : c ? d : e` is parsed
- **THEN** the nesting groups to the right

#### Scenario: Ternary without an alternative branch
- **WHEN** `:` and its expression are missing
- **THEN** a diagnostic pointing at the incomplete ternary is emitted

## REMOVED Requirements

### Requirement: Complete conditional and loop forms

**Reason**: It stated together four forms -- `if` as a value, brace-free `if`, the traditional `for`, and `do ... while` -- which belong to different requirements, and it did so without the rules this change needed to fix: what closes a parenthesis-free header, and why the brace-free form does not admit `else`.

**Migration**: The traditional `for` and `do ... while` move to *Loops*; `if` as a value and the brace-free `if`, to *Conditional statement*, which also fixes the rejection of `else` on the brace-free form; the preference for the ternary, to *Ternary expression*; and the optional parentheses, which no source had captured, to *Optional parentheses in control headers*.
