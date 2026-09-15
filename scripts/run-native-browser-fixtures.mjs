#!/usr/bin/env node
// Run browser-compatible fixture source through the native zirk run path.
//
// The runner intentionally normalizes only observable stdout and exit status:
// compiler paths, temporary names, and native executable paths never become
// fixture expectations.
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const fixtures = process.argv.slice(2);
if (fixtures.length === 0) {
  throw new Error("usage: run-native-browser-fixtures.mjs FIXTURE.json [...]");
}

for (const fixturePath of fixtures) {
  const fixture = JSON.parse(await readFile(fixturePath, "utf8"));
  const directory = await mkdtemp(join(tmpdir(), "zirk-browser-fixture-"));
  try {
    const sourcePath = join(directory, "fixture.zrk");
    await writeFile(sourcePath, fixture.source);
    let stdout = "";
    let stderr = "";
    let category = "completed";
    try {
      ({ stdout } = await run(
        "cargo",
        ["run", "--quiet", "-p", "zirk-cli", "--bin", "zirk", "--", "run", sourcePath],
        { cwd: process.cwd(), maxBuffer: 1024 * 1024 },
      ));
    } catch (error) {
      stdout = error.stdout ?? "";
      stderr = error.stderr ?? "";
      category = "runtime";
    }
    const normalized = stdout.replace(/\r\n/g, "\n");
    if (category !== fixture.completion) {
      throw new Error(
        fixture.name + ": " + category + " != " + fixture.completion + "\n" + stderr,
      );
    }
    if (normalized !== fixture.expectedOutput.join("")) {
      throw new Error(
        fixture.name
          + ": native output differs from fixture expectation\nexpected: "
          + JSON.stringify(fixture.expectedOutput.join(""))
          + "\nactual: "
          + JSON.stringify(normalized),
      );
    }
    console.log(fixture.name + ": " + category);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}
