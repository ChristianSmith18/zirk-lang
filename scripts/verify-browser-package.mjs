#!/usr/bin/env node
// Verify a package's immutable provenance fields and every emitted artifact.
import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import { join } from "node:path";

const [packageDir, expectedRevision] = process.argv.slice(2);
if (!packageDir || !expectedRevision) {
  throw new Error("usage: verify-browser-package.mjs PACKAGE_DIRECTORY EXPECTED_REVISION");
}
const manifest = JSON.parse(await readFile(join(packageDir, "manifest.json"), "utf8"));
if (manifest.schemaVersion !== 1 || manifest.apiVersion !== 1) {
  throw new Error("unsupported browser package manifest schema or API version");
}
if (manifest.sourceRevision !== expectedRevision) {
  throw new Error(`stale package revision: ${manifest.sourceRevision} != ${expectedRevision}`);
}
if (!Array.isArray(manifest.operations)
    || manifest.operations.length !== 3
    || !["check", "run", "format"].every((operation) => manifest.operations.includes(operation))) {
  throw new Error("manifest must advertise the check, run, and format operations");
}
if (!manifest.objectModel
    || !Array.isArray(manifest.objectModel.supported)
    || manifest.objectModel.supported.length === 0
    || !Array.isArray(manifest.objectModel.excluded)) {
  throw new Error("manifest must publish the object-model coverage verdicts");
}
if (!manifest.trace || manifest.trace.effect !== "observation" || !Array.isArray(manifest.trace.kinds)) {
  throw new Error("manifest must describe the execution trace as an observation feature");
}
if (!Array.isArray(manifest.files) || manifest.files.length === 0) {
  throw new Error("manifest has no artifact records");
}
for (const file of manifest.files) {
  if (!file.path || file.path.includes("/") || !/^[a-f0-9]{64}$/.test(file.sha256)) {
    throw new Error("manifest contains an invalid artifact record");
  }
  const path = join(packageDir, file.path);
  const content = await readFile(path);
  if ((await stat(path)).size !== file.bytes) {
    throw new Error(`size mismatch for ${file.path}`);
  }
  const actual = createHash("sha256").update(content).digest("hex");
  if (actual !== file.sha256) throw new Error(`checksum mismatch for ${file.path}`);
}
console.log(`browser package integrity verified at ${expectedRevision}`);
