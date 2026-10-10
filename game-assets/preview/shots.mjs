// Renders one screenshot per category with Playwright: node preview/shots.mjs OUT_DIR
import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
import { serve } from './serve.mjs';

const require = createRequire(process.env.PLAYWRIGHT_MODULES ?? '/Users/cristian/.npm/_npx/e41f203b7505f1fb/node_modules/');
const { chromium } = require('playwright');
const out = process.argv[2] ?? 'preview/shots';
await mkdir(out, { recursive: true });
const server = await serve(0);
const port = server.address().port;
const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader'] });
const page = await browser.newPage({ viewport: { width: 1400, height: 860 } });
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
for (const category of ['characters', 'items', 'goals', 'world', 'props', 'effects', 'teaching']) {
  await page.goto(`http://localhost:${port}/?category=${category}&shot=1&warm=45`);
  await page.waitForFunction(() => window.__ready, null, { timeout: 30000 });
  await page.waitForTimeout(400);
  await page.screenshot({ path: `${out}/${category}.png` });
  console.log('shot', category);
}
await browser.close();
server.close();
if (errors.length) {
  console.error(errors.join('\n'));
  process.exit(1);
}
