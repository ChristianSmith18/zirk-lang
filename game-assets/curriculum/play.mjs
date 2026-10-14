// Play a level from the terminal.
//
//   node curriculum/play.mjs 3.1              write the starter (once), run it, give a verdict
//   node curriculum/play.mjs 3.1 --watch      re-run every time you save the file
//   node curriculum/play.mjs 3.1 --hint 2     show hint 2 (hints are 1..3)
//   node curriculum/play.mjs 2.8 --answer 1   prediction levels: bet on an option
//   node curriculum/play.mjs --list           list every level
//
// Your file lives in ~/.zirk-play/<id>.zrk (set ZIRK_PLAY_DIR to move it). The
// game's hidden helpers (walk, turn_right, inspect, …) are added around it
// exactly as the real game will, so your file only holds your own code.
import { readFileSync, writeFileSync, existsSync, mkdirSync, watch, mkdtempSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { homedir, tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { simulate } from './lib.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '..', '..');
const runner = process.env.ZIRK_REPRO ?? join(repo, 'target/debug/examples/interp_repro');
const data = JSON.parse(readFileSync(join(here, 'levels.json'), 'utf8'));
const args = process.argv.slice(2);
const flag = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
const id = args.find((a) => /^\d+\.\d+$/.test(a));

const color = (code, text) => (process.stdout.isTTY ? `\x1b[${code}m${text}\x1b[0m` : text);
const green = (t) => color('32', t);
const red = (t) => color('31', t);
const dim = (t) => color('2', t);
const bold = (t) => color('1', t);

if (args.includes('--list') || !id) {
  for (const l of data.levels) console.log(`${l.id.padEnd(5)} ${l.type.padEnd(8)} ${l.title}`);
  if (!id && !args.includes('--list')) console.log('\nusage: node curriculum/play.mjs <level> [--watch | --hint N | --answer N]');
  process.exit(id || args.includes('--list') ? 0 : 1);
}

const level = data.levels.find((l) => l.id === id);
if (!level) {
  console.error(`no level ${id}; try --list`);
  process.exit(1);
}
if (!existsSync(runner)) {
  console.error(`missing ${runner}\nbuild it with: cargo build -p zirk-browser --example interp_repro`);
  process.exit(2);
}

const IMPORT = 'import { stdout, Stringifiable } from std.io; ';
const scratch = mkdtempSync(join(tmpdir(), 'zirk-play-'));
let counter = 0;

function run(code, extra = '') {
  const helpers = level.prelude.map((name) => data.preludes[name]).join('\n');
  const file = join(scratch, `p${counter++}.zrk`);
  writeFileSync(file, `${IMPORT}${code}\n${helpers}\n${extra}\n`);
  const result = spawnSync(runner, [file], { encoding: 'utf8', timeout: 20000 });
  const diagnostics = [];
  const output = [];
  let failure = null;
  let completed = false;
  for (const line of result.stdout.split('\n').slice(1)) {
    let m;
    if ((m = /^\[diag (\w+) (\w+)\] (.*)$/.exec(line))) diagnostics.push({ severity: m[1], code: m[2], message: m[3] });
    else if ((m = /^\[failure: (\w+) (\w+) (.*)\]$/.exec(line))) failure = { kind: m[1], code: m[2], message: m[3] };
    else if (/^\[completion: /.test(line)) completed = true;
    else if (!/^\[no completion/.test(line)) output.push(line);
  }
  while (output.length && output.at(-1) === '') output.pop();
  const panicked = /panicked/.test(result.stderr ?? '');
  return { diagnostics, output, failure, completed, panicked };
}

function header() {
  console.log(`${bold(`Nivel ${level.id} — ${level.title}`)}  ${dim(`[${level.type}]`)}`);
  console.log(`${bold('Concepto:')} ${level.concept}`);
  console.log(`${bold('Historia:')} ${level.story}`);
  console.log(`${bold('Objetivo:')} ${level.goal}`);
  if (level.map) {
    console.log(`${bold('Mapa')} (S inicio, G meta, c moneda, # muro, ~ agua; empiezas mirando al ${level.facing ?? 'east'}):`);
    const withMaps = (level.cases ?? []).filter((c) => c.map);
    if (withMaps.length) for (const c of withMaps) console.log(`  ${dim(c.name)}\n${c.map.map((r) => `   ${r}`).join('\n')}`);
    else console.log(level.map.map((r) => `   ${r}`).join('\n'));
  }
  if (level.cases) console.log(dim(`Tu programa debe servir en ${level.cases.length} casos de prueba: ${level.cases.map((c) => c.name).join(', ')}.`));
  console.log('');
}

function diagnosticsBlock(r) {
  for (const d of r.diagnostics.filter((x) => x.severity === 'error')) console.log(red(`  error[${d.code}]: ${d.message}`));
  if (r.panicked) console.log(red('  el compilador se detuvo con un fallo interno (no es culpa tuya)'));
  if (r.failure) console.log(red(`  el programa se detuvo: ${r.failure.code} — ${r.failure.message}`));
}

function judge(code) {
  const scenarios = level.cases ?? [{ name: 'único caso' }];
  let allOk = true;
  for (const scenario of scenarios) {
    const r = run(code, scenario.helpers ?? '');
    const label = scenarios.length > 1 ? `caso «${scenario.name}»: ` : '';
    const broken = r.diagnostics.some((d) => d.severity === 'error') || r.failure || r.panicked || !r.completed;
    if (broken) {
      console.log(`${label}${red('✘')}`);
      diagnosticsBlock(r);
      allOk = false;
      continue;
    }
    if (level.type === 'grid') {
      const commands = r.output.filter((l) => l.startsWith('@'));
      const result = simulate({ ...level, map: scenario.map ?? level.map }, commands);
      console.log(`${label}${result.ok ? green('✔') : red('✘')} ${result.ok ? `${result.steps} órdenes${level.par ? ` (par: ${level.par})` : ''}` : result.why}`);
      allOk &&= result.ok;
    } else {
      const expected = scenario.expect ?? level.expect;
      const ok = JSON.stringify(r.output) === JSON.stringify(expected);
      console.log(`${label}${ok ? green('✔') : red('✘')}`);
      if (!ok) {
        console.log(`  esperado: ${JSON.stringify(expected)}\n  obtenido: ${JSON.stringify(r.output)}`);
        allOk = false;
      }
    }
  }
  console.log(allOk ? green(bold('\n¡Nivel superado!')) : red('\nTodavía no.'));
  return allOk;
}

header();

if (level.type === 'predict') {
  if (flag('--hint')) console.log('Los niveles de predicción no tienen pistas: la explicación llega al revelar.');
  console.log(`${bold('Programa:')}\n${level.program.split('\n').map((l) => `   ${l}`).join('\n')}`);
  level.options.forEach((o, i) => console.log(`  ${i}) ${JSON.stringify(o)}`));
  const answer = flag('--answer');
  if (answer === undefined) {
    console.log(`\nElige con: node curriculum/play.mjs ${level.id} --answer <número>`);
    process.exit(0);
  }
  const real = run(level.program);
  const actual = real.failure ? real.failure.code : real.output.join('\n');
  const ok = level.options[Number(answer)] === actual;
  console.log(`\nResultado real: ${JSON.stringify(actual)}\n${ok ? green(bold('¡Acertaste!')) : red('No era esa.')}\n${level.explanation ?? ''}`);
  process.exit(ok ? 0 : 1);
}

const hint = Number(flag('--hint'));
if (hint) {
  console.log(level.hints[hint - 1] ? `${bold(`Pista ${hint}:`)} ${level.hints[hint - 1]}` : 'Las pistas son 1, 2 y 3.');
  process.exit(0);
}

const dir = process.env.ZIRK_PLAY_DIR ?? join(homedir(), '.zirk-play');
mkdirSync(dir, { recursive: true });
const file = join(dir, `${level.id}.zrk`);
if (!existsSync(file)) {
  writeFileSync(file, level.starter);
  console.log(`${dim('Escribí el código inicial en')} ${file}\n`);
} else {
  console.log(`${dim('Usando tu archivo')} ${file}\n`);
}
console.log(dim(`Pistas: node curriculum/play.mjs ${level.id} --hint 1 (también 2 y 3)\n`));

const ok = judge(readFileSync(file, 'utf8'));
if (args.includes('--watch')) {
  console.log(dim('\nEsperando cambios en el archivo… (Ctrl+C para salir)'));
  let timer;
  watch(file, () => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      console.log(`\n${'─'.repeat(50)}`);
      judge(readFileSync(file, 'utf8'));
    }, 150);
  });
} else {
  process.exit(ok ? 0 : 1);
}
