## MODIFIED Requirements

### Requirement: Collection families and alias boundary
Array and fixed `T[n]` SHALL be ordered fixed-size references, List an ordered dynamic reference, Map and Set insertion-ordered references, Deque an ordered double-ended sequence reference, PriorityQueue a priority-ordered reference with stable tie ordering, Queue a FIFO-restricted reference, Stack a LIFO-restricted reference, Range a lazy value, and Tuple a heterogeneous value. Whole reference assignment SHALL alias; extraction SHALL follow the declared value/reference semantics and SHALL deep-clone where projection semantics require an independent value. Map and Set iteration SHALL preserve insertion order independently of their internal hash-table order.

#### Scenario: List element extraction
- **WHEN** `mut first = users[0]` extracts a User and first is mutated
- **THEN** the stored users[0] remains unchanged

#### Scenario: Indexed place mutation
- **WHEN** `users[0].name = "Grace"` is assigned
- **THEN** the User stored in the list is mutated

#### Scenario: Map and Set preserve insertion order
- **WHEN** entries or elements are inserted into a map or set and then iterated
- **THEN** they are observed in insertion order regardless of hash seed or bucket layout

### Requirement: Collection `length` is usable in loop-bound comparisons
Every collection `length` member SHALL return `UInt64`, representing a non-negative element, entry, or item count, and SHALL participate in the language's numeric comparison rules so that the idiomatic `for mut i = 0; i < xs.length; i++` loop compiles and runs without explicit casts or counter-type annotations. The declared `length` types across `Array`, `List`, `Map`, and `Set` SHALL all be `UInt64`, and the handbook SHALL document that type for each.

#### Scenario: Index loop over an array
- **WHEN** `for mut i = 0; i < courses.length; i++ { stdout.println(courses[i]); }` is compiled
- **THEN** no type-mismatch diagnostic is emitted and each element is printed in order

#### Scenario: Index loop over a list
- **WHEN** the same loop pattern iterates a `List<T>` by index
- **THEN** it compiles and runs identically to the array case

#### Scenario: Map and Set lengths use UInt64
- **WHEN** `length` is read from a `Map<K, V>` or `Set<T>`
- **THEN** the expression type is `UInt64` and its value is the number of present associations or unique elements

#### Scenario: Documented `length` types agree
- **WHEN** the handbook entries for `Array`, `List`, `Map`, and `Set` are compared
- **THEN** each declares `length` as `UInt64`

## ADDED Requirements

### Requirement: Deque provides complete double-ended sequence operations
`Deque<T>()` SHALL construct an empty generic managed reference sequence for every element type with defined collection storage, copy, and tracing semantics. It SHALL provide `push_front(T): Void`, `push_back(T): Void`, `pop_front(): Result<T, EmptyCollectionError>`, `pop_back(): Result<T, EmptyCollectionError>`, `front(): Result<T, EmptyCollectionError>`, `back(): Result<T, EmptyCollectionError>`, `length: UInt64`, `is_empty: Boolean`, `clear(): Void`, and insertion-order iteration. Growth failure SHALL raise catchable `AllocationError` before mutation; structural mutation SHALL invalidate existing iterators.

#### Scenario: Deque operates at both ends
- **WHEN** values are pushed and popped at the front and back
- **THEN** each operation returns the value at the requested end and remaining values preserve sequence order

#### Scenario: Empty deque construction uses its expected type
- **WHEN** `Deque()` initializes a value declared as `Deque<Int32>`
- **THEN** the expression resolves to an empty `Deque<Int32>` without an explicit runtime type argument

#### Scenario: Empty deque operations return typed outcomes
- **WHEN** either pop or end inspection is called on an empty deque
- **THEN** the operation returns `Error(EmptyCollectionError)` without mutating the deque

#### Scenario: Deque allocation failure leaves contents intact
- **WHEN** a growth operation cannot allocate storage
- **THEN** it raises `AllocationError` and leaves the prior sequence unchanged

### Requirement: PriorityQueue has a stable, explicit priority order
`PriorityQueue<T>()` SHALL construct an empty collection for values with a defined total ordering and expose `enqueue(T): Void`, `peek(): Result<T, EmptyCollectionError>`, `dequeue(): Result<T, EmptyCollectionError>`, `length: UInt64`, `is_empty: Boolean`, and iteration in dequeue order. The least value under the type's natural total ordering SHALL be dequeued first. Values that compare equal SHALL be dequeued in insertion order. Enqueue allocation failure SHALL raise catchable `AllocationError` without partial mutation; structural mutation SHALL invalidate existing iterators.

#### Scenario: Priority queue removes the least value first
- **WHEN** values with different priorities are enqueued and removed
- **THEN** each dequeue returns the least remaining value under the defined ordering

#### Scenario: Equal-priority items remain stable
- **WHEN** multiple distinguishable values compare equal and are enqueued in sequence
- **THEN** those values are dequeued in their original insertion order

#### Scenario: Priority queue empty outcome is typed
- **WHEN** `peek` or `dequeue` is called on an empty priority queue
- **THEN** it returns `Error(EmptyCollectionError)` and leaves the queue empty

#### Scenario: Priority queue construction preserves its element type
- **WHEN** `PriorityQueue()` initializes a value declared as `PriorityQueue<Int32>`
- **THEN** it resolves to an empty queue whose ordering is the natural total order of `Int32`

#### Scenario: Priority queue rejects an unordered element type
- **WHEN** a priority queue is instantiated for a type with no defined total order
- **THEN** semantic analysis reports the missing ordering requirement before lowering

#### Scenario: Priority queue allocation failure is atomic
- **WHEN** enqueue cannot allocate storage for an item
- **THEN** it raises `AllocationError` and leaves all existing priorities and stable tie order unchanged

### Requirement: Queue exposes FIFO-only operations
`Queue<T>()` SHALL construct an empty collection for every element type with defined collection storage, copy, and tracing semantics and SHALL expose `enqueue(T): Void`, `dequeue(): Result<T, EmptyCollectionError>`, `peek(): Result<T, EmptyCollectionError>`, `length: UInt64`, `is_empty: Boolean`, `clear(): Void`, and FIFO iteration. It SHALL NOT expose operations that remove or inspect the newest element at the back. Structural mutation SHALL invalidate existing iterators; growth failure SHALL raise catchable `AllocationError` and leave its contents unchanged.

#### Scenario: Queue removes in FIFO order
- **WHEN** several values are enqueued and then dequeued
- **THEN** values are returned in enqueue order

#### Scenario: Queue construction uses its expected type
- **WHEN** `Queue()` initializes a value declared as `Queue<String>`
- **THEN** the expression resolves to an empty `Queue<String>`

#### Scenario: Queue empty outcome is typed
- **WHEN** `dequeue` or `peek` is called on an empty queue
- **THEN** it returns `Error(EmptyCollectionError)` without changing the queue

#### Scenario: Queue does not expose deque-only operations
- **WHEN** source attempts to remove an item from the newest end of a `Queue<T>`
- **THEN** semantic analysis reports that the FIFO-only operation is unavailable

### Requirement: Stack exposes LIFO-only operations
`Stack<T>()` SHALL construct an empty collection for every element type with defined collection storage, copy, and tracing semantics and SHALL expose `push(T): Void`, `pop(): Result<T, EmptyCollectionError>`, `peek(): Result<T, EmptyCollectionError>`, `length: UInt64`, `is_empty: Boolean`, `clear(): Void`, and iteration from top to bottom. It SHALL NOT expose operations that remove or inspect an arbitrary position. Structural mutation SHALL invalidate existing iterators; growth failure SHALL raise catchable `AllocationError` and leave its contents unchanged.

#### Scenario: Stack removes in LIFO order
- **WHEN** several values are pushed and then popped
- **THEN** values are returned in reverse push order

#### Scenario: Stack construction uses its expected type
- **WHEN** `Stack()` initializes a value declared as `Stack<Int32>`
- **THEN** the expression resolves to an empty `Stack<Int32>`

#### Scenario: Stack empty outcome is typed
- **WHEN** `pop` or `peek` is called on an empty stack
- **THEN** it returns `Error(EmptyCollectionError)` without changing the stack

#### Scenario: Stack does not expose indexed mutation
- **WHEN** source attempts indexed removal or insertion on a `Stack<T>`
- **THEN** semantic analysis reports that the restricted operation is unavailable

### Requirement: New collection families publish bounded operation costs
With constant-cost comparison, hashing, and equality, `Deque` end insertion, removal, and inspection SHALL be amortized `O(1)`; `Queue` enqueue, dequeue, and peek SHALL be amortized `O(1)`; and `Stack` push, pop, and peek SHALL be amortized `O(1)`. `PriorityQueue` enqueue and dequeue SHALL be `O(log n)`, peek SHALL be `O(1)`, and priority-ordered iteration SHALL be `O(n log n)` with no more than `O(n)` temporary storage. `Map`/`Set` lookup, insertion, and removal SHALL be expected `O(1)`; traversal SHALL be `O(n)`; and set algebra SHALL be expected `O(n + m)`. The handbook SHALL publish these bounds and identify operations that allocate.

#### Scenario: Sequence end-operation costs are bounded
- **WHEN** collection complexity is checked for Deque, Queue, and Stack
- **THEN** end operations and inspection meet the documented amortized constant-time bounds

#### Scenario: Priority operations and ordered traversal costs are bounded
- **WHEN** a priority queue enqueues, dequeues, peeks, or iterates `n` values
- **THEN** each operation meets its documented `O(log n)`, `O(1)`, or `O(n log n)` bound and iteration uses at most `O(n)` temporary storage

#### Scenario: Hash collection costs are documented
- **WHEN** Map/Set operation costs are reviewed for constant-cost keys/elements
- **THEN** lookup and mutation are expected `O(1)`, traversal is `O(n)`, and set algebra is expected `O(n + m)`

### Requirement: New collection families render in their observable order
`Deque<T>`, `Queue<T>`, `Stack<T>`, and `PriorityQueue<T>` SHALL support `to_string()`, printing, and string interpolation. Rendering SHALL use the same order as iteration: front to back for Deque, FIFO order for Queue, top to bottom for Stack, and dequeue order for PriorityQueue. Rendering SHALL not mutate the collection and SHALL use each element's standard string conversion.

#### Scenario: Deque and Queue rendering follows sequence order
- **WHEN** Deque or Queue contains multiple values and is converted to text
- **THEN** the text lists values in front-to-back or FIFO iteration order without changing the collection

#### Scenario: Stack and PriorityQueue rendering follows iteration order
- **WHEN** Stack or PriorityQueue contains multiple values and is converted to text
- **THEN** the text lists values top-to-bottom or least-priority-first, respectively, without changing the collection

#### Scenario: New collections work in interpolation
- **WHEN** a collection value is interpolated into a string or passed to `print`/`println`
- **THEN** its deterministic collection representation is emitted and its contents remain unchanged
