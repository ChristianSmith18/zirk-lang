// Verifies GLB export in a real browser for every category: node preview/export-check.mjs
import { createRequire } from 'node:module';
import { serve } from './serve.mjs';

const require = createRequire(process.env.PLAYWRIGHT_MODULES ?? '/Users/cristian/.npm/_npx/e41f203b7505f1fb/node_modules/');
const { chromium } = require('playwright');
const server = await serve(0);
const port = server.address().port;
const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader'] });
const context = await browser.newContext({ acceptDownloads: true, viewport: { width: 1200, height: 800 } });
const page = await context.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
let failed = false;
for (const category of ['characters', 'items', 'goals', 'world', 'props', 'effects', 'teaching']) {
  await page.goto(`http://localhost:${port}/?category=${category}`);
  await page.waitForFunction(() => window.__ready);
  const [download] = await Promise.all([page.waitForEvent('download', { timeout: 20000 }), page.getByRole('button', { name: 'Export GLB' }).click()]);
  const path = await download.path();
  const { readFile } = await import('node:fs/promises');
  const bytes = await readFile(path);
  const magic = bytes.subarray(0, 4).toString('ascii');
  const ok = magic === 'glTF' && bytes.length > 2000;
  failed ||= !ok;
  console.log(category, ok ? 'ok' : 'BAD', `${bytes.length} bytes`, download.suggestedFilename());
}
await browser.close();
server.close();
if (errors.length || failed) {
  console.error(errors.join('\n'));
  process.exit(1);
}
