# Zirk for Python Programmers

Zirk favors readable code and strong tooling, but its program model differs fundamentally from Python's dynamic runtime. Types are checked before native code is produced, imports are resolved as part of a compiled project, and implicit runtime flexibility is replaced by explicit unions, generics, interfaces, and conversions.

## Types exist before execution

```zirk
fn total(prices: List<Float64>): Float64 {
    return prices.reduce(0.0, (sum, price) => sum + price);
}
```

Inference reduces local annotation noise, but it never changes the fact that the compiler knows a concrete type. A value cannot acquire an unrelated type through reassignment. Lossy numeric conversions require deliberate operations.

## Absence is narrow

There is no universal `None`-like inhabitant and no `undefined`. `null` is valid only for `T?`. Code must narrow, safely access, match, or provide a fallback before treating the value as `T`.

## Objects have declared contracts

Classes, interfaces, traits, records, value classes, and enums serve different purposes. Prefer algebraic enums and `match` when the set of variants is closed. Prefer an interface or trait when multiple types share a behavior contract. Do not rely on accidental duck typing.

Slices accept Python-shaped omitted components, including `[::]`, `[n:w]`, and
`[::-1]`, but explicit out-of-range bounds are errors rather than clamped. A
slice is always an independent deep copy. Callable annotations use `Fn(P...) =>
R`, and closed-domain `match` is exhaustive with no pattern guards.

## Effects belong to the project

Filesystem, network, process, environment, and compile-time access are declared capabilities. A library declares requirements; the final application grants permissions. Native resources also have explicit cleanup contracts rather than depending on nondeterministic object finalization.

## Recommended route

Study bindings and inference first, then nullability, functions, declared data types, generics, errors, resources, and modules. Read the project and package chapters before translating a Python application structure: `init.zrk`, locked dependencies, native targets, and standalone output are central to Zirk rather than optional ecosystem conventions.

---

**Previous:** [← Zirk for TypeScript Programmers](02-zirk-for-typescript-programmers.md) · **Next:** [ Zirk for Rust, C, and C++ Programmers](04-zirk-for-rust-c-cpp-programmers.md)
