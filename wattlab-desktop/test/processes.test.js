'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');
const {
  LaunchError,
  freePort,
  generateToken,
  redact,
  resolvePaths,
  running,
  spawnOwned,
  stopOwned,
  waitReady,
} = require('../src/processes');

const quiet = () => {};

test('launch tokens satisfy the API token rules and differ per launch', () => {
  const first = generateToken();
  assert.match(first, /^[A-Za-z0-9_-]{32,64}$/);
  assert.notEqual(first, generateToken());
});

test('child output never forwards the token', () => {
  assert.equal(redact('token=abc abc', ['abc']), 'token=[REDACTED] [REDACTED]');
});

test('free ports are bindable loopback ports', async () => {
  const port = await freePort();
  assert.ok(port > 0 && port < 65536);
});

test('paths prefer a release API build and fall back to python3', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'wattlab-desktop-'));
  assert.equal(resolvePaths(root, {}).api, null);
  assert.equal(resolvePaths(root, {}).python, 'python3');
  for (const profile of ['debug', 'release']) {
    const dir = path.join(root, 'wattlab-api', 'target', profile);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'wattlab-api'), '');
  }
  assert.equal(resolvePaths(root, {}).api, path.join(root, 'wattlab-api', 'target', 'release', 'wattlab-api'));
  fs.rmSync(root, { recursive: true });
});

test('stop signals the whole owned process group', async () => {
  const child = spawnOwned('sleep', 'sh', ['-c', 'sleep 30 & wait'], { log: quiet });
  assert.equal(await stopOwned(child, 5000), 'stopped');
  assert.equal(running(child), false);
});

test('stop escalates to SIGKILL after the budget', async () => {
  const child = spawnOwned('stubborn', 'sh', ['-c', 'trap "" TERM; sleep 30 & wait; sleep 30'], { log: quiet });
  await new Promise((resolve) => setTimeout(resolve, 200));
  assert.equal(await stopOwned(child, 300), 'killed');
});

test('stopping a finished child is a no-op', async () => {
  const child = spawnOwned('done', 'true', [], { log: quiet });
  await child.exited;
  assert.equal(await stopOwned(child, 100), 'not_running');
});

function statusServer(handler) {
  return new Promise((resolve) => {
    const server = http.createServer(handler);
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

test('readiness requires the authenticated status endpoint', async () => {
  let seen;
  const server = await statusServer((request, response) => {
    seen = request.headers.authorization;
    response.end(JSON.stringify({ service: 'wattlab-api', status: 'ok' }));
  });
  const child = spawnOwned('placeholder', 'sleep', ['5'], { log: quiet });
  await waitReady(child, `http://127.0.0.1:${server.address().port}`, 'secret-token', 2000);
  assert.equal(seen, 'Bearer secret-token');
  await stopOwned(child, 1000);
  server.close();
});

test('readiness fails fast on a refused token and on an exited API', async () => {
  const server = await statusServer((_request, response) => {
    response.statusCode = 401;
    response.end('{}');
  });
  const child = spawnOwned('placeholder', 'sleep', ['5'], { log: quiet });
  await assert.rejects(waitReady(child, `http://127.0.0.1:${server.address().port}`, 't', 2000), LaunchError);
  await stopOwned(child, 1000);
  server.close();
  const gone = spawnOwned('gone', 'true', [], { log: quiet });
  await gone.exited;
  await assert.rejects(waitReady(gone, 'http://127.0.0.1:1', 't', 2000), /exited/);
});
