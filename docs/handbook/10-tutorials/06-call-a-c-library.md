# Call a C Library

Select a C ABI function with explicit widths and ownership. Declare the native artifact and supported targets, then write the smallest unsafe binding for pointers, buffers, and error codes.

Wrap it in a safe Zirk function that validates inputs, returns `Result`, manages native handles as resources, and prevents borrowed memory from escaping. Add target-aware tests and confirm that incompatible architectures fail before linking.

---

**Previous:** [← Build and Publish a Library](./05-build-and-publish-a-library.md) · **Next:** [Write a Decorator →](./07-write-a-decorator.md)
