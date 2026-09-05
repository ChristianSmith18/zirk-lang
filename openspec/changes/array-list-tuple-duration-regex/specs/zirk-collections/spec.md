# zirk-collections Specification

## Purpose

Defines native collections and product types: `Tuple`, `Array<T>`, and `List<T>`.

## ADDED Requirements

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

`Range<T>` SHALL represent a sequence from `start` to `end` with an optional `step`. It SHALL be constructible with `start..end` and `start..end..step`. `Range<T>` SHALL satisfy `Iterable<T>` for numeric `T` and `Duration`. It SHALL support `reverse()` and slicing.

#### Scenario: Numeric range
- **WHEN** `for i in 0..5 { stdout.println(i); }` is executed
- **THEN** it prints `0` through `4`

#### Scenario: Range with step
- **WHEN** `for i in 0..10..2 { stdout.println(i); }` is executed
- **THEN** it prints `0`, `2`, `4`, `6`, `8`

#### Scenario: Range of Duration
- **WHEN** `for d in 0s..5s..1s { stdout.println(d); }` is executed
- **THEN** it prints five `Duration` values from `0s` to `4s`

### Requirement: Remove value class references

Any collection example or test that used `value class` for domain types SHALL be migrated to `record` or `class` as appropriate. The compiler SHALL no longer recognize the `value class` declaration form.

#### Scenario: Collection of domain values
- **WHEN** `record UserId { value: UInt64; }` is used in a `List<UserId>`
- **THEN** the program compiles and the list contains independent `UserId` values
