// Verifies the learning-game curriculum document by running it.
//
//   node curriculum/verify.mjs [CURRICULUM.md]
//
// Every level in the document carries machine-readable blocks:
//
//   ```level            JSON: id, type, prelude, expectations
//   ```zirk starter ID  the code the learner starts from
//   ```zirk solution ID the reference solution
//   ```zirk program ID  (predict levels) the program whose output is the answer
//   ```zirk prelude N   hidden helper functions a level may use
//   ```zirk finding ID  a language limit the document relies on, with a
//   ```expect ID        JSON stating what the compiler/interpreter does with it
//   ```diagnostics      JSON glossary: every code a level exercises, and only those
//
// The verifier assembles each program exactly as the game would (hidden import
// first, learner code, hidden helpers last), runs it through the browser
// interpreter, and checks the level's promises. A level that cannot be solved
// by its own solution, or whose starter already passes, fails verification.
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '..', '..');
const docPath = resolve(process.argv[2] ?? join(repo, 'ZIRK_LEARNING_GAME_CURRICULUM.md'));
const runner = process.env.ZIRK_REPRO ?? join(repo, 'target/debug/examples/interp_repro');
if (!existsSync(runner)) {
  console.error(`missing ${runner}; build it with: cargo build -p zirk-browser --example interp_repro`);
  process.exit(2);
}

// ---------------------------------------------------------------- parsing
const text = readFileSync(docPath, 'utf8');
const blocks = [];
for (const match of text.matchAll(/```(\w+)([^\n]*)\n([\s\S]*?)```/g)) {
  blocks.push({ lang: match[1], tag: match[2].trim().split(/\s+/).filter(Boolean), body: match[3] });
}
const levels = new Map();
const preludes = new Map();
const findings = new Map();
const findingExpect = new Map();
let glossary = null;
for (const b of blocks) {
  if (b.lang === 'level') {
    const level = JSON.parse(b.body);
    levels.set(level.id, { ...level, starter: undefined, solution: undefined, program: undefined });
  }
}
for (const b of blocks) {
  if (b.lang === 'expect') findingExpect.set(b.tag[0], JSON.parse(b.body));
  if (b.lang === 'diagnostics') glossary = JSON.parse(b.body);
}
for (const b of blocks) {
  if (b.lang !== 'zirk') continue;
  const [kind, id] = b.tag;
  if (kind === 'prelude') preludes.set(id, b.body);
  else if (kind === 'finding') findings.set(id, b.body);
  else if (['starter', 'solution', 'program'].includes(kind)) {
    const level = levels.get(id);
    if (!level) throw new Error(`block "${kind} ${id}" has no matching level block`);
    level[kind] = b.body;
  }
}

// ---------------------------------------------------------------- running
const IMPORT = 'import { stdout, Stringifiable } from std.io; ';
const scratch = mkdtempSync(join(tmpdir(), 'zirk-curriculum-'));
let counter = 0;

/** Assemble and run a learner program the way the game does. */
function run(level, code, extra = '') {
  const helpers = (level.prelude ?? []).map((name) => {
    if (!preludes.has(name)) throw new Error(`level ${level.id}: unknown prelude "${name}"`);
    return preludes.get(name);
  });
  // The hidden import goes on the learner's first line so line numbers survive.
  const source = `${IMPORT}${code}\n${helpers.join('\n')}\n${extra}\n`;
  const file = join(scratch, `l${counter++}.zrk`);
  writeFileSync(file, source);
  const result = spawnSync(runner, [file], { encoding: 'utf8', timeout: 20000 });
  const lines = result.stdout.split('\n').slice(1); // first line is the file banner
  const diagnostics = [];
  const output = [];
  let completion = null;
  let failure = null;
  for (const line of lines) {
    let m;
    if ((m = /^\[diag (\w+) (\w+)\] (.*)$/.exec(line))) diagnostics.push({ severity: m[1], code: m[2], message: m[3] });
    else if ((m = /^\[failure: (\w+) (\w+) (.*)\]$/.exec(line))) failure = { kind: m[1], code: m[2], message: m[3] };
    else if (/^\[completion: /.test(line)) completion = 'completed';
    else if (/^\[no completion\/failure\]/.test(line)) completion = null;
    else output.push(line);
  }
  while (output.length && output.at(-1) === '') output.pop();
  const errors = diagnostics.filter((d) => d.severity === 'error');
  return { errors, diagnostics, output, completion, failure, source, stderr: result.stderr ?? '' };
}

// ---------------------------------------------------------------- grid simulator
const DIRS = { east: [1, 0], south: [0, 1], west: [-1, 0], north: [0, -1] };
const ORDER = ['north', 'east', 'south', 'west'];

function simulate(level, commands) {
  const map = level.map.map((row) => row.split(''));
  let x = 0;
  let y = 0;
  let coins = 0;
  let total = 0;
  map.forEach((row, ry) => row.forEach((cell, rx) => {
    if (cell === 'S') [x, y] = [rx, ry];
    if (cell === 'c') total++;
  }));
  let facing = level.facing ?? 'east';
  const cellAt = (cx, cy) => map[cy]?.[cx];
  const fail = (why) => ({ ok: false, why, steps: commands.length });
  const move = (dx, dy) => {
    x += dx;
    y += dy;
  };
  for (const command of commands) {
    const [name, arg] = command.slice(1).split(' ');
    if (name === 'walk') {
      const [dx, dy] = DIRS[facing];
      for (let i = 0; i < Number(arg); i++) {
        const next = cellAt(x + dx, y + dy);
        if (next === undefined || next === '#') return fail(`blocked at (${x},${y}) facing ${facing}`);
        if (next === '~') return fail(`fell into the water at (${x + dx},${y + dy})`);
        move(dx, dy);
      }
    } else if (name === 'hop') {
      const [dx, dy] = DIRS[facing];
      const landing = cellAt(x + 2 * dx, y + 2 * dy);
      if (landing === undefined || landing === '#' || landing === '~') return fail(`bad landing at (${x + 2 * dx},${y + 2 * dy})`);
      move(2 * dx, 2 * dy);
    } else if (name === 'turn') {
      const i = ORDER.indexOf(facing);
      facing = ORDER[(i + (arg === 'left' ? 3 : 1)) % 4];
    } else if (name === 'collect') {
      if (cellAt(x, y) !== 'c') return fail(`nothing to collect at (${x},${y})`);
      map[y][x] = '.';
      coins++;
    } else {
      return fail(`unknown command ${command}`);
    }
  }
  const need = level.coinsRequired ?? total;
  if (cellAt(x, y) !== 'G') return fail(`ended at (${x},${y}), not on the goal`);
  if (coins < need) return fail(`collected ${coins} of ${need} coins`);
  return { ok: true, steps: commands.length, coins };
}

// ---------------------------------------------------------------- level checks
const problems = [];
const report = [];
const exercised = new Set();
const bad = (level, message) => problems.push(`L${level.id}: ${message}`);
const OOP = /\b(class|interface|trait|extends|implements|inner|abstract)\b/;

const commandsOf = (r) => r.output.filter((l) => l.startsWith('@'));
const scenarios = (level) => level.cases ?? [{ name: 'default' }];

function passes(level, r, scenario) {
  if (r.errors.length || r.failure) return false;
  if (level.type === 'grid') return simulate({ ...level, map: scenario.map ?? level.map }, commandsOf(r)).ok;
  const expected = scenario.expect ?? level.expect;
  return JSON.stringify(r.output) === JSON.stringify(expected);
}

// ---------------------------------------------------------------- findings
for (const [id, code] of findings) {
  const expectation = findingExpect.get(id);
  if (!expectation) { problems.push(`finding ${id}: missing expect block`); continue; }
  const r = run({ id: `finding ${id}`, prelude: [] }, code);
  if (expectation.diagnostic) {
    exercised.add(expectation.diagnostic);
    if (r.errors[0]?.code !== expectation.diagnostic) problems.push(`finding ${id}: expected ${expectation.diagnostic}, got ${r.errors[0]?.code ?? r.failure?.code ?? 'a passing program'}`);
  } else if (expectation.runtime) {
    exercised.add(expectation.runtime);
    if (r.failure?.code !== expectation.runtime) problems.push(`finding ${id}: expected runtime ${expectation.runtime}, got ${r.failure?.code ?? r.errors[0]?.code ?? 'a passing program'}`);
  } else if (expectation.output) {
    if (r.errors.length || r.failure || JSON.stringify(r.output) !== JSON.stringify(expectation.output)) problems.push(`finding ${id}: expected output ${JSON.stringify(expectation.output)}, got ${JSON.stringify(r.output)} ${r.errors[0]?.code ?? ''}`);
  } else if (expectation.compilerPanic) {
    if (!/panicked/.test(r.stderr)) problems.push(`finding ${id}: expected a compiler panic, but none happened`);
  } else {
    problems.push(`finding ${id}: unknown expectation`);
  }
}

for (const level of levels.values()) {
  const tag = `${level.id} ${level.type}`;
  if (level.starterDiagnostic) exercised.add(level.starterDiagnostic);
  if (level.starterRuntime) exercised.add(level.starterRuntime);
  for (const m of level.mistakes ?? []) {
    if (m.diagnostic) exercised.add(m.diagnostic);
    if (m.runtime) exercised.add(m.runtime);
    if (m.limit) exercised.add('ZB102');
  }
  for (const [field, code] of [['starter', level.starter], ['solution', level.solution], ['program', level.program]]) {
    if (code && OOP.test(code.replace(/\/\/.*$/gm, ''))) bad(level, `${field} uses an OOP keyword; the basics curriculum must not`);
  }
  if (level.type === 'predict') {
    if (!level.program) { bad(level, 'predict level needs a program block'); continue; }
    const r = run(level, level.program);
    if (r.errors.length) { bad(level, `program does not compile: ${r.errors[0].code} ${r.errors[0].message}`); continue; }
    if (r.failure) exercised.add(r.failure.code);
    const actual = r.failure ? `${r.failure.code}` : r.output.join('\n');
    if (level.options[level.answer] !== actual) bad(level, `answer "${level.options[level.answer]}" but the program prints "${actual}"`);
    if (new Set(level.options).size !== level.options.length) bad(level, 'predict options must be distinct');
    report.push(`${tag}: ok`);
    continue;
  }
  if (!level.solution) { bad(level, 'missing solution block'); continue; }
  if (!level.starter) { bad(level, 'missing starter block'); continue; }

  for (const scenario of scenarios(level)) {
    const extra = scenario.helpers ?? '';
    const solved = run(level, level.solution, extra);
    if (!passes(level, solved, scenario)) {
      const why = solved.errors[0] ? `${solved.errors[0].code} ${solved.errors[0].message}`
        : solved.failure ? `${solved.failure.code} ${solved.failure.message}`
        : level.type === 'grid' ? simulate({ ...level, map: scenario.map ?? level.map }, commandsOf(solved)).why
        : `output ${JSON.stringify(solved.output)} != ${JSON.stringify(scenario.expect ?? level.expect)}`;
      bad(level, `solution fails case "${scenario.name}": ${why}`);
    } else if (level.type === 'grid') {
      const result = simulate({ ...level, map: scenario.map ?? level.map }, commandsOf(solved));
      if (level.par && result.steps > level.par) bad(level, `solution uses ${result.steps} commands, par is ${level.par}`);
    }
  }
  // The starter must be a real task: it may not already solve every case.
  const starterSolvesAll = scenarios(level).every((s) => passes(level, run(level, level.starter, s.helpers ?? ''), s));
  if (starterSolvesAll) bad(level, 'the starter already passes; the level would be a no-op');
  if (level.starterDiagnostic) {
    const r = run(level, level.starter, scenarios(level)[0].helpers ?? '');
    const code = r.errors[0]?.code;
    if (code !== level.starterDiagnostic) bad(level, `starter should fail with ${level.starterDiagnostic}, got ${code ?? 'no compile error'}`);
  }
  if (level.starterRuntime) {
    const r = run(level, level.starter, scenarios(level)[0].helpers ?? '');
    if (r.failure?.code !== level.starterRuntime) bad(level, `starter should fail at runtime with ${level.starterRuntime}, got ${r.failure?.code ?? 'no runtime failure'}`);
  }
  // Documented common mistakes must really behave as the document says: apply
  // the textual change to the solution and check the promised diagnostic or
  // runtime failure.
  for (const mistake of level.mistakes ?? []) {
    const [from, to] = mistake.replace;
    if (!level.solution.includes(from)) { bad(level, `mistake text "${from}" is not in the solution`); continue; }
    const broken = level.solution.replaceAll(from, to);
    const r = run(level, broken, scenarios(level)[0].helpers ?? '');
    if (mistake.diagnostic) {
      if (r.errors[0]?.code !== mistake.diagnostic) bad(level, `mistake "${from}" -> "${to}" should give ${mistake.diagnostic}, got ${r.errors[0]?.code ?? (r.failure?.code ?? 'a passing program')}`);
    } else if (mistake.limit) {
      if (r.failure?.kind !== 'Limit') bad(level, `mistake "${from}" -> "${to}" should hit an execution limit, got ${r.failure?.kind ?? r.errors[0]?.code ?? 'a passing program'}`);
    } else if (mistake.runtime) {
      if (r.failure?.code !== mistake.runtime) bad(level, `mistake "${from}" -> "${to}" should fail with ${mistake.runtime}, got ${r.failure?.code ?? r.errors[0]?.code ?? 'a passing program'}`);
    } else if (mistake.wrongResult) {
      // A real mistake must break at least one case; passing every case means
      // the documented "error" is harmless.
      const everyCasePasses = scenarios(level).every((scenario) => passes(level, run(level, broken, scenario.helpers ?? ''), scenario));
      if (everyCasePasses) bad(level, `mistake "${from}" -> "${to}" still passes every case; it is not a mistake`);
    }
  }
  // Warnings in a solution would teach bad habits.
  const warn = run(level, level.solution, scenarios(level)[0].helpers ?? '').diagnostics.filter((d) => d.severity === 'warning');
  if (warn.length) bad(level, `solution compiles with a warning: ${warn[0].code} ${warn[0].message}`);
  report.push(`${tag}: ok`);
}

// The glossary must describe exactly the diagnostics the course relies on.
if (!glossary) problems.push('missing ```diagnostics glossary block');
else {
  for (const code of exercised) if (!(code in glossary)) problems.push(`glossary: ${code} is exercised by a level but not explained`);
  for (const code of Object.keys(glossary)) if (!exercised.has(code)) problems.push(`glossary: ${code} is explained but no level or finding exercises it`);
}

const total = levels.size;
console.log(`${total} levels, ${findings.size} findings and ${Object.keys(glossary ?? {}).length} diagnostics checked in ${docPath}`);
if (problems.length) {
  console.error(`\n${problems.length} problem(s):\n- ${problems.join('\n- ')}`);
  process.exit(1);
}
console.log('all levels verified');
