// Rewrites ```zirk fences in the docs to canonical formatter output.
// Only fences that parse as complete programs are rewritten; fragments and
// pseudocode blocks are left untouched.
//
// Usage: node scripts/format-doc-examples.mjs [--check] [zirk-binary]
//   --check  report files whose fences would change without writing; exit 1.
import { mkdtempSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const args = process.argv.slice(2);
const check = args.includes("--check");
const binary = args.find((arg) => !arg.startsWith("--")) ?? "target/debug/zirk";
const root = resolve(new URL("..", import.meta.url).pathname);
const docsDir = join(root, "docs");

function* markdownFiles(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) yield* markdownFiles(path);
    else if (entry.endsWith(".md")) yield path;
  }
}

function* zirkFences(lines) {
  let open = -1;
  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trim();
    if (open === -1 && trimmed.startsWith("```zirk")) open = i;
    else if (open !== -1 && trimmed === "```") {
      yield { start: open + 1, end: i };
      open = -1;
    }
  }
}

const scratch = mkdtempSync(join(tmpdir(), "zirk-doc-fences-"));
const scratchFile = join(scratch, "fence.zrk");

let reformatted = 0;
let unchanged = 0;
let skipped = 0;
const drifted = [];

for (const file of markdownFiles(docsDir)) {
  const original = readFileSync(file, "utf8");
  const lines = original.split("\n");
  const edits = [];
  for (const fence of zirkFences(lines)) {
    const source = lines.slice(fence.start, fence.end).join("\n") + "\n";
    writeFileSync(scratchFile, source);
    const run = spawnSync(binary, ["format", scratchFile], { encoding: "utf8" });
    if (run.status !== 0) {
      skipped += 1; // fragment or pseudocode: never rewritten
      continue;
    }
    const formatted = readFileSync(scratchFile, "utf8");
    if (formatted === source) {
      unchanged += 1;
      continue;
    }
    reformatted += 1;
    edits.push({ start: fence.start, end: fence.end, lines: formatted.trimEnd().split("\n") });
  }
  if (edits.length === 0) continue;
  drifted.push(file);
  if (check) continue;
  for (const edit of edits.reverse()) {
    lines.splice(edit.start, edit.end - edit.start, ...edit.lines);
  }
  writeFileSync(file, lines.join("\n"));
}

console.log(
  `doc examples: ${reformatted} fences reformatted, ${unchanged} already canonical, ` +
    `${skipped} fragments skipped`,
);
if (check && drifted.length > 0) {
  for (const file of drifted) console.log(`needs formatting: ${file}`);
  process.exit(1);
}
