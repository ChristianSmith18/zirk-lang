# Automatic Vectorization

LLVM may vectorize loops when dependency and alias analysis prove it preserves semantics. Release optimization cannot weaken bounds, overflow, equality, ordering, or error guarantees.

Measure before rewriting code around target details. Portable source plus representative benchmarks is the default.

Vectorization requires proof that iterations are independent or that a
reduction is associative under the operation's actual Zirk semantics. Potential
aliasing, checked failure order, observable reference identity, volatile/native
effects, cancellation points, and unsafe transaction journals can prevent the
transformation.

Release diagnostics may explain why a hot loop did or did not vectorize, but
vectorization is never a correctness requirement. Floating reductions are not
reassociated unless the language/API explicitly permits that numerical model.

Use representative inputs and targets when measuring. A speedup on one
architecture does not justify weakening a portable contract or forcing a
target-specific code path on every build.

---

**Previous:** [← Portable SIMD](08-portable-simd.md) · **Next:** [ Why No Inline Assembly?](10-why-no-inline-assembly.md)
