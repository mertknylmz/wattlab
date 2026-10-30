'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createHandler } = require('../src/renderer-protocol');

function setup(api = { origin: 'http://127.0.0.1:9', token: 'launch-token' }) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'wattlab-renderer-'));
  fs.writeFileSync(path.join(dir, 'index.html'), '<!doctype html>');
  fs.writeFileSync(path.join(dir, 'app.js'), 'void 0');
  fs.writeFileSync(path.join(os.tmpdir(), 'wattlab-secret.js'), 'secret');
  const calls = [];
  const handler = createHandler({
    rendererDir: dir,
    api: () => api,
    fetch: async (url, init) => {
      calls.push({ url, init });
      return new Response('{}', { status: 200 });
    },
  });
  return { handler, calls };
}

test('serves bundled screens with a strict content security policy', async () => {
  const { handler } = setup();
  const index = await handler(new Request('wattlab://app/'));
  assert.equal(index.status, 200);
  assert.match(index.headers.get('content-type'), /text\/html/);
  assert.match(index.headers.get('content-security-policy'), /script-src 'self'/);
  assert.equal((await handler(new Request('wattlab://app/app.js'))).status, 200);
});

test('never serves files outside the renderer build', async () => {
  const { handler } = setup();
  for (const url of ['wattlab://app/../wattlab-secret.js', 'wattlab://app/%2e%2e/wattlab-secret.js', 'wattlab://other/app.js']) {
    assert.equal((await handler(new Request(url))).status, 404, url);
  }
});

test('forwards API calls with the launch token, replacing renderer headers', async () => {
  const { handler, calls } = setup();
  const response = await handler(
    new Request('wattlab://app/api/v1/settings?x=1', {
      method: 'PUT',
      headers: { Authorization: 'Bearer forged', Cookie: 'a=b', 'Content-Type': 'application/json' },
      body: '{"ftp_w":250}',
    }),
  );
  assert.equal(response.status, 200);
  assert.equal(calls[0].url, 'http://127.0.0.1:9/api/v1/settings?x=1');
  assert.equal(calls[0].init.headers.get('authorization'), 'Bearer launch-token');
  assert.equal(calls[0].init.headers.get('cookie'), null);
  assert.equal(Buffer.from(calls[0].init.body).toString(), '{"ftp_w":250}');
});

test('API calls before the API is ready or after it fails report unavailability', async () => {
  assert.equal((await setup(null).handler(new Request('wattlab://app/api/v1/status'))).status, 503);
  const failing = createHandler({
    rendererDir: os.tmpdir(),
    api: () => ({ origin: 'http://127.0.0.1:9', token: 't' }),
    fetch: async () => {
      throw new TypeError('connection refused');
    },
  });
  assert.equal((await failing(new Request('wattlab://app/api/v1/status'))).status, 502);
});
