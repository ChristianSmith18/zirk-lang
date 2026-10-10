// Minimal static server for the preview: node preview/serve.mjs [port]
import http from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
const types = { '.js': 'text/javascript', '.html': 'text/html', '.json': 'application/json', '.css': 'text/css' };

export function serve(port = 5173) {
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, 'http://localhost').pathname;
    let file;
    if (url === '/') file = join(root, 'preview/index.html');
    else if (url.startsWith('/vendor/three/')) file = join(root, 'node_modules/three', url.slice('/vendor/three/'.length));
    else file = join(root, url);
    file = normalize(file);
    // Never serve outside the package.
    if (!file.startsWith(root)) {
      res.writeHead(403).end();
      return;
    }
    try {
      const body = await readFile(file);
      res.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream' });
      res.end(body);
    } catch {
      res.writeHead(404).end();
    }
  });
  return new Promise((done) => server.listen(port, () => done(server)));
}

// `pathToFileURL` percent-encodes like `import.meta.url` does, so this also
// works when the checkout path contains spaces.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const server = await serve(Number(process.argv[2] ?? 5173));
  console.log(`Zirk asset preview: http://localhost:${server.address().port}/`);
}
