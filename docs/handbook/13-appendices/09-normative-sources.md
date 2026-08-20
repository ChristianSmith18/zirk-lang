# Normative Sources

Authority order:

1. [`ZIRK_SPEC_FINAL.md`](../../ZIRK_SPEC_FINAL.md) for identity, scope, exclusions, and conflict resolution.
2. [`CORE_LANGUAGE_SEMANTICS.md`](../../CORE_LANGUAGE_SEMANTICS.md),
   [`ERROR_RESOURCE_PERMISSION_SEMANTICS.md`](../../ERROR_RESOURCE_PERMISSION_SEMANTICS.md),
   [`MEMORY_AND_UNSAFE_SEMANTICS.md`](../../MEMORY_AND_UNSAFE_SEMANTICS.md),
   [`STRUCTURED_CONCURRENCY_SEMANTICS.md`](../../STRUCTURED_CONCURRENCY_SEMANTICS.md),
   and [`DECORATOR_SEMANTICS.md`](../../DECORATOR_SEMANTICS.md) for their
   accepted cross-feature domains.
3. [`ZIRK_LANGUAGE_SPEC.md`](../../ZIRK_LANGUAGE_SPEC.md),
   [`ZIRK_RUNTIME_SPEC.md`](../../ZIRK_RUNTIME_SPEC.md),
   [`ZIRK_STDLIB_SPEC.md`](../../ZIRK_STDLIB_SPEC.md), and
   [`ZIRK_COMPILER_SPEC.md`](../../ZIRK_COMPILER_SPEC.md) for specialized detail
   not superseded by a newer checkpoint.
4. Main OpenSpec capability specs as conformance requirements and delivery
   structure, without allowing phase-local narrowing to override final language
   semantics.
5. [`01_plantilla_zirk.md`](../../01_plantilla_zirk.md), ADRs, phase designs,
   tasks, and archived changes as topic inventory, rationale, or implementation
   evidence—never as authority over later final decisions.

Unresolved ambiguity is recorded rather than silently resolved.

For implementation, read the final semantic owner first and then Feature Status
to determine current pipeline coverage. For documentation, edit the canonical
owner and every derivative index/example in the same change.

---

**Previous:** [← Roadmap](08-roadmap.md) · **Next:** End
