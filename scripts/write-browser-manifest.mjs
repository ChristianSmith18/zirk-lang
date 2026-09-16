#!/usr/bin/env node
// Generate the machine-readable provenance record for one browser package.
// This intentionally uses only Node built-ins so it is available beside the
// existing module-worker smoke runner.
import { createHash } from "node:crypto";
import { readFile, readdir, stat, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { execFileSync } from "node:child_process";

const [packageDir, sourceRevision] = process.argv.slice(2);
if (!packageDir || !sourceRevision) {
  throw new Error("usage: write-browser-manifest.mjs PACKAGE_DIRECTORY SOURCE_REVISION");
}

const files = [];
for (const entry of await readdir(packageDir)) {
  if (entry === "manifest.json") continue;
  const path = join(packageDir, entry);
  if ((await stat(path)).isFile()) {
    const content = await readFile(path);
    files.push({
      path: entry,
      bytes: content.byteLength,
      sha256: createHash("sha256").update(content).digest("hex"),
    });
  }
}
files.sort((left, right) => left.path.localeCompare(right.path));
const commandVersion = (command, args) => {
  try {
    return execFileSync(command, args, { encoding: "utf8" }).trim();
  } catch {
    return "unavailable";
  }
};
const wasm = files.find((file) => file.path === "zirk_browser_bg.wasm");
if (!wasm) throw new Error("package is missing zirk_browser_bg.wasm");

const manifest = {
  schemaVersion: 1,
  apiVersion: 1,
  compilerVersion: process.env.npm_package_version ?? "0.0.0",
  sourceRevision,
  operations: ["check", "run", "format"],
  limits: {
    sourceBytes: 262144,
    diagnostics: 128,
    instructions: 2000000,
    callDepth: 128,
    heapBytes: 16777216,
    outputBytes: 262144,
    outputEvents: 2048,
    stackFrames: 64,
  },
  exclusions: ["extern", "pointer", "native-slice", "process", "shell", "filesystem", "environment", "secrets", "network", "clock", "random", "DOM"],
  tools: {
    node: process.version,
    rustc: commandVersion("rustc", ["--version"]),
    wasmBindgen: commandVersion("wasm-bindgen", ["--version"]),
  },
  files,
};
await writeFile(join(packageDir, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`browser manifest written for ${basename(packageDir)} at ${sourceRevision}`);
