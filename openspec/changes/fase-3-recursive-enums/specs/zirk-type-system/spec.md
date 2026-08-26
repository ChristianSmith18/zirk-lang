## ADDED Requirements

### Requirement: An enum declaration may reference its own type

An enum's variant payload MAY name the enum's own type, including a self-instantiation of its own generic type parameters, and declaration order between an enum and a class or another enum that reference each other SHALL NOT matter.

#### Scenario: Non-generic recursive enum declares
- **WHEN** `enum IntList { Nil, Cons(head: Int32, tail: IntList) }` is declared
- **THEN** it declares successfully and `IntList` values can be constructed and pattern-matched at any depth

#### Scenario: Generic recursive enum declares and specializes
- **WHEN** `enum Tree<T> { Leaf, Node(value: T, left: Tree<T>, right: Tree<T>) }` is instantiated as `Tree<Int32>`
- **THEN** it declares successfully and the instantiation lowers to one concrete layout, reused for every recursive occurrence of `Tree<Int32>` within it
