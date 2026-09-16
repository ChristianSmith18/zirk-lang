// Module-worker compatible smoke test: no window or document is used.
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const packageDir = process.argv[2];
if (!packageDir) throw new Error("usage: node smoke-browser-package.mjs PACKAGE_DIRECTORY");

const modulePath = resolve(packageDir, "zirk_browser.js");
const wasmPath = resolve(packageDir, "zirk_browser_bg.wasm");
const bindings = await import(modulePath);
bindings.initSync({ module: await readFile(wasmPath) });
const api = new bindings.BrowserApi();
api.initialize();
const formatted = JSON.parse(api.format(JSON.stringify({
  apiVersion: 1,
  source: 'fn main(): Void {  stdout.println(  1 ); }',
  virtualName: 'format.zrk',
})));
if (formatted.failure || formatted.formattedSource !== 'fn main(): Void { stdout.println(1); }') {
  throw new Error(`unexpected format response: ${JSON.stringify(formatted)}`);
}
const fixtures = [
  ['hello', 'fn main(): Void { stdout.println("Hello World"); }', 'completed', 'Hello World\n'],
  ['call', 'fn add(left: Int32, right: Int32): Int32 { return left + right; } fn main(): Void { stdout.println(add(40, 2)); }', 'completed', '42\n'],
  ['fatal', 'fn main(): Void { fatalError("fixture failure"); }', 'runtime', ''],
];
for (const [name, source, expected, output] of fixtures) {
  const response = JSON.parse(api.run(JSON.stringify({
    apiVersion: 1, source, virtualName: `${name}.zrk`,
  })));
  const actual = response.output?.map((event) => event.text).join("") ?? "";
  const category = response.failure?.kind ?? response.completion;
  if (category !== expected || actual !== output) {
    throw new Error(`unexpected ${name} response: ${JSON.stringify(response)}`);
  }
}
console.log("browser module-worker smoke test passed");
