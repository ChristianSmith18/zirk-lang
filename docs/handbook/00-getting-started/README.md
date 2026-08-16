# Getting Started with Zirk

This section takes you from evaluating Zirk to understanding a complete minimal program. Read it in order if this is your first encounter with the language.

You will learn what Zirk is, which problems shaped it, how to distinguish the normative language from the compiler's current implementation, how the toolchain is expected to be installed, and how source becomes a native executable.

## Chapters

1. [What Is Zirk?](./01-what-is-zirk.md) introduces the language through a small program.
2. [Why Zirk Exists](./02-why-zirk-exists.md) explains the design trade-offs.
3. [Language Status](./03-language-status.md) separates specification, implementation, and historical exploration.
4. [Installation](./04-installation.md) describes supported installation contracts and verification.
5. [Your First Program](./05-first-program.md) builds a complete minimal application.
6. [How Zirk Compiles](./06-how-zirk-compiles.md) follows source through the frontend, IR, LLVM, and linker.
7. [Next Steps](./07-next-steps.md) chooses the right path through the larger handbook.

> **Implementation status:** Zirk is under active development. Commands in this section describe the normative toolchain interface; a current build may implement only a subset. Always verify with `zirk --help` and the feature-status reference for the revision you are using.

---

**Previous:** [← The Zirk Handbook](../README.md) · **Next:** [ What Is Zirk?](01-what-is-zirk.md)
