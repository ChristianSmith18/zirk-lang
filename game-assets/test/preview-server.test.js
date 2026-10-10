import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// Regression: the server used to exit silently when the checkout path held a
// space, because the "run directly" check compared an encoded URL to a raw path.
test('the preview server starts from the command line and serves the page, three and the assets', async () => {
  const script = fileURLToPath(new URL('../preview/serve.mjs', import.meta.url));
  const child = spawn(process.execPath, [script, '0'], { stdio: ['ignore', 'pipe', 'pipe'] });
  try {
    const url = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('server did not announce a URL')), 8000);
      child.stdout.on('data', (chunk) => {
        const match = /http:\/\/localhost:(\d+)\//.exec(String(chunk));
        if (match) {
          clearTimeout(timer);
          resolve(match[0]);
        }
      });
      child.on('exit', (code) => reject(new Error(`server exited early with code ${code}`)));
    });
    for (const path of ['', 'src/index.js', 'vendor/three/build/three.module.js']) {
      const response = await fetch(url + path);
      assert.equal(response.status, 200, path);
    }
    const escape = await fetch(url + '..%2F..%2Fetc%2Fpasswd');
    assert.notEqual(escape.status, 200, 'must not serve files outside the package');
  } finally {
    child.kill();
  }
});
