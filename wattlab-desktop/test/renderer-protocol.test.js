'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createHandler } = require('../src/renderer-protocol');
const { createSessionStore } = require('../src/session-store');

function memorySession(initial = null, rememberAvailable = true) {
  let token = initial;
  const events = [];
  return {
    events,
    token: () => token,
    set(value, remember) {
      token = value;
      events.push(['set', value, remember]);
    },
    clear() {
      token = null;
      events.push(['clear']);
    },
    rememberAvailable: () => rememberAvailable,
  };
}

function setup({ api = { origin: 'http://127.0.0.1:9' }, session = memorySession('member-token'), fetchImpl } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'wattlab-renderer-'));
  fs.writeFileSync(path.join(dir, 'index.html'), '<!doctype html>');
  fs.writeFileSync(path.join(dir, 'app.js'), 'void 0');
  fs.writeFileSync(path.join(os.tmpdir(), 'wattlab-secret.js'), 'secret');
  const calls = [];
  const handler = createHandler({
    rendererDir: dir,
    api: () => api,
    session,
    fetch: fetchImpl || (async (url, init) => {
      calls.push({ url, init });
      return new Response('{}', { status: 200 });
    }),
  });
  return { handler, calls, session };
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

test('forwards member API calls with the login token, replacing renderer headers', async () => {
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
  assert.equal(calls[0].init.headers.get('authorization'), 'Bearer member-token');
  assert.equal(calls[0].init.headers.get('cookie'), null);
  assert.equal(Buffer.from(calls[0].init.body).toString(), '{"ftp_w":250}');
});

test('keeps login and register public and strips the token from the renderer response', async () => {
  const session = memorySession(null);
  const { handler, calls } = setup({
    session,
    fetchImpl: async (url, init) => {
      calls.push({ url, init });
      return new Response(JSON.stringify({ member: { username: 'holy' }, token: 'secret-login', expires_at: 't' }), {
        status: url.endsWith('/register') ? 201 : 200,
      });
    },
  });
  const registered = await handler(
    new Request('wattlab://app/api/v1/auth/register', {
      method: 'POST',
      headers: { 'X-WattLab-Remember': '1', Authorization: 'Bearer forged' },
      body: '{"username":"holy","password":"correct horse"}',
    }),
  );
  assert.equal(registered.status, 201);
  assert.deepEqual(await registered.json(), { member: { username: 'holy' }, expires_at: 't' });
  assert.equal(calls[0].init.headers.get('authorization'), null);
  assert.equal(calls[0].init.headers.get('x-wattlab-remember'), null);
  assert.deepEqual(session.events, [['set', 'secret-login', true]]);
});

test('forgets the token on logout or a later 401, but not on a wrong password', async () => {
  const session = memorySession('member-token');
  const { handler } = setup({
    session,
    fetchImpl: async (url) => {
      if (url.includes('/logout')) return new Response(null, { status: 204 });
      if (url.includes('/login')) return new Response(JSON.stringify({ error: 'invalid_credentials' }), { status: 401 });
      return new Response(JSON.stringify({ error: 'unauthorized' }), { status: 401 });
    },
  });
  assert.equal((await handler(new Request('wattlab://app/api/v1/auth/logout', { method: 'POST' }))).status, 204);
  assert.deepEqual(session.events, [['clear']]);
  session.set('member-token', false);
  session.events.length = 0;
  assert.equal((await handler(new Request('wattlab://app/api/v1/auth/login', { method: 'POST', body: '{}' }))).status, 401);
  assert.deepEqual(session.events, []);
  assert.equal((await handler(new Request('wattlab://app/api/v1/settings'))).status, 401);
  assert.deepEqual(session.events, [['clear']]);
});

test('reports whether remember-me storage is available without contacting the API', async () => {
  const { handler, calls } = setup({ session: memorySession(null, false) });
  const response = await handler(new Request('wattlab://app/api/v1/desktop/capabilities'));
  assert.deepEqual(await response.json(), { remember_available: false });
  assert.equal(calls.length, 0);
});

test('API calls before the API is ready or after it fails report unavailability', async () => {
  assert.equal((await setup({ api: null }).handler(new Request('wattlab://app/api/v1/status'))).status, 503);
  const failing = createHandler({
    rendererDir: os.tmpdir(),
    api: () => ({ origin: 'http://127.0.0.1:9' }),
    session: memorySession('t'),
    fetch: async () => {
      throw new TypeError('connection refused');
    },
  });
  assert.equal((await failing(new Request('wattlab://app/api/v1/status'))).status, 502);
});

test('session store writes only when the OS can encrypt the token', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'wattlab-session-'));
  const file = path.join(dir, 'session.bin');
  const cipher = {
    isEncryptionAvailable: () => true,
    getSelectedStorageBackend: () => 'gnome_libsecret',
    encryptString: (value) => Buffer.from(`enc:${value}`),
    decryptString: (buf) => buf.toString().slice(4),
  };
  const store = createSessionStore({ fs, path, safeStorage: cipher, file });
  assert.equal(store.save('abc'), true);
  assert.equal(store.load(), 'abc');
  const plain = {
    isEncryptionAvailable: () => true,
    getSelectedStorageBackend: () => 'basic_text',
    encryptString: () => Buffer.from('nope'),
    decryptString: () => 'nope',
  };
  const unsafe = createSessionStore({ fs, path, safeStorage: plain, file: path.join(dir, 'plain.bin') });
  assert.equal(unsafe.available(), false);
  assert.equal(unsafe.save('abc'), false);
  assert.equal(fs.existsSync(path.join(dir, 'plain.bin')), false);
});
