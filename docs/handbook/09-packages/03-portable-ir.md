# Portable IR

Portable typed IR preserves generic specialization, safety checks, devirtualization, escape analysis, vectorization, and debug metadata without selecting a machine target.

The final build compiles package IR with application, stdlib, and runtime for one aligned target and ABI. IR version compatibility is checked before code generation.

Portable does not mean trustworthy. The consumer verifies package integrity,
parses the versioned format defensively, validates typed IR invariants, and
re-runs target/safety checks before optimization. Unknown features or versions
produce a package diagnostic rather than falling back to an older meaning.

Generic specialization and target layout happen in the consuming build. A
package may also declare native artifacts, but those are separate target-
specific inputs and never masquerade as portable IR.

## Union type compatibility

Portable union metadata has a versioned schema and a canonical ABI fingerprint
for each exported union layout. The fingerprint represents the normalized
logical alternatives; it excludes source order, alias spelling, compiler-local
intern IDs, target triple, and physical field offsets. A decoder rejects an
unsupported package or IR schema, and an importer rejects a missing or
mismatched union fingerprint with a rebuild instruction.

The union metadata encoder, decoder, and compatibility validator are tested in
the IR crate. The current CLI still loads dependency source into one
compilation graph and does not yet run this compiled-artifact validation at a
package-link boundary; do not treat the metadata API alone as proof that every
cached package is checked before linking.

---

**Previous:** [← Public API](02-public-api.md) · **Next:** [ Add, Remove, and Install](04-add-remove-install.md)
