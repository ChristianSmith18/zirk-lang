# zirk-collections Specification

## Purpose
Defines native collection types, copying and mutation rules, slicing,
iteration validity, complexity, and collection capabilities.

## Requirements

### Requirement: Standard collection families are deterministic and ergonomic
The standard library SHALL provide fixed `Array`, automatically growing `List`,
insertion-ordered `Map`/`Set`, `Deque`, `PriorityQueue`, restricted
`Queue`/`Stack`, and finite `Range`. List allocation capacity, reservation, and
shrinking SHALL remain implementation details rather than public source APIs;
exact fixed storage SHALL use `Array`.

#### Scenario: List grows
- **WHEN** values are added beyond the list's current internal allocation
- **THEN** the runtime grows storage automatically or returns a typed allocation
  failure without requiring a capacity operation from source code

### Requirement: Eager collections and lazy iterators have an explicit boundary
Collection transformations SHALL materialize eagerly, iterator adapters SHALL
remain single-pass and lazy, and materialization SHALL require explicit
`collect` or a typed `to_*` terminal. Task-aware streams SHALL NOT implement an
iterator contract that hides `await`, and standard `Range` SHALL require a
finite end.

#### Scenario: Lazy map becomes a list
- **WHEN** an iterator pipeline ends with `collect()` in a `List<T>` context
- **THEN** it consumes once, materializes a list, and reports allocation failure
  through the documented channel

### Requirement: Collection families and alias boundary
Array and fixed `T[n]` SHALL be ordered fixed-size references, List an ordered dynamic reference, Map a key/value reference without implicit order, Set a unique-membership reference without implicit order, Range a lazy value, and Tuple a heterogeneous value. Whole reference assignment SHALL alias; extraction SHALL deep-clone.

#### Scenario: List element extraction
- **WHEN** `mut first = users[0]` extracts a User and first is mutated
- **THEN** the stored users[0] remains unchanged

#### Scenario: Indexed place mutation
- **WHEN** `users[0].name = "Grace"` is assigned
- **THEN** the User stored in the list is mutated

### Requirement: Collection `length` is usable in loop-bound comparisons

Every collection `length` member SHALL return a non-negative integer type that participates in the language's numeric comparison rules, so that the idiomatic `for mut i = 0; i < xs.length; i++` loop compiles and runs without explicit casts or counter-type annotations. The declared `length` types across `Array`, `List`, `Map`, and `Set` SHALL be consistent: either all signed or all unsigned, and the handbook SHALL document the same type for each.

#### Scenario: Index loop over an array
- **WHEN** `for mut i = 0; i < courses.length; i++ { stdout.println(courses[i]); }` is compiled
- **THEN** no type-mismatch diagnostic is emitted and each element is printed in order

#### Scenario: Index loop over a list
- **WHEN** the same loop pattern iterates a `List<T>` by index
- **THEN** it compiles and runs identically to the array case

#### Scenario: Documented `length` types agree
- **WHEN** the handbook entries for `Array`, `List`, `Map`, and `Set` are compared
- **THEN** they declare `length` with a consistent signedness across all four families

### Requirement: Indexing and safe access
String, Tuple, Array, and List SHALL accept normalized negative indexes. Direct missing index/key access SHALL produce a typed controlled exception. Safe `get` on Array/List SHALL return `Result<T, IndexOutOfBoundsError>` and safe `get` on Map SHALL return `Result<V, LookupError>`. These SHALL be the sole safe lookup methods for those operations; `get_or_null` SHALL NOT be published. In a nullable value context, `get` SHALL follow the language-wide Result projection rule and produce `T?` or `V?`; an explicit Result context or `match` SHALL preserve the typed lookup error.

#### Scenario: Dynamic heterogeneous tuple index
- **WHEN** a nonconstant variable indexes a heterogeneous Tuple
- **THEN** compilation fails because one result type cannot be selected

#### Scenario: A safe sequence lookup finds an element
- **WHEN** `items.get(index)` addresses an element within the normalized bounds
- **THEN** the operation produces `Ok(element)` and a nullable value context receives that element

#### Scenario: A safe sequence lookup is out of bounds
- **WHEN** `items.get(index)` addresses no element
- **THEN** an explicit Result context receives `Error(IndexOutOfBoundsError)` and a nullable value context receives `null`

#### Scenario: A safe map lookup misses
- **WHEN** `map.get(key)` has no matching key
- **THEN** an explicit Result context receives `Error(LookupError)` and a nullable value context receives `null`

#### Scenario: Collection lookup preserves its error on request
- **WHEN** a caller matches `items.get(index)` or stores it as `Result<T, IndexOutOfBoundsError>`
- **THEN** the original lookup success or typed error remains available

#### Scenario: Removed nullable lookup alias
- **WHEN** source calls `items.get_or_null(index)` or `map.get_or_null(key)`
- **THEN** compilation reports that the member is unavailable and recommends `get`, nullable projection, `??`, or `match`

### Requirement: Python-style slice components with strict bounds
String, Array, and List slices SHALL accept `[start:end:step]` with omitted positive defaults `(0,length,1)` and omitted negative defaults `(last,before-first,negative step)`. End SHALL be exclusive, zero step and explicitly out-of-range bounds SHALL fail, and slices SHALL deep-copy independent results. Tuple, Map, and Set SHALL not slice.

#### Scenario: Complete reverse slice
- **WHEN** `values[::-1]` is evaluated
- **THEN** it returns an independent reverse-order copy

#### Scenario: Omitted ordinary step
- **WHEN** `values[n:w]` is evaluated
- **THEN** its step is one

### Requirement: Slice replacement and structural APIs
Slice assignment SHALL require exactly equal element count and SHALL never resize. Array SHALL reject structural growth; List SHALL use explicit add/insert/remove/splice operations. Mutating operations SHALL require a non-strict mutable referent and SHALL report typed allocation/capacity errors.

#### Scenario: Unequal slice replacement
- **WHEN** a two-element slice is assigned three replacements
- **THEN** a controlled slice-length-mismatch error is produced without partial mutation

### Requirement: Iteration, invalidation, views, and equality
Iteration SHALL use `Iteration<T>.Item/Done`, yield independent copies, and make `Iterable<out T>` create a new iterator. Structural mutation SHALL invalidate existing iterators deterministically. Ordinary views SHALL be explicit read-only shared storage with bounded lifetime. Collection equality SHALL follow order for arrays/lists, mappings for Map, membership for Set, and range definition for Range.

#### Scenario: Structural invalidation
- **WHEN** a List grows after an iterator is created and that iterator advances
- **THEN** it produces IteratorInvalidatedError rather than observing stale storage

### Requirement: Collection literal construction

`Array<T>` and `List<T>` literals and constructors SHALL accept scalar elements and explicit spread elements/arguments. Each spread source SHALL implement `Iterable<T>`, SHALL be consumed once, and SHALL expand in place before final allocation. Unannotated literals retain their existing default collection type; destination context selects `List<T>` when requested.

#### Scenario: Array constructor spread
- **WHEN** `Array(...values)` is constructed from `[1, 2, 3]`
- **THEN** the array contains three copied elements in order

#### Scenario: List literal spread
- **WHEN** `inmut values: List<Int32> = [...source]` is constructed
- **THEN** the list contains every source element and remains independently resizable

### Requirement: Negative indexing counts from the end

`Array<T>`, `List<T>`, and `String` index/slice reads and writes SHALL
accept negative indices, resolved as `length + index`.

#### Scenario: Last element

- **WHEN** `a[-1]` is read on a three-element array
- **THEN** it yields the third element

#### Scenario: Still bounds checked

- **WHEN** `a[-4]` is read on a three-element array
- **THEN** a controlled bounds error is raised

### Requirement: Collection clone and rendering

`Array<T>` and `List<T>` SHALL expose `clone()` producing an independent collection and `to_string()` producing `[e0, e1, …]`. `Map<K, V>`, `Set<T>`, and `Range<T>` SHALL expose deterministic `{k: v, …}`, `{e, …}`, and literal-mirroring rendering. All collection types SHALL be valid in print/println and string interpolation.

#### Scenario: Clone separates

- **WHEN** `l.clone()` is taken and the clone is mutated
- **THEN** the source list is unchanged

#### Scenario: Rendering

- **WHEN** `List(1, 2).to_string()` runs
- **THEN** the result is `[1, 2]`

#### Scenario: Map rendering
- **WHEN** `Map<Int32, String>` holds keys `2` and `1`
- **THEN** it renders `{1: a, 2: b}` in deterministic order

### Requirement: Array slicing

`Array<T>` SHALL support `a[start:end:step]` producing a new `Array<T>`
copy in safe code, with the same bound/step rules as `String` slicing.

#### Scenario: Middle slice

- **WHEN** `a[1:4]` is taken from a five-element array
- **THEN** the result is an `Array<T>` of three copied elements

### Requirement: List value removal

`List<T>` SHALL expose `remove(value)` removing the first equal element
and returning `Boolean` when `T` supports equality, alongside the existing
`remove(index)`.

#### Scenario: Value removal

- **WHEN** `List(1, 2).remove(2)` runs
- **THEN** it returns `true` and the list holds `[1]`

### Requirement: Tuple value product type

The language SHALL provide a `Tuple(A, B, ...)` value type constructed with `(a, b, ...)` and indexed only by a compile-time constant. Tuples SHALL copy as independent values and SHALL support destructuring and direct pattern matching.

#### Scenario: Tuple construction and indexing
- **WHEN** `inmut t: Tuple(Int32, String) = (42, "x");` is declared
- **THEN** `t[0]` is `Int32` and `t[1]` is `String`

#### Scenario: Tuple destructuring
- **WHEN** `inmut (n, s) = (42, "x");` is evaluated
- **THEN** `n` is `42` and `s` is `"x"`

#### Scenario: Tuple in match
- **WHEN** `match t { (0, _) => {} (n, s) => {} }` is evaluated
- **THEN** the matching branch is selected and binds `n` and `s`

### Requirement: Array type and construction

`Array<T>` SHALL be a contiguous native reference container. It SHALL support construction with explicit capacity, indexed read and write, `length`, `is_empty`, and `for ... in` iteration through `Iterable<T>`. Bounds access SHALL produce a controlled error.

#### Scenario: Array construction and indexing
- **WHEN** `mut a: Array<Int32> = Array<Int32>(3); a[0] = 1; a[1] = 2;` is executed
- **THEN** `a[0] == 1` and `a[1] == 2` and `a.length == 3`

#### Scenario: Array bounds check
- **WHEN** `a[10]` is read on an `Array` of length 3
- **THEN** an `IndexOutOfBoundsError` is produced

#### Scenario: Array iteration
- **WHEN** `for x in a { stdout.println(x); }` is executed
- **THEN** each element is printed in order

### Requirement: List type and resizable operations

`List<T>` SHALL be a resizable native reference container. It SHALL support `add(value)`, `insert(index, value)`, `remove(index)`, `remove(value)`, indexed read/write, `length`, `is_empty`, and `for ... in`.

#### Scenario: List add and remove
- **WHEN** `mut l: List<Int32> = List<Int32>(); l.add(1); l.add(2); l.remove(0);` is executed
- **THEN** `l[0] == 2` and `l.length == 1`

#### Scenario: List insertion
- **WHEN** `l.insert(0, 5);` is executed on a list containing `[1, 2]`
- **THEN** the list becomes `[5, 1, 2]`

### Requirement: Iterable and Iterator for collections

`Array<T>`, `List<T>`, `String`, and `Range` SHALL satisfy `Iterable<T>` and produce an `Iterator<T>` with `next(): T?` and `has_next(): Boolean`.

#### Scenario: Manual iteration
- **WHEN** `mut it = a.iterator();` is called on an `Array<Int32>`
- **THEN** `it.next()` returns the first element, then subsequent elements, then `null`

### Requirement: Range<T> generic range and iteration

`Range<T>` SHALL represent a finite arithmetic sequence from `start` to `end`.
`start..end` SHALL exclude the endpoint and `start..=end` SHALL include it.
An optional step SHALL use the colon form `start..end:step` or
`start..=end:step`; the legacy second-`..` step form SHALL be rejected.
Omitted steps SHALL follow the bounds (`+1` ascending, `-1` descending);
non-literal operands SHALL be braced. Zero steps fail with
`InvalidStepError` and contradictory direction fails with
`InvalidRangeDirectionError` before iteration or allocation. Valid elements
are integer scalar families and `Duration`. `Range<T>` SHALL satisfy
`Iterable<T>`, support slicing, and expand in `[...]`, `Array(...)`, and
`List(...)`. Range builder methods are not supported.

#### Scenario: Numeric range
- **WHEN** `for i in 0..5 { stdout.println(i); }` is executed
- **THEN** it prints `0` through `4`

#### Scenario: Range with step
- **WHEN** `for i in 0..10:2 { stdout.println(i); }` is executed
- **THEN** it prints `0`, `2`, `4`, `6`, `8`

#### Scenario: Range of Duration
- **WHEN** `for d in 0s..5s:1s { stdout.println(d); }` is executed
- **THEN** it prints five `Duration` values from `0s` to `4s`

#### Scenario: Descending range infers a negative step

- **WHEN** `for i in 2..0 { stdout.println(i); }` is executed
- **THEN** it prints `2`, then `1`

#### Scenario: Contradictory constant step

- **WHEN** `for i in 0..10:-1 { }` is written
- **THEN** a compile-time diagnostic states the step moves away from the range end

#### Scenario: Interpolated bound

- **WHEN** `inmut end: Int32 = 3;` and `for i in 0..{end} { }` are written
- **THEN** the bound is evaluated once and the loop iterates `0`, `1`, and `2`

#### Scenario: Range constructor expansion

- **WHEN** `List(0..=2)` is constructed
- **THEN** it is equivalent to `List(0, 1, 2)`

### Requirement: Remove value class references

Any collection example or test that used `value class` for domain types SHALL be migrated to `class`, `Object`, tuples, or enums according to whether it needs identity, open properties, positional values, or a closed set of alternatives. The compiler SHALL no longer recognize the `value class` declaration form.

#### Scenario: Collection of domain values
- **WHEN** a class representing a domain identity is used in a `List<UserId>`
- **THEN** the program compiles and list elements retain their class reference identity

### Requirement: Collection representation is checked before code generation

Every collection element type accepted by semantic analysis SHALL have defined storage, equality or hashing where applicable, ABI conversion, and GC tracing behavior. Unsupported combinations SHALL fail in semantic analysis with a diagnostic naming the type and operation.

#### Scenario: Unsupported map value is rejected early
- **WHEN** a map operation uses a value representation that has no runtime encoding
- **THEN** semantic analysis reports the unsupported representation before IR or LLVM emission

#### Scenario: Supported composite element is lowered
- **WHEN** a supported composite element type is used in an array or list
- **THEN** lowering and code generation preserve its layout and managed-reference paths

### Requirement: Collections preserve dynamic object identity and tracing
Collections SHALL retain dynamic `Object` values as managed references. Copying a collection element SHALL preserve the referenced object's identity; collection growth, slicing, cloning, replacement, and garbage collection SHALL keep all reachable dynamic property values valid alongside references in tuples, enums, nullable values, and closures.

#### Scenario: List retains an object value
- **WHEN** a dynamic object is inserted into a list and the original binding goes out of scope
- **THEN** the object remains reachable through the list

#### Scenario: Collection copy preserves object identity
- **WHEN** a collection containing an object is copied under the collection's ordinary copy rules
- **THEN** any copied element still refers to the same object identity
