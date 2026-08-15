# Why Static Types?

Zirk uses static types to make nullability, expected failure, callable relationships, permissions, resources, generic constraints, and concurrent sharing visible before native code runs. Inference keeps local code concise without erasing those facts.

Types also form the public package API and portable IR contract, enabling incremental invalidation and target specialization. The trade-off is that ambiguous dynamic behavior must become an explicit union, interface, conversion, or result.

---

**Previous:** [← Explanations](./README.md) · **Next:** [Why Structured Concurrency? →](./02-why-structured-concurrency.md)
