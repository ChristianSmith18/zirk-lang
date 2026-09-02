## MODIFIED Requirements

### Requirement: Constructs outside the subset

The parser SHALL emit a specific diagnostic for constructs that exist in the language but are not implemented yet, distinguishing them from syntax errors.

This phase removes from that list `class`, `construct`, `this`, `record`, `type`, `public`, `private`, `protected`, `abstract`, `implements`, `extends`, `from`, `as`, and `is`.

#### Scenario: Later-phase construct
- **WHEN** `try`, `task`, `parallel`, or `thread` is parsed
- **THEN** the diagnostic SHALL name the construct
- **AND** SHALL state that it is not implemented yet
- **AND** SHALL NOT be reported as an unexpected token

#### Scenario: `init.zrk` out of scope
- **WHEN** an `init.zrk` file is found
- **THEN** the diagnostic states that declarative project configuration arrives in a later phase

### Requirement: Nullability

The parser SHALL recognize `T?` as a type annotation, `null` as a literal, `??` as null coalescing, and `?.` as safe access, per `ZIRK_LANGUAGE_SPEC.md` section 4.

`?.` is no longer deferred: the previous phase postponed it because no type had members, and classes bring them.

#### Scenario: Nullable type
- **WHEN** `mut name: String? = null;` is parsed
- **THEN** a declaration with type `String?` and a null initializer is produced

#### Scenario: Null coalescing
- **WHEN** `name ?? "anonymous"` is parsed
- **THEN** an expression with a fallback is produced

#### Scenario: `??` precedence
- **WHEN** `a ?? b || c` is parsed
- **THEN** the tree represents `(a ?? b) || c`

#### Scenario: Safe access
- **WHEN** `user?.name` is parsed
- **THEN** a safe-access expression on the member is produced

## ADDED Requirements

### Requirement: Class syntax

The parser SHALL recognize `class`, its fields and methods, `construct`, `this`, the visibility modifiers, `abstract`, `extends`, and `implements`, per `ZIRK_LANGUAGE_SPEC.md` section 7.

#### Scenario: Full class
- **WHEN** `class User implements Serializable { public inmut id: Int32; construct(id: Int32) { this.id = id; } }` is parsed
- **THEN** a declaration with an implemented contract, one field, and one constructor is produced

#### Scenario: Combined inheritance and contracts
- **WHEN** `class Admin extends User implements Auditable, Cloneable { }` is parsed
- **THEN** a declaration with one superclass and two contracts is produced

#### Scenario: `construct` outside a class
- **WHEN** `construct` appears at the file's top level
- **THEN** a diagnostic stating that a constructor belongs to a class is emitted

#### Scenario: Several constructors and named arguments
- **WHEN** a class declares several `construct` and is constructed with reordered named arguments
- **THEN** the tree keeps every signature and every argument's label for semantic resolution

### Requirement: Contract syntax

The parser SHALL recognize `interface` and `trait` with their methods, and admit a body only in a `trait`'s.

#### Scenario: Interface
- **WHEN** `interface Serializable { fn serialize(): String; }` is parsed
- **THEN** a declaration with a bodiless signature is produced

#### Scenario: Trait with implementation
- **WHEN** `trait Greet { fn hello(): String { return "hello"; } }` is parsed
- **THEN** a declaration whose method has a body is produced

### Requirement: Generics syntax

The parser SHALL recognize type parameters `<T>` in declarations, type arguments at use sites, and constraints with `from`.

#### Scenario: Generic function with a constraint
- **WHEN** `fn serialize<T from Serializable>(value: T): String { }` is parsed
- **THEN** a type parameter with a constraint is produced

#### Scenario: Several type parameters
- **WHEN** `class Map<K, V> { }` is parsed
- **THEN** two type parameters are produced

#### Scenario: Type argument at the use site
- **WHEN** `mut b: Box<Int32>;` is parsed
- **THEN** the type carries an argument

#### Scenario: `<` that does not open generics
- **WHEN** `a < b` is parsed
- **THEN** a comparison is produced, not a type argument

### Requirement: Data type syntax

The parser SHALL recognize `record`, value classes, enum variants with associated data, unions `A | B`, and aliases with `type`.

#### Scenario: Enum with associated data
- **WHEN** `enum Shape { Circle(Int32), Rect(Int32, Int32) }` is parsed
- **THEN** two variants are produced with one and two associated types respectively

#### Scenario: Traditional enum mapping
- **WHEN** `enum Direction { North -> "N", South }` is parsed
- **THEN** `North` keeps its explicit mapping and `South` has no explicit mapping

#### Scenario: Pattern with destructuring
- **WHEN** `match s { Shape.Circle(r) => r, _ => 0 }` is parsed
- **THEN** the pattern binds a name to the associated value

#### Scenario: Union
- **WHEN** `mut x: Int32 | String;` is parsed
- **THEN** a type with two members is produced

#### Scenario: Alias
- **WHEN** `type Id = Int32;` is parsed
- **THEN** an alias declaration is produced

### Requirement: Phase 3 does not yet parse the final function type

The Phase 3 parser SHALL NOT yet accept the final syntax
`Function(Int32, Int32) => Int32` nor its alias `Fn(Int32, Int32) => Int32`.

This is a temporary restriction of the Phase 3 compiler. The syntax, signature
compatibility, and final escaping are already decided in the canonical checkpoint.

#### Scenario: Function type in an annotation
- **WHEN** a type annotation shaped like a function signature is parsed
- **THEN** a diagnostic stating that function types arrive in a later phase is emitted

#### Scenario: The lambda as an expression is unchanged
- **WHEN** `(a: Int32): Int32 => a + 1` is parsed in value position
- **THEN** a lambda is produced, the same as in the previous phase

### Requirement: Cast syntax

The parser SHALL recognize the postfix form `expr as T` and the prefix form `<T>expr`, per `ZIRK_LANGUAGE_SPEC.md` section 11.

#### Scenario: Postfix cast
- **WHEN** `mut v = source as String;` is parsed
- **THEN** a conversion to the named type is produced

#### Scenario: Prefix cast
- **WHEN** `mut v = <String>source;` is parsed
- **THEN** the same conversion as the postfix form is produced

#### Scenario: Cast that reinterprets memory
- **WHEN** a cast requiring `unsafe` is parsed
- **THEN** a diagnostic stating that the low-level tier arrives in a later phase is emitted
