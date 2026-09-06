# Tasks: fase-3-verificacion-constraints

## 1. Constraint verification at the use site

- [ ] 1.1 Collect the declared `from` constraints of each `TypeParamInfo` during `enter_type_params` and resolve each constraint name to a contract.
- [ ] 1.2 Verify every concrete type argument against its parameter's constraints at instantiation sites (type annotations, constructor calls, function calls, generic defaults), emitting a diagnostic naming the type parameter and each missing contract.
- [ ] 1.3 Add valid and invalid corpus/sema fixtures covering satisfied, missing, and combined `A & B` constraints.

## 2. Body checked once against constraints

- [ ] 2.1 Resolve member access on a value of type-parameter type against the union of its declared contract constraints.
- [ ] 2.2 Reject member access not guaranteed by any constraint, with help text indicating which constraint must declare it.
- [ ] 2.3 Confirm the body is checked exactly once at the declaration (error reported once regardless of instantiation count).

## 3. Declared variance verification

- [ ] 3.1 Replace `report_declared_variance`'s `PENDING_FEATURE` with positional analysis: `out T` only in output positions, `in T` only in input positions, mutable attribute use requires invariance.
- [ ] 3.2 Emit declaration-site diagnostics for misplaced variance and for a type parameter used in a `mut` field while declared `in`/`out`.
- [ ] 3.3 Preserve `Fn` intrinsic variance behavior.
- [ ] 3.4 Add fixtures for legal and illegal variance positions.

## 4. Spec and status cleanup

- [ ] 4.1 Remove `value class` mentions from code comments/diagnostics that still reference it as a live construct.
- [ ] 4.2 Update `docs/init/ZIRK_FEATURE_STATUS.md` generics and variance rows.
- [ ] 4.3 Run `openspec validate fase-3-verificacion-constraints --strict`.

## 5. Verification

- [ ] 5.1 `cargo test --workspace` green with `LLVM_SYS_201_PREFIX` set.
- [ ] 5.2 `./scripts/sync-website-content.sh` reviewed for affected public status content.
