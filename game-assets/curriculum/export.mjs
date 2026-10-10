// Turns the curriculum document into the data the game loads.
//
//   node curriculum/export.mjs [CURRICULUM.md] [OUT.json]
//
// It also enforces the document's structure: every level needs a title, a
// concept, a story, a goal, exactly three hints and its code blocks. Reference
// solutions are NOT exported; the client must never ship them.
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '..', '..');

export function exportCurriculum(markdown, { knownAssets } = {}) {
  // Blocks are pulled out first so their content cannot be mistaken for prose.
  const blocks = [];
  const prose = markdown.replace(/```(\w+)([^\n]*)\n([\s\S]*?)```/g, (_, lang, tag, body) => {
    blocks.push({ lang, tag: tag.trim().split(/\s+/).filter(Boolean), body });
    return `\u0000BLOCK${blocks.length - 1}\u0000`;
  });

  const levels = [];
  const problems = [];
  let sector = null;
  const sections = prose.split(/\n(?=#{2,3} )/);
  for (const section of sections) {
    const sectorMatch = /^## (\d+)\. (?:Sector (\d+) — )?(.+)$/m.exec(section.split('\n')[0]);
    if (/^## /.test(section)) {
      sector = sectorMatch ? { section: Number(sectorMatch[1]), number: sectorMatch[2] ? Number(sectorMatch[2]) : null, name: sectorMatch[3].trim() } : null;
    }
    const heading = /^### Nivel (\d+\.\d+) — (.+)$/m.exec(section.split('\n')[0]);
    if (!heading) continue;
    const [, id, title] = heading;
    const field = (label) => {
      const m = new RegExp(`\\*\\*${label}:\\*\\*\\s*([\\s\\S]*?)(?=\\n\\*\\*|\\n\\u0000|\\n###|$)`).exec(section);
      return m ? m[1].replace(/\s+/g, ' ').trim() : null;
    };
    const used = [...section.matchAll(/\u0000BLOCK(\d+)\u0000/g)].map((m) => blocks[Number(m[1])]);
    const levelBlock = used.find((b) => b.lang === 'level');
    if (!levelBlock) { problems.push(`${id}: no level block`); continue; }
    const level = JSON.parse(levelBlock.body);
    if (level.id !== id) problems.push(`${id}: the level block says id "${level.id}"`);
    const code = (kind) => used.find((b) => b.lang === 'zirk' && b.tag[0] === kind && b.tag[1] === id)?.body;
    const hintsText = field('Pistas');
    // Hints are written "(1) ... (2) ... (3) ...": split on those markers in order,
    // so parentheses inside a hint (a call such as `walk(1)`) are harmless.
    const hints = [];
    if (hintsText) {
      let cursor = 0;
      const starts = [];
      for (const n of [1, 2, 3]) {
        const at = hintsText.indexOf(`(${n}) `, cursor);
        if (at < 0) break;
        starts.push(at);
        cursor = at + 4;
      }
      starts.forEach((at, i) => hints.push(hintsText.slice(at + 4, starts[i + 1] ?? hintsText.length).trim()));
    }
    const entry = {
      id,
      sector: Number(id.split('.')[0]),
      sectorName: sector?.name ?? null,
      title: title.trim(),
      type: level.type,
      concept: field('Concepto'),
      story: field('Historia'),
      goal: field('Objetivo') ?? field('Objetivo \\(predicción\\)'),
      // Only backticked ids on the single Assets line; art notes live on their own line.
      assets: [...((/\*\*Assets:\*\*([^\n]*)/.exec(section)?.[1] ?? '').matchAll(/`([a-z0-9-]+)`/g))].map((m) => m[1]),
      artNotes: field('Notas de arte'),
      hints,
      extra: field('★★★'),
      prelude: level.prelude ?? [],
    };
    if (level.type === 'predict') {
      entry.program = code('program');
      entry.options = level.options;
      entry.answer = level.answer;
      entry.explanation = field('Explicación al revelar');
    } else {
      entry.starter = code('starter');
      if (level.expect) entry.expect = level.expect;
      if (level.cases) entry.cases = level.cases;
      if (level.map) entry.map = level.map;
      if (level.facing) entry.facing = level.facing;
      if (level.par) entry.par = level.par;
      if (level.coinsRequired) entry.coinsRequired = level.coinsRequired;
    }
    // Structure checks: the game cannot present a level that lacks these.
    for (const key of ['concept', 'story', 'goal']) if (!entry[key]) problems.push(`${id}: missing ${key}`);
    // Prediction levels teach through the reveal, not through hints.
    if (level.type === 'predict' ? hints.length !== 0 && hints.length !== 3 : hints.length !== 3) problems.push(`${id}: needs exactly 3 hints, found ${hints.length}`);
    if (level.type === 'predict' ? !entry.program : !entry.starter) problems.push(`${id}: missing code block`);
    if (knownAssets) for (const asset of entry.assets) if (!knownAssets.has(asset)) problems.push(`${id}: unknown asset "${asset}" (not in game-assets/src/catalog.js)`);
    if (level.type === 'predict' && !entry.explanation) problems.push(`${id}: predict level needs an explanation`);
    levels.push(entry);
  }

  const preludes = {};
  for (const b of blocks) if (b.lang === 'zirk' && b.tag[0] === 'prelude') preludes[b.tag[1]] = b.body;

  const ids = levels.map((l) => l.id);
  if (new Set(ids).size !== ids.length) problems.push('duplicate level ids');
  return { levels, preludes, problems };
}

if (process.argv[1] && import.meta.url === new URL(`file://${resolve(process.argv[1]).split('/').map(encodeURIComponent).join('/')}`).href) {
  const docPath = resolve(process.argv[2] ?? join(repo, 'ZIRK_LEARNING_GAME_CURRICULUM.md'));
  const outPath = resolve(process.argv[3] ?? join(here, 'levels.json'));
  // The asset names in the document must be real catalog entries.
  const { CATALOG } = await import('../src/catalog.js');
  const { levels, preludes, problems } = exportCurriculum(readFileSync(docPath, 'utf8'), { knownAssets: new Set(CATALOG.map((a) => a.id)) });
  if (problems.length) {
    console.error(`${problems.length} structure problem(s):\n- ${problems.join('\n- ')}`);
    process.exit(1);
  }
  writeFileSync(outPath, `${JSON.stringify({ schemaVersion: 1, preludes, levels }, null, 2)}\n`);
  console.log(`exported ${levels.length} levels to ${outPath}`);
}
