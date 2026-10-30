'use strict';
// Handler for the app's private wattlab://app/ scheme. Static files come from the
// bundled renderer build; /api/* is forwarded to the local API with the
// per-launch token added here, so the renderer never sees a credential.

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

function status(code) {
  return new Response(null, { status: code });
}

/**
 * @param {object} options
 * @param {string} options.rendererDir absolute path of the built screens
 * @param {() => ({origin: string, token: string} | null)} options.api current API, null while starting
 * @param {typeof fetch} options.fetch network fetch used for forwarding (Electron's net.fetch)
 */
function createHandler({ rendererDir, api, fetch }) {
  const root = path.resolve(rendererDir);
  return async (request) => {
    const url = new URL(request.url);
    if (url.host !== 'app') return status(404);

    if (url.pathname.startsWith('/api/')) {
      const target = api();
      if (!target) return status(503);
      const headers = new Headers(request.headers);
      headers.delete('cookie');
      headers.set('Authorization', `Bearer ${target.token}`);
      const init = { method: request.method, headers, redirect: 'manual' };
      if (request.method !== 'GET' && request.method !== 'HEAD') init.body = await request.arrayBuffer();
      try {
        return await fetch(`${target.origin}${url.pathname}${url.search}`, init);
      } catch {
        return status(502);
      }
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
