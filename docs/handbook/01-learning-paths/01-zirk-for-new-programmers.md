# Zirk for New Programmers

Programming is the practice of describing data, decisions, repetition, and effects precisely enough that a computer can perform them. Zirk helps by checking many relationships before a program runs, but the most important skill is learning to make those relationships explicit.

## Start with values and names

```zirk
inmut course = "Zirk basics";
mut completed_lessons = 0;

completed_lessons += 1;
```

A value such as a string or integer carries a type. A binding gives that value a name. `inmut` means the name cannot be assigned again; `mut` permits reassignment. Prefer `inmut` until change is part of the problem you are modeling.

## Organize behavior with functions

```zirk
fn square(value: Int32): Int32 {
    return value * value;
}
```

The function contract says what comes in and what returns. The compiler rejects `square("four")` before execution because a string does not satisfy the `Int32` parameter.

## Make decisions explicitly

```zirk
fn describe(score: Int32): String {
    if score >= 70 {
        return "passing";
    } else {
        return "keep practicing";
    }
}
```

Conditions must be Boolean; integers are not secretly true or false. Later, `match` will help you cover every case of a structured value.

## Follow this order

1. Program structure, bindings, everyday types, and nullability.
2. Operators, control flow, and functions.
3. Classes, interfaces, records, enums, generics, and collections.
4. Errors and resources before concurrency.
5. Projects, permissions, tests, and packages after the core language.

At each checkpoint, write a small program, run `zirk check`, deliberately introduce one type error, and read every part of the diagnostic. Being able to explain why invalid code is invalid is a stronger milestone than memorizing syntax.

---

**Previous:** [← Learning Paths](README.md) · **Next:** [ Zirk for TypeScript Programmers](02-zirk-for-typescript-programmers.md)
