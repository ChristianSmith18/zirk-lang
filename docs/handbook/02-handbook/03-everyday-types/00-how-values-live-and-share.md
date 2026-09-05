# How Zirk Values Live and Share

Every Zirk value answers two questions: *what can I do with it?* and *what
happens when I copy it?* The first question is the type. The second question is
the **memory behavior** — whether the value is stored with you, copied on
assignment, or shared through a reference.

This chapter introduces the five memory positions that appear again and again in
the type chapters. Learn the spectrum first, and the individual types become
variations on a single idea.

## The five positions

| Position | Copy/Share rule | Representative types | Mental model |
|---|---|---|---|
| Immediate value | Assignment copies the whole value | `Int32`, `Boolean`, `Char`, `Float64` | A number in a box |
| Native value | Assignment copies an independent semantic value | `Date`, `Duration`, `record`, `Tuple` | An immutable payload |
| Managed reference | Assignment makes another path to the same instance | `String`, `Array<T>`, `List<T>` | A label on a shared object |
| Borrowed / dependent view | A short-lived, non-owning view | `Weak<T>`, `Dependent<T>`, `NativeSlice<T>` | A ticket that may expire |
| Unsafe pointer | Raw memory with no lifetime guarantee | `Pointer<T>` | An address and a prayer |

The compiler is free to optimize layout, cache, and allocation, but the
observable behavior of each position is fixed. A `String` always shares; a
record always copies.

## Position 1: Immediate values

```zirk
mut first: Int32 = 3;
mut second = first;
second += 1;

// first is 3, second is 4
```

`first` and `second` are independent. The same rule applies to `Boolean`,
`Char`, and every `Float` width. Immediate values have no identity, so `is` is
not meaningful for them.

## Position 2: Native values

```zirk
record Point { x: Float64; y: Float64; }

inmut a = Point(x: 1.0, y: 2.0);
inmut b = a;

// a and b are equal but independent
a == b; // true
```

`Date`, `Duration`, `record`, and `Tuple` are values: assigning one copies the
semantic content. No two names observe the same hidden state. You cannot take an
address of a value; you can only receive a new value or a projected copy.

## Position 3: Managed references

```zirk
mut text = "hello";
mut alias = text;
alias[0] = 'H';

// both names see "Hello"
text == "Hello"; // true
text is alias;   // true
```

`String`, `Array<T>`, `List<T>`, `Map<K,V>`, and `Set<T>` are managed
references. The runtime keeps the object alive while any live name points to it.
If you want an independent copy, call `clone()`.

## Position 4: Borrowed and dependent views

```zirk
mut numbers: List<Int32> = List(1, 2, 3);
mut slice = numbers[0:2]; // a view, not a new list

// Mutating the list invalidates the view if the contract requires it
```

`Weak<T>` does not keep its referent alive. `Dependent<T>` and
`NativeSlice<T>` are views whose validity is tied to a larger owner. They are
useful for zero-copy access, but they cannot outlive the value they observe.

## Position 5: Unsafe pointers

```zirk
unsafe {
    mut raw: Pointer<Int32> = some_native_address();
    *raw = 42;
}
```

`Pointer<T>` is a raw address. It may be null. It may dangle. It carries no
runtime ownership. Use it only in `unsafe` blocks or when interoperating with
native code.

## The same data, different contracts

A point in space can be represented in every position:

```zirk
record Point { x: Float64; y: Float64; }

inmut value = Point(x: 1.0, y: 2.0);          // independent copy
mut shared = class Point2D { x: Float64; y: Float64; }; // identity, shared
mut unsafe = Pointer<Point>(...);             // raw address
```

Choosing the position is the first step in choosing the type. The rest of this
unit repeats the same rule for each concrete type.

## What comes next

1. [Object and the Type Tree](01-object-and-type-hierarchy.md) places every
type under the conceptual `Object` root.
2. [Type Categories](01a-type-categories.md) maps the five positions to Zirk's
categories.
3. [Value and Reference Behavior](01b-value-and-reference-behavior.md) shows
what `==` and `is` mean across the spectrum.
4. [Choosing a Type](01f-choosing-a-type.md) turns a problem into a type choice.

---

**Previous:** [← Everyday Types](README.md) · **Next:** [ Object and the Type Tree](01-object-and-type-hierarchy.md)
