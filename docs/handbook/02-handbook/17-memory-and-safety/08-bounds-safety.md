# Bounds Safety

Safe indexing checks bounds unless the compiler proves the access valid. Optimization may remove a redundant check but cannot remove the guarantee.

Out-of-range access produces a controlled error according to the collection API, never undefined behavior. Use optional lookup when absence is an expected case.

---

**Previous:** [← unsafe Blocks](07-unsafe-blocks.md) · **Next:** [ Null Safety](09-null-safety.md)
