# Browser package handoff

The browser interpreter is a checked-source to verified-IR interpreter. It is
not a Zirk-to-WebAssembly backend and must never be presented as one.

Build a handoff only from a clean, committed compiler revision:

```sh
git diff --check
git diff --quiet
git rev-parse HEAD
./scripts/audit-browser-dependencies.sh
./scripts/build-browser-package.sh /absolute/empty/output/zirk-browser
./scripts/check-browser-package-size.sh /absolute/empty/output/zirk-browser
node ./scripts/smoke-browser-package.mjs /absolute/empty/output/zirk-browser
```

Record the resulting compiler revision with the package handoff. The output
directory contains `zirk_browser_bg.wasm`, ESM glue, wasm-bindgen declarations,
the strict browser contract declaration, and `LICENSE`. Do not overwrite an
existing output directory: the build command rejects it so an artifact cannot
silently lose provenance.

The site repository consumes only this verified directory. Its worker must
await WASM instantiation, create `BrowserApi`, call `initialize()`, then send
JSON requests to `check`, `run`, or `format`. The package manifest advertises
all three operations; the additive `format` response carries `formattedSource`
on success or ordered diagnostics on invalid input. Wire the playground's
Format control to that operation only after recording the new committed source
revision and verifying this package. Before initialization, the facade returns
the deterministic `ZB103` request failure.

The package `manifest.json` also carries `objectModel`, the supported and
excluded object-model construct ids from
`crates/zirk-browser/oop-catalog.json`. The verification script requires it, and
`cargo test -p zirk-browser` keeps it honest against the conformance fixtures
and the native corpus (see `docs/browser-oop-coverage.md`).
