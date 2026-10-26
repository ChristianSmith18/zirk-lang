## ADDED Requirements

### Requirement: Type relation solving is deterministic and bounded
Subtype, assignability, and generic relation queries SHALL use canonical pair memoization scoped to one immutable analysis snapshot. A relation query SHALL stop after 100,000 memoized pair visits or recursion depth 256. Each union or intersection normalization SHALL stop after 256 source or flattened member visits; this bounds its pairwise candidate comparisons to at most 65,536. Exceeding a relation or normalization budget SHALL emit source-located `E0482` with the operation category and configured bound. The compiler SHALL NOT return a partial relation or normalized result, silently widen, or reuse facts across document generations.

#### Scenario: Relation budget is exceeded
- **WHEN** a type-relation query exceeds its configured visit or recursion limit
- **THEN** analysis returns `E0482` at the triggering type expression and does not produce a partial assignability answer

#### Scenario: Normalization budget is exceeded
- **WHEN** a union or intersection contains more than 256 source or flattened members
- **THEN** analysis returns source-located `E0482` and does not intern a partial normalized type

### Requirement: Type algebra defines sound assignability
The checker SHALL define assignability for primitive, singleton, structural, nominal, generic, union, intersection, nullable, Never, and Dynamic types. Union sources SHALL be assignable to a destination only when every possible source alternative fits. A value SHALL be assignable to a union destination when it fits at least one alternative. A value SHALL be assignable to an intersection destination only when it satisfies every constituent. Intersection-source reasoning SHALL use the combined proven members and SHALL NOT infer unrelated nominal identity. Dynamic SHALL remain explicitly runtime-tagged and safely narrowed; internal error-recovery Unknown SHALL NOT become a public unchecked any type.

#### Scenario: Union source needs every branch to fit
- **WHEN** String | Int32 is assigned to String | Int32 | Boolean
- **THEN** the assignment succeeds because every source alternative is admitted

#### Scenario: Union cannot be assigned to one constituent
- **WHEN** String | Int32 is assigned to String without narrowing
- **THEN** the checker rejects the assignment

#### Scenario: Intersection destination requires all members
- **WHEN** a value is assigned to Readable & Closeable
- **THEN** both structural requirements are checked

#### Scenario: Dynamic remains checked
- **WHEN** a Dynamic value is used as a concrete type without successful narrowing
- **THEN** the operation is rejected or performs the specified checked conversion

### Requirement: Singleton values widen only under explicit inference rules
Singleton values SHALL be assignable to their owning primitive or value type. Type inference SHALL preserve a singleton when required by an explicit singleton annotation, a singleton union alternative, a constant type expression, or an immutable literal context. Mutable bindings SHALL widen to their declared base type unless a narrower invariant is proven. Contextual typing SHALL NOT silently widen away a discriminant required by the expected type.

#### Scenario: Literal fits its primitive type
- **WHEN** the singleton 404 is assigned to Int32
- **THEN** assignment succeeds if the exact value is representable by Int32

#### Scenario: Context preserves a union discriminant
- **WHEN** { active: true } is checked against a union requiring active: true or active: false
- **THEN** contextual typing retains the true singleton while selecting the matching alternative

#### Scenario: Mutable variable widens safely
- **WHEN** mut active = true is declared without a singleton expectation
- **THEN** its inferred type permits later assignment of false

### Requirement: Conditional checks narrow local and property paths
Boolean conditions, null comparisons, and literal equality or inequality SHALL refine only the reachable branch's local or property path. The true and false branches SHALL remove only alternatives proven impossible by the condition. A nullable value compared with `null` SHALL narrow to the present type or `Null` on the corresponding branch. Comparing a finite singleton union with a literal SHALL preserve the exact matching alternative. The declared type SHALL remain unchanged after the branch.

#### Scenario: Boolean condition has complementary branch types
- **WHEN** a Boolean local is used directly as an `if` condition
- **THEN** the true arm sees `const(true)` and the false arm sees `const(false)`

#### Scenario: Null comparison narrows both outcomes
- **WHEN** a nullable local is compared with `null` using `==` or `!=`
- **THEN** the present branch sees the non-null type and the absent branch sees `Null`

#### Scenario: Literal equality removes only impossible alternatives
- **WHEN** a local of type `const(1) | const(2)` is compared with `1`
- **THEN** the equal branch sees `const(1)` and the unequal branch sees `const(2)`

### Requirement: Satisfies expressions preserve precise inferred types
An expression written as `value satisfies Target` SHALL be checked for assignability to `Target` and SHALL retain its original inferred type as the expression result. The operator SHALL not perform a runtime conversion or narrow the source expression. A failed relation SHALL produce the normal source-located type mismatch.

#### Scenario: Singleton type survives a satisfies check
- **WHEN** a value with singleton type `3` is checked using `value satisfies Int32`
- **THEN** the expression is accepted and its resulting type remains the singleton `3`

#### Scenario: Incompatible satisfies check fails
- **WHEN** a String expression is checked using `value satisfies Int32`
- **THEN** the checker reports a type mismatch at the satisfies expression

#### Scenario: Float singleton matching uses ordered equality
- **WHEN** a Float match compares negative zero with the `0.0` pattern
- **THEN** the arm matches in both the interpreter and native backend; NaN remains unavailable as a singleton and never matches

#### Scenario: Char singleton matching compares one grapheme
- **WHEN** a Char match uses an escaped or multi-code-point grapheme pattern
- **THEN** the interpreter and native backend select the same arm using canonical Char value equality

### Requirement: Common capabilities and fields are computed across unions
Before narrowing, member access on a union SHALL be allowed only for a compatible member available on every alternative. Read results SHALL be the normalized union of each alternative's result. A write SHALL satisfy every possible destination and mutability requirement. Callable common members SHALL use argument and result types that are safe for every alternative; incompatible signatures SHALL be unavailable until narrowing.

#### Scenario: Common read property
- **WHEN** each union alternative has a name: String property
- **THEN** value.name has type String before narrowing

#### Scenario: Common property has different literal values
- **WHEN** active is true in one alternative and false in another
- **THEN** the common read type is Boolean and a condition can narrow the receiver

#### Scenario: Member absent on one alternative
- **WHEN** a member exists on only some alternatives
- **THEN** direct member access is rejected until the value is narrowed

### Requirement: Type relations and inference remain bounded and deterministic
Type relation, normalization, generic inference, recursive aliases, and type-level computation SHALL use deterministic memoization and explicit resource budgets. Exceeding a budget SHALL produce a stable compile-time diagnostic with the responsible type expression; it SHALL NOT overflow the compiler stack, hang, or vary with hash iteration order.

#### Scenario: Deep recursive type is bounded
- **WHEN** a recursively composed type exceeds the documented depth or work budget
- **THEN** compilation reports the limit and source location without a crash

#### Scenario: Repeated relation is memoized
- **WHEN** the same complex assignability pair is checked repeatedly in one compilation
- **THEN** the result remains identical and the checker reuses its canonical relation result
