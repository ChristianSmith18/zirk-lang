import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { exportCurriculum } from '../curriculum/export.mjs';
import { CATALOG } from '../src/catalog.js';

const here = fileURLToPath(new URL('.', import.meta.url));
const doc = fileURLToPath(new URL('../../ZIRK_LEARNING_GAME_CURRICULUM.md', import.meta.url));
const levelsFile = fileURLToPath(new URL('../curriculum/levels.json', import.meta.url));
const markdown = readFileSync(doc, 'utf8');
const knownAssets = new Set(CATALOG.map((a) => a.id));

test('the curriculum document has a valid structure and only real assets', () => {
  const { levels, problems } = exportCurriculum(markdown, { knownAssets });
  assert.deepEqual(problems, []);
  assert.equal(levels.length, 53);
  const sectors = new Map();
  for (const level of levels) sectors.set(level.sector, (sectors.get(level.sector) ?? 0) + 1);
  assert.deepEqual([...sectors.keys()], [1, 2, 3, 4, 5, 6, 7]);
  for (const level of levels) {
    assert.ok(level.assets.length > 0, `${level.id} lists no assets`);
    assert.ok(['grid', 'output', 'fix', 'predict'].includes(level.type), `${level.id} has type ${level.type}`);
  }
});

test('level ids run in order inside each sector, with no gaps', () => {
  const { levels } = exportCurriculum(markdown, { knownAssets });
  const bySector = new Map();
  for (const level of levels) {
    const minor = Number(level.id.split('.')[1]);
    const list = bySector.get(level.sector) ?? [];
    list.push(minor);
    bySector.set(level.sector, list);
  }
  for (const [sector, minors] of bySector) {
    assert.deepEqual(minors, minors.map((_, i) => i + 1), `sector ${sector} numbering`);
  }
});

test('the exported data never contains a reference solution', () => {
  const { levels } = exportCurriculum(markdown, { knownAssets });
  const exported = JSON.stringify(levels);
  for (const match of markdown.matchAll(/```zirk solution ([\d.]+)\n([\s\S]*?)```/g)) {
    const body = match[2].trim();
    assert.ok(!exported.includes(JSON.stringify(body).slice(1, -1)), `level ${match[1]} leaks its solution`);
  }
});

test('the course stays within the basics: no OOP keywords in any level code', () => {
  const OOP = /\b(class|interface|trait|extends|implements|inner|abstract)\b/;
  for (const match of markdown.matchAll(/```zirk (starter|solution|program) ([\d.]+)\n([\s\S]*?)```/g)) {
    assert.ok(!OOP.test(match[3].replace(/\/\/.*$/gm, '')), `level ${match[2]} (${match[1]}) uses an OOP keyword`);
  }
});

test('the committed levels.json is up to date with the document', () => {
  const { levels } = exportCurriculum(markdown, { knownAssets });
  const committed = JSON.parse(readFileSync(levelsFile, 'utf8'));
  assert.equal(committed.schemaVersion, 1);
  assert.deepEqual(committed.levels, levels, 'run `npm run curriculum:export` and commit levels.json');
});

test('every level is solvable by its own solution (runs the interpreter)', { skip: !existsSync(fileURLToPath(new URL('../../target/debug/examples/interp_repro', import.meta.url))) && 'build with: cargo build -p zirk-browser --example interp_repro' }, () => {
  const result = spawnSync(process.execPath, [fileURLToPath(new URL('../curriculum/verify.mjs', import.meta.url))], { encoding: 'utf8', cwd: here, timeout: 120000 });
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /all levels verified/);
});
