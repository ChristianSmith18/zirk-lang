// Behavioral verification of a browser package, used to prove that a build
// step such as wasm-opt changes size and nothing else.
//
// Loads the real wasm-bindgen glue and WASM in Node (no window or document),
// then runs every conformance fixture (plain and traced) and every valid
// program of the native end-to-end corpus. Each case must meet its expectation
// (fixture output, native output, or a capability exclusion). The digest covers
// output, completion, failure, diagnostic codes and trace of every case, so two
// binaries with the same digest are observably identical on this suite.
//
// usage: verify-browser-package-behavior.mjs PACKAGE_DIRECTORY [WASM_FILE]
import { readFile, readdir } from "node:fs/promises";
import { resolve, join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
const [pkg, wasmOverride] = process.argv.slice(2);
const bindings = await import(resolve(pkg, "zirk_browser.js"));
bindings.initSync({ module: await readFile(wasmOverride ?? resolve(pkg, "zirk_browser_bg.wasm")) });
const api = new bindings.BrowserApi(); api.initialize();
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const results = [];
const run = (source, name, extra = {}) => JSON.parse(api.run(JSON.stringify({ apiVersion: 1, source, virtualName: name, ...extra })));
const sig = (r) => ({ out: (r.output ?? []).map((e) => e.text).join(""), c: r.completion, f: r.failure && [r.failure.kind, r.failure.code, r.failure.message] , d: (r.diagnostics ?? []).map((x) => x.code), t: r.trace });
const fixDir = join(root, "crates/zirk-browser/fixtures");
for (const f of (await readdir(fixDir)).filter((n) => n.endsWith(".json")).sort()) {
  const fx = JSON.parse(await readFile(join(fixDir, f), "utf8"));
  const r = run(fx.source, fx.virtualName);
  results.push(["fixture:" + fx.name, sig(r), sig(r).out === fx.expectedOutput.join("")]);
  const t = run(fx.source, fx.virtualName, { trace: {} });
  results.push(["trace:" + fx.name, sig(t), t.trace?.events?.length > 0]);
}
const corpus = join(root, "crates/zirk-cli/tests/corpus/valid");
for (const f of (await readdir(corpus)).filter((n) => n.endsWith(".zrk")).sort()) {
  let exp; try { exp = await readFile(join(corpus, f.replace(".zrk", ".out")), "utf8"); } catch { continue; }
  const r = run(await readFile(join(corpus, f), "utf8"), "corpus/" + f);
  const s = sig(r);
  results.push(["corpus:" + f, s, s.f ? s.f[0] === "capability" : s.out === exp]);
}
const bad = results.filter((r) => !r[2]);
if (bad.length > 0) process.exitCode = 1;
console.log(JSON.stringify({ total: results.length, failedExpectation: bad.map((b) => b[0]), digest: createHash("sha256").update(JSON.stringify(results.map((r) => [r[0], r[1]]))).digest("hex") }));
