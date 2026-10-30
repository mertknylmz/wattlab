'use strict';
// Electron main process: a process manager only. Bluetooth stays in the Python
// agent, business rules in the Rust API. The window shows the bundled screens
// from wattlab://app/; their /api requests are forwarded to the local API.

const fs = require('node:fs');
const path = require('node:path');
const { app, BrowserWindow, net, protocol, session } = require('electron');
const {
  LaunchError,
  freePort,
  generateToken,
  resolvePaths,
  runStep,
  running,
  spawnOwned,
  stopOwned,
  waitReady,
} = require('./processes');
const { ORIGIN, createHandler } = require('./renderer-protocol');

// API, Python agent and PostgreSQL are not bundled yet, so a packaged build
// needs WATTLAB_ROOT pointing at a prepared WattLab checkout.
const ROOT = process.env.WATTLAB_ROOT
  ? path.resolve(process.env.WATTLAB_ROOT)
  : app.isPackaged
    ? null
    : path.resolve(__dirname, '..', '..');
const RENDERER = path.join(__dirname, '..', 'renderer');
const state = { api: null, agent: null, origin: null, token: null, window: null, quitting: false, stopped: false };

protocol.registerSchemesAsPrivileged([
  { scheme: 'wattlab', privileges: { standard: true, secure: true, supportFetchAPI: true, stream: true } },
]);

function log(line) {
  console.log(line);
}

function showStatus(message, error = false) {
  if (!state.window || state.window.isDestroyed()) return;
  void state.window.loadFile(path.join(__dirname, 'loading.html'), { query: { message, error: error ? '1' : '0' } });
}

function createWindow() {
  const window = new BrowserWindow({
    width: 1280,
    height: 860,
    minWidth: 380,
    backgroundColor: '#101412',
    title: 'WattLab',
    autoHideMenuBar: true,
    webPreferences: {
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      webSecurity: true,
    },
  });
  // Only the bundled screens and the local status page may be shown.
  window.webContents.on('will-navigate', (event, url) => {
    const target = new URL(url);
    if (target.protocol === 'file:' || target.origin === ORIGIN) return;
    event.preventDefault();
  });
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  return window;
}

async function start() {
  session.defaultSession.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  protocol.handle(
    'wattlab',
    createHandler({
      rendererDir: RENDERER,
      api: () => (state.origin && state.token ? { origin: state.origin, token: state.token } : null),
      fetch: (url, init) => net.fetch(url, init),
    }),
  );

  state.window = createWindow();
  state.window.on('closed', () => {
    state.window = null;
  });
  showStatus('Yerel servisler başlatılıyor…');

  try {
    if (!ROOT) {
      throw new LaunchError('Paketlenmiş sürüm için WATTLAB_ROOT ile hazırlanmış WattLab klasörünü belirt.');
    }
    if (!fs.existsSync(path.join(RENDERER, 'index.html'))) {
      throw new LaunchError('Ekranlar derlenmemiş. wattlab-ui içinde "npm run build" çalıştır.');
    }
    const paths = resolvePaths(ROOT);
    if (!paths.api) {
      throw new LaunchError('WattLab API derlenmemiş. wattlab-api içinde "cargo build" çalıştır.');
    }
    showStatus('Yerel veritabanı hazırlanıyor…');
    await runStep('database', paths.python, [paths.localDb, 'start'], { cwd: ROOT, env: process.env, log });

    const token = generateToken();
    const port = await freePort();
    const origin = `http://127.0.0.1:${port}`;
    const env = {
      ...process.env,
      WATTLAB_API_PORT: String(port),
      WATTLAB_API_TOKEN: token,
      WATTLAB_API_URL: origin,
      WATTLAB_ENV_FILE: paths.envFile,
    };
    const owned = { cwd: ROOT, env, secrets: [token], log };

    showStatus('WattLab API başlatılıyor…');
    state.api = spawnOwned('api', paths.api, [], owned);
    await waitReady(state.api, origin, token);
    state.origin = origin;
    state.token = token;
    state.api.exited.then(() => {
      if (!state.quitting) showStatus('WattLab API beklenmedik şekilde durdu. Uygulamayı yeniden başlat.', true);
    });

    // The agent only touches Bluetooth after the user presses "connect" on the screen.
    if (process.env.WATTLAB_DESKTOP_TRAINER !== '0') {
      state.agent = spawnOwned('agent', paths.python, ['-u', paths.agent], owned);
      state.agent.exited.then(() => {
        if (!state.quitting) log('[desktop] trainer agent stopped; the screen shows it as not running.');
      });
    }
    if (state.window) await state.window.loadURL(`${ORIGIN}/#/live`);
  } catch (error) {
    const message = error instanceof LaunchError ? error.message : 'Yerel başlatma başarısız oldu; günlük çıktısını incele.';
    log(`[desktop] ${message}`);
    showStatus(message, true);
  }
}

/** Agent first so it can finish the session and release the trainer while the API is up. */
async function shutdown() {
  for (const [name, child] of [['agent', state.agent], ['api', state.api]]) {
    if (child && running(child)) log(`[desktop] ${name}: ${await stopOwned(child)}`);
  }
}

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.enableSandbox();
  app.on('second-instance', () => {
    if (state.window) {
      if (state.window.isMinimized()) state.window.restore();
      state.window.focus();
    }
  });
  app.whenReady().then(start);
  app.on('window-all-closed', () => app.quit());
  app.on('before-quit', (event) => {
    if (state.stopped) return;
    event.preventDefault();
    if (state.quitting) return;
    state.quitting = true;
    shutdown().finally(() => {
      state.stopped = true;
      app.quit();
    });
  });
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => app.quit());
}
