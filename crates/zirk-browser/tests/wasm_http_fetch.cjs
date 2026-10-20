const assert = require('node:assert/strict');
const { BrowserApi } = require(process.argv[2]);

async function main() {
  const api = new BrowserApi();
  api.initialize();
  const source = `import { stdout } from std.io;
import { HTTP, HTTPHeaders, HTTPMethod, URL } from std.http;
fn main(): Void {
  mut body: Array<Byte> = [112 as Byte, 97 as Byte, 121 as Byte];
  mut headers = HTTPHeaders.create();
  _ = headers.add("x-test", "sent");
  match URL.parse("https://example.com/submit") {
    Ok(url) => match HTTP.request(HTTPMethod.Post, url, body, headers) {
      Ok(response) => {
        stdout.println(response.status_code());
        match response.text() {
          Ok(text) => stdout.println(text);
          Error(error) => stdout.println(error.to_string());
        }
      }
      Error(error) => stdout.println(error.to_string());
    }
    Error(_) => stdout.println("url-error");
  }
}`;
  const request = JSON.stringify({ apiVersion: 1, source, virtualName: 'wasm-fetch-check.zrk' });
  let fetchCalls = 0;
  let observed;
  const fetcher = async (url, options) => {
    fetchCalls++;
    observed = {
      url,
      method: options.method,
      body: [...options.body],
      header: options.headers.get('x-test'),
      credentials: options.credentials,
      redirect: options.redirect,
    };
    return new Response(
      new ReadableStream({ start(controller) { controller.enqueue(new TextEncoder().encode('ok')); controller.close(); } }),
      { status: 201, headers: { 'content-type': 'text/plain' } },
    );
  };

  const success = JSON.parse(await api.runWithFetch(
    request, fetcher, JSON.stringify(['https://example.com']), 1024, 1024, null,
  ));
  assert.equal(success.failure, null);
  assert.equal(success.completion, 'completed');
  assert.deepEqual(success.output.map(event => event.text), ['201\n', 'ok\n']);
  assert.deepEqual(observed, {
    url: 'https://example.com/submit',
    method: 'POST',
    body: [112, 97, 121],
    header: 'sent',
    credentials: 'omit',
    redirect: 'error',
  });

  const jsonPostRequest = JSON.stringify({
    apiVersion: 1,
    virtualName: 'wasm-json-post.zrk',
    source: `import { HTTP, URL } from std.http;
fn main(): Void {
  match URL.parse("https://example.com/json") {
    Ok(url) => { _ = HTTP.post_json(url, "true"); }
    Error(_) => {}
  }
}`,
  });
  let jsonPostObserved;
  const jsonPost = JSON.parse(await api.runWithFetch(
    jsonPostRequest,
    async (url, options) => {
      jsonPostObserved = {
        url,
        method: options.method,
        body: new TextDecoder().decode(options.body),
        contentType: options.headers.get('content-type'),
      };
      return new Response('json-ok', { status: 200 });
    },
    JSON.stringify(['https://example.com']),
    1024,
    1024,
    null,
  ));
  assert.equal(jsonPost.failure, null);
  assert.deepEqual(jsonPostObserved, {
    url: 'https://example.com/json',
    method: 'POST',
    body: 'true',
    contentType: 'application/json',
  });

  const queryRequest = JSON.stringify({
    apiVersion: 1,
    virtualName: 'wasm-query.zrk',
    source: `import { HTTP, URL } from std.http;
fn main(): Void {
  match URL.parse("https://example.com/query") {
    Ok(url) => { _ = HTTP.query(url, "true"); }
    Error(_) => {}
  }
}`,
  });
  let queryObserved;
  const queryResponse = JSON.parse(await api.runWithFetch(
    queryRequest,
    async (url, options) => {
      queryObserved = {
        url,
        method: options.method,
        body: new TextDecoder().decode(options.body),
        contentType: options.headers.get('content-type'),
      };
      return new Response(null, { status: 204 });
    },
    JSON.stringify(['https://example.com']),
    1024,
    1024,
    null,
  ));
  assert.equal(queryResponse.failure, null);
  assert.deepEqual(queryObserved, {
    url: 'https://example.com/query',
    method: 'QUERY',
    body: 'true',
    contentType: 'application/json',
  });

  const invalidJsonQuery = JSON.stringify({
    apiVersion: 1,
    virtualName: 'wasm-invalid-query.zrk',
    source: `import { stdout } from std.io;
import { HTTP, URL } from std.http;
fn main(): Void {
  match URL.parse("https://example.com/query") {
    Ok(url) => match HTTP.query(url, "not-json") {
      Ok(_) => stdout.println("unexpected success");
      Error(error) => stdout.println(error.to_string());
    }
    Error(_) => {}
  }
}`,
  });
  let invalidQueryFetchCalls = 0;
  const invalidQuery = JSON.parse(await api.runWithFetch(
    invalidJsonQuery,
    async () => { invalidQueryFetchCalls++; return new Response(null, { status: 200 }); },
    JSON.stringify(['https://example.com']),
    1024,
    1024,
    null,
  ));
  assert.equal(invalidQuery.failure, null);
  assert.equal(invalidQuery.output.at(-1)?.text, 'InvalidJson\n');
  assert.equal(invalidQueryFetchCalls, 0, 'invalid JSON QUERY must fail before Fetch');
  const afterCapability = JSON.parse(api.run(request));
  assert.equal(afterCapability.failure?.kind, 'capability');
  assert.equal(fetchCalls, 1, 'a later run must not retain the prior Fetch capability');

  const denied = JSON.parse(await api.runWithFetch(
    request, fetcher, JSON.stringify(['https://elsewhere.test']), 1024, 1024, null,
  ));
  assert.equal(denied.failure?.kind, 'capability');
  assert.equal(fetchCalls, 1, 'a denied origin must not invoke Fetch');

  let streamCancelled = false;
  const oversizedFetcher = async () => new Response(
    new ReadableStream({
      start(controller) { controller.enqueue(new TextEncoder().encode('response too large')); },
      cancel() { streamCancelled = true; },
    }),
    { status: 200 },
  );
  const oversized = JSON.parse(await api.runWithFetch(
    request, oversizedFetcher, JSON.stringify(['https://example.com']), 1024, 3, null,
  ));
  assert.equal(oversized.failure, null);
  assert.equal(oversized.output.at(-1)?.text, 'BodyTooLarge\n');
  assert.equal(streamCancelled, true, 'the response reader must be cancelled at the byte limit');

  const corsFailure = JSON.parse(await api.runWithFetch(
    request,
    async () => { throw new TypeError('Fetch rejected by browser policy'); },
    JSON.stringify(['https://example.com']),
    1024,
    1024,
    null,
  ));
  assert.equal(corsFailure.failure, null);
  assert.equal(corsFailure.output.at(-1)?.text, 'BrowserPolicy\n');

  const requestTooLarge = JSON.parse(await api.runWithFetch(
    request, fetcher, JSON.stringify(['https://example.com']), 2, 1024, null,
  ));
  assert.equal(requestTooLarge.output.at(-1)?.text, 'BodyTooLarge\n');
  assert.equal(fetchCalls, 1, 'an oversized request body must not invoke Fetch');

  const preAborted = new AbortController();
  preAborted.abort();
  const cancelledBeforeStart = JSON.parse(await api.runWithFetch(
    request, fetcher, JSON.stringify(['https://example.com']), 1024, 1024, preAborted.signal,
  ));
  assert.equal(cancelledBeforeStart.failure?.limit, 'cancelled');
  assert.equal(fetchCalls, 1, 'a pre-cancelled execution must not invoke Fetch');

  const duringFetch = new AbortController();
  let pendingFetchCalls = 0;
  const pendingFetcher = (_url, options) => {
    pendingFetchCalls++;
    return new Promise((_resolve, reject) => {
      options.signal.addEventListener('abort', () => reject(new Error('aborted')), { once: true });
    });
  };
  const pending = api.runWithFetch(
    request, pendingFetcher, JSON.stringify(['https://example.com']), 1024, 1024, duringFetch.signal,
  );
  setTimeout(() => duringFetch.abort(), 0);
  const cancelledDuringFetch = JSON.parse(await pending);
  assert.equal(cancelledDuringFetch.failure?.limit, 'cancelled');
  assert.equal(pendingFetchCalls, 1);

  console.log(JSON.stringify({
    success: success.output,
    jsonPost: jsonPostObserved,
    query: queryObserved,
    invalidJsonQuery: invalidQuery.output,
    afterCapability: afterCapability.failure,
    denied: denied.failure,
    oversized: oversized.output,
    responseStreamCancelled: streamCancelled,
    corsFailure: corsFailure.output,
    requestTooLarge: requestTooLarge.output,
    cancelledBeforeStart: cancelledBeforeStart.failure,
    cancelledDuringFetch: cancelledDuringFetch.failure,
  }, null, 2));
  api.free();
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
