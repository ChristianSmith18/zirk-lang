# Why the C ABI?

The C ABI is the most portable stable boundary shared by operating systems and native libraries. It avoids depending on unstable C++ or Rust compiler ABIs while allowing wrappers around those ecosystems.

Its simplicity shifts ownership, errors, layout, and safety into explicit binding contracts—which is why safe wrappers remain essential.

---

**Previous:** [← Why No Inline Assembly?](./08-why-no-inline-assembly.md) · **Next:** [Why Permissions? →](./10-why-permissions.md)
