## 1. Hub and narrative

- [ ] 1.1 Add `docs/handbook/02-handbook/03-everyday-types/00-how-values-live-and-share.md` with the memory/ownership spectrum and links to per-type chapters.
- [ ] 1.2 Rewrite `docs/handbook/02-handbook/03-everyday-types/01a-type-categories.md` with runnable examples for every category and explicit sharing/mutation rules.
- [ ] 1.3 Add `docs/handbook/02-handbook/03-everyday-types/02-choosing-a-type.md` that maps common problems to the right type category.

## 2. Per-type chapters

- [ ] 2.1 Expand `docs/handbook/02-handbook/12-collections/01-arrays.md` with construction, fixed/variable length, sharing, and invalid examples.
- [ ] 2.2 Expand `docs/handbook/02-handbook/12-collections/03-lists.md` with growth, insertion, removal, sharing, and invalid alias examples.
- [ ] 2.3 Add `docs/handbook/02-handbook/03-everyday-types/12-regex.md` (or place under `std.text`) with literal, match, find, replace, and implementation status.
- [ ] 2.4 Expand `docs/handbook/02-handbook/17-memory-and-safety/05-pointers.md` and add `docs/handbook/02-handbook/17-memory-and-safety/06-native-slice.md` with unsafe examples and status callouts.
- [ ] 2.5 Expand `docs/handbook/02-handbook/17-memory-and-safety/04-safe-references.md` into a `Weak`, `Dependent`, and pinning section with examples.
- [ ] 2.6 Add `docs/handbook/02-handbook/03-everyday-types/13-callable-types.md` for `Fn` and `Function` syntax, closures, and implementation status.

## 3. Reference and catalog

- [ ] 3.1 Expand `docs/handbook/11-reference/03-built-in-types.md` to include one example per major spectrum position and links to detailed chapters.
- [ ] 3.2 Update `docs/handbook/SUMMARY.md` with the new hub, choosing-a-type page, and per-type chapters in canonical order.
- [ ] 3.3 Add/verify previous and next links in every new and updated chapter.

## 4. Status and website sync

- [ ] 4.1 Review `docs/init/ZIRK_FEATURE_STATUS.md` and `docs/handbook/11-reference/12-feature-status.md` for any claim affected by the new examples.
- [ ] 4.2 Commit the handbook changes in `zirk-lang` and note the clean commit revision.
- [ ] 4.3 Run `./scripts/sync-website-content.sh` in `../zirk-lang-site` with the new commit and an explicit `--audit-date`.
- [ ] 4.4 Verify the website build (`ng build`) and tests (`yarn content:test`) pass.
