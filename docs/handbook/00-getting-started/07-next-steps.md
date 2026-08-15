# Next Steps

You now know Zirk's intended scope, its status model, the shape of a minimal project, and the compiler pipeline. The best next chapter depends on what you want to accomplish.

## Learn the language in order

If Zirk is your first language, begin with [Zirk for New Programmers](../01-learning-paths/01-zirk-for-new-programmers.md). It introduces programming concepts alongside Zirk syntax and includes checkpoints before moving to the next topic.

If you already program, choose the nearest comparison path:

- [TypeScript programmers](../01-learning-paths/02-zirk-for-typescript-programmers.md)
- [Python programmers](../01-learning-paths/03-zirk-for-python-programmers.md)
- [Rust, C, and C++ programmers](../01-learning-paths/04-zirk-for-rust-c-cpp-programmers.md)
- [Java and C# programmers](../01-learning-paths/05-zirk-for-java-csharp-programmers.md)

These are not translation tables. They identify familiar-looking constructs whose semantics differ, then route you into the full handbook.

## Evaluate the development experience

Read [Tooling in Five Minutes](../01-learning-paths/06-tooling-in-five-minutes.md) if you want to see the intended edit–check–format–test–build loop before studying every language feature.

## Look up one answer

The reference section is organized for retrieval: keywords, operators, built-in types, literals, grammar, attributes, diagnostics, CLI commands, targets, permissions, the standard library, and implementation status. A reference tells you what a construct means; the handbook explains how to reason with it.

## Understand the rationale

The explanations section covers why Zirk uses static types, structured concurrency, both `Result` and exceptions, resource-aware `match with`, explicit permissions, a portable package IR, and a C-compatible interoperability boundary.

## Keep status in view

Whichever path you choose, remember that the handbook teaches the normative Zirk 1.x design. Check status notices before assuming that a chapter's examples run in the current compiler. Missing implementation is tracked separately from language semantics.

---

**Previous:** [← How Zirk Compiles](./06-how-zirk-compiles.md) · **Next:** [Learning Paths →](../01-learning-paths/README.md)
