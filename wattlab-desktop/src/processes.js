'use strict';
// Owned child processes for the desktop shell. Kept free of Electron so it can be
// tested with plain Node. Only processes started here are ever signalled.

const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const net = require('node:net');
const path = require('node:path');
const { spawn } = require('node:child_process');

const STOP_BUDGET_MS = 30_000;

class LaunchError extends Error {}

/** Per-launch API token; matches the API token rules (32–64 URL-safe chars). */
function generateToken() {
  return crypto.randomBytes(32).toString('base64url');
}

/** Ask the OS for a free loopback port so an existing API is never taken over. */
function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

function resolvePaths(root, env = process.env) {
  const target = env.CARGO_TARGET_DIR
    ? path.resolve(root, env.CARGO_TARGET_DIR)
    : path.join(root, 'wattlab-api', 'target');
  const apiCandidates = env.WATTLAB_API_BIN
    ? [env.WATTLAB_API_BIN]
    : [path.join(target, 'release', 'wattlab-api'), path.join(target, 'debug', 'wattlab-api')];
  const venvPython = path.join(root, '.venv', 'bin', 'python');
  return {
    root,
    api: apiCandidates.find((candidate) => fs.existsSync(candidate)) ?? null,
    python: fs.existsSync(venvPython) ? venvPython : 'python3',
    envFile: path.join(root, '.env'),
    localDb: path.join(root, 'scripts', 'local_db.py'),
    agent: path.join(root, 'experiments', 'bluetooth', 'trainer_agent.py'),
  };
}

/** Child output may echo configuration; never forward the token. */
function redact(text, secrets) {
  return secrets.filter(Boolean).reduce((value, secret) => value.split(secret).join('[REDACTED]'), text);
}

/**
 * Start a child in its own process group so shutdown can signal everything it
 * spawned without touching unrelated processes.
 */
function spawnOwned(name, command, args, { env, cwd, secrets = [], log = console.log } = {}) {
  const child = spawn(command, args, { cwd, env, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
  const forward = (chunk) => {
    for (const line of redact(chunk.toString(), secrets).split('\n')) {
      if (line.trim()) log(`[${name}] ${line}`);
    }
  };
  child.stdout.on('data', forward);
  child.stderr.on('data', forward);
  child.exited = new Promise((resolve) => {
    child.once('exit', (code, signal) => resolve({ code, signal }));
    child.once('error', () => resolve({ code: null, signal: null, error: true }));
  });
  return child;
}

function running(child) {
  return child.exitCode === null && child.signalCode === null && child.pid !== undefined;
}

/** SIGTERM the child's process group, escalate to SIGKILL after the budget. */
async function stopOwned(child, budgetMs = STOP_BUDGET_MS) {
  if (!child || !running(child)) return 'not_running';
  const signal = (name) => {
    try {
      process.kill(-child.pid, name);
    } catch {
      // Already gone.
    }
  };
  signal('SIGTERM');
  let timer;
  const timedOut = new Promise((resolve) => {
    timer = setTimeout(() => resolve('timeout'), budgetMs);
  });
  const result = await Promise.race([child.exited.then(() => 'stopped'), timedOut]);
  clearTimeout(timer);
  if (result === 'timeout') {
    signal('SIGKILL');
    await child.exited;
    return 'killed';
  }
  return 'stopped';
}

/** Run a setup step to completion (for example the local database helper). */
async function runStep(name, command, args, options) {
  const child = spawnOwned(name, command, args, options);
  const { code } = await child.exited;
  if (code !== 0) throw new LaunchError(`${name} failed; inspect the log output.`);
}

function getStatus(origin, token) {
  return new Promise((resolve) => {
    const request = http.get(
      `${origin}/api/v1/status`,
      { headers: { Authorization: `Bearer ${token}` }, timeout: 1000 },
      (response) => {
        let body = '';
        response.on('data', (chunk) => (body += chunk));
        response.on('end', () => {
          try {
            resolve({ status: response.statusCode, body: JSON.parse(body) });
          } catch {
            resolve({ status: response.statusCode, body: null });
          }
        });
      },
    );
    request.on('timeout', () => request.destroy());
    request.on('error', () => resolve(null));
  });
}

/** Ready means the authenticated status endpoint answers as wattlab-api. */
async function waitReady(child, origin, token, timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (!running(child)) throw new LaunchError('API exited before it was ready; inspect the log output.');
    const reply = await getStatus(origin, token);
    if (reply?.status === 401) throw new LaunchError('Local API refused the launch token.');
    if (reply?.status === 200 && reply.body?.service === 'wattlab-api' && reply.body?.status === 'ok') return;
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new LaunchError('API readiness timed out.');
}

module.exports = {
  LaunchError,
  STOP_BUDGET_MS,
  freePort,
  generateToken,
  redact,
  resolvePaths,
  runStep,
  running,
  spawnOwned,
  stopOwned,
  waitReady,
};
