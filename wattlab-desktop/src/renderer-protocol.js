'use strict';
// Handler for the app's private wattlab://app/ scheme. Static files come from the
// bundled renderer build; /api/* is forwarded to the local API. Login tokens stay
// in this process: the renderer never sees a credential.

const fs = require('node:fs/promises');
const path = require('node:path');

const ORIGIN = 'wattlab://app';
const CSP =
  "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; " +
  "object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
};
const PUBLIC_AUTH = new Set(['/api/v1/auth/login', '/api/v1/auth/register']);

function status(code) {
  return new Response(null, { status: code });
}

function json(body, code = 200) {
  return new Response(JSON.stringify(body), {
    status: code,
    headers: { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' },
  });
}

/**
 * @param {object} options
 * @param {string} options.rendererDir
 * @param {() => ({origin: string} | null)} options.api
 * @param {object} options.session { token(), set(token, remember), clear(), rememberAvailable() }
 * @param {typeof fetch} options.fetch
 */
function createHandler({ rendererDir, api, session, fetch }) {
  const root = path.resolve(rendererDir);
  return async (request) => {
    const url = new URL(request.url);
    if (url.host !== 'app') return status(404);

    if (url.pathname === '/api/v1/desktop/capabilities') {
      return json({ remember_available: Boolean(session?.rememberAvailable()) });
    }

    if (url.pathname.startsWith('/api/')) {
      const target = api();
      if (!target) return status(503);
      const headers = new Headers(request.headers);
      headers.delete('cookie');
      headers.delete('authorization');
      const remember = headers.get('x-wattlab-remember') === '1';
      headers.delete('x-wattlab-remember');
      const publicAuth = PUBLIC_AUTH.has(url.pathname);
      const token = session?.token();
      if (!publicAuth && token) headers.set('Authorization', `Bearer ${token}`);
      const init = { method: request.method, headers, redirect: 'manual' };
      if (request.method !== 'GET' && request.method !== 'HEAD') init.body = await request.arrayBuffer();
      let response;
      try {
        response = await fetch(`${target.origin}${url.pathname}${url.search}`, init);
      } catch {
        return status(502);
      }
      if (publicAuth && response.ok) {
        const body = await response.json().catch(() => null);
        if (body && typeof body.token === 'string') {
          session?.set(body.token, remember);
          delete body.token;
          return json(body, response.status);
        }
      }
      if (url.pathname === '/api/v1/auth/logout' && response.status === 204) session?.clear();
      if (!publicAuth && response.status === 401) session?.clear();
      return response;
    }

    let relative;
    try {
      relative = decodeURIComponent(url.pathname === '/' ? '/index.html' : url.pathname);
    } catch {
      return status(400);
    }
    const file = path.resolve(root, `.${relative}`);
    if (!file.startsWith(root + path.sep)) return status(404);
    const type = TYPES[path.extname(file)];
    if (!type) return status(404);
    try {
      const body = await fs.readFile(file);
      return new Response(body, {
        headers: {
          'Content-Type': type,
          'Content-Security-Policy': CSP,
          'Cache-Control': 'no-store',
          'X-Content-Type-Options': 'nosniff',
        },
      });
    } catch {
      return status(404);
    }
  };
}

module.exports = { ORIGIN, createHandler };
