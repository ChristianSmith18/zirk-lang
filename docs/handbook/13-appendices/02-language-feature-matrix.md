# Language Feature Matrix

| Area | Zirk 1.x contract |
| --- | --- |
| Types | Static with inference, unions, nullable, generics |
| Objects | Classes, records, enums, interfaces, traits |
| Data | Tuples, unions, `type` aliases, derived equality/`Clone` |
| Collections | `Array<T>` fixed-capacity, `List<T>` resizable, ranges, iteration contracts |
| Text | Grapheme `Char`, `String`, interpolation, `re'...'` `Regex` |
| Time | `Duration` nanosecond primitive; civil/zone temporal family |
| Failure | `Result`, exceptions, `fatalError` |
| Memory | Automatic; no public ownership/RC semantics |
| Concurrency | `concurrent`, `spawn`/`Job<T>`, `Timer`, `parallel`, typed channels; managed OS threads and synchronization APIs are not currently exposed |
| Resources | `Resource<E>` and `match with` |
| Native | C ABI, `unsafe`, pointers, intrinsics, SIMD |
| Build | LLVM, portable IR, native targets, locked packages |

---

**Previous:** [← Glossary](01-glossary.md) · **Next:** [ Valid and Invalid Examples](03-valid-and-invalid-examples.md)
