// Browser state tests for outages, stale data and overlapping requests; no packages.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const TOKEN = 'test-only-token-for-integration-tests';
function browser() {
  let clock = 0; const nodes = new Map(), calls = [], timers = new Map(); let next = 0;
  function node() { return { textContent: '', value: '', hidden: false, className: '', children: [], attributes: {}, handlers: {}, addEventListener(name, fn) { this.handlers[name] = fn; }, replaceChildren() { this.children = []; }, append(child) { this.children.push(child); }, setAttribute(name, value) { this.attributes[name] = value; } }; }
  const context = vm.createContext({ document: { getElementById(id) { if (!nodes.has(id)) nodes.set(id, node()); return nodes.get(id); }, createElementNS: () => node() }, window: { addEventListener() {} }, performance: { now: () => clock }, Intl, Date, AbortController,
    setTimeout: (fn) => { timers.set(++next, fn); return next; }, clearTimeout: (id) => timers.delete(id), setInterval() {},
    fetch: (url, options) => new Promise((resolve, reject) => calls.push({ url, options, resolve, reject })),
  });
  vm.runInContext(fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8'), context);
  return { context, nodes, calls, timers, clock(v) { clock = v; }, run(s) { return vm.runInContext(s, context); }, connect(value = TOKEN) { context.document.getElementById('token').value = value; nodes.get('connect-form').handlers.submit({ preventDefault() {} }); } };
}
const settle = () => new Promise((resolve) => setImmediate(resolve));
function data() { const now = new Date().toISOString(); return { session: { id: 'session-one', started_at: now, finished_at: null }, captured_at: now, ftms_received_at: now, cycling_power_received_at: now, connection_state: 'streaming', power_source: 'cycling_power', power_w: -12, speed_kmh: 0, cadence_rpm: null }; }
function respond(call, body = data(), status = 200) { call.resolve({ ok: status === 200, status, json: async () => body }); }
test('local authentication sends only a Bearer header and clears the form', async () => {
  const b = browser(); b.connect(); assert.equal(b.calls.length, 1); const request = b.calls[0];
  assert.equal(request.url, '/api/v1/live'); assert.equal(request.options.headers.Authorization, `Bearer ${TOKEN}`); assert.equal(request.options.credentials, 'omit'); assert.equal(b.nodes.get('token').value, '');
  respond(request); await settle(); assert.equal(b.nodes.get('power').textContent, '-12'); assert.equal(b.nodes.get('speed').textContent, '0'); assert.equal(b.nodes.get('cadence').textContent, '—');
  assert.equal(b.nodes.get('access').hidden, true);
});
test('values expire while a request stalls, without waiting for the next reply', async () => {
  const b = browser(); b.connect(); respond(b.calls[0]); await settle(); b.clock(5001); b.run('render()');
  assert.equal(b.nodes.get('power').textContent, '—'); assert.equal(b.nodes.get('speed').textContent, '—'); assert.equal(b.nodes.get('trainer-status').textContent, 'Veri bekleniyor');
});
test('API failure clears measurements and schedules automatic retry', async () => {
  const b = browser(); b.connect(); respond(b.calls[0]); await settle(); b.run('poll(generation)'); b.calls[1].reject(new Error('network')); await settle();
  assert.equal(b.nodes.get('power').textContent, '—'); assert.equal(b.nodes.get('api-status').textContent, 'API’ye ulaşılamıyor'); assert.ok(b.timers.size > 0);
});
test('unauthorized reply forgets the token instead of retrying forever', async () => {
  const b = browser(); b.connect(); respond(b.calls[0], {}, 401); await settle(); assert.equal(b.run('token'), ''); assert.equal(b.nodes.get('access').hidden, false); assert.equal(b.nodes.get('disconnect').hidden, true); assert.equal(b.timers.size, 0);
});
test('a late response from an earlier connection cannot replace a new view', async () => {
  const b = browser(); b.connect(); const old = b.calls[0]; b.connect(); const latest = data(); latest.power_w = 200;
  respond(b.calls[1], latest); await settle(); respond(old); await settle(); assert.equal(b.nodes.get('power').textContent, '200');
  b.nodes.get('disconnect').handlers.click(); assert.equal(b.nodes.get('power').textContent, '—'); assert.equal(b.run('token'), '');
});
test('finished sessions and reconnecting trainers hide even recently received metrics', async () => {
  for (const mutate of [(s) => { s.session.finished_at = s.captured_at; }, (s) => { s.connection_state = 'retry_wait'; }]) {
    const b = browser(); b.connect(); const snapshot = data(); mutate(snapshot); respond(b.calls[0], snapshot); await settle(); assert.equal(b.nodes.get('power').textContent, '—'); assert.equal(b.nodes.get('speed').textContent, '—');
  }
});

test('chart distinguishes waiting for samples from a real zero-power trace', async () => {
  const b = browser(); b.run('drawChart()'); assert.equal(b.nodes.get('chart-empty').hidden, false);
  b.connect(); const snapshot = data(); snapshot.power_w = 0; respond(b.calls[0], snapshot); await settle();
  b.run('history = [{ time: 0, value: 0 }, { time: 1000, value: 0 }]; drawChart()');
  assert.equal(b.nodes.get('chart-empty').hidden, true); assert.equal(b.nodes.get('power-chart').children.length, 1);
});

test('a real zero reply is captured before a finish reply between chart ticks', async () => {
  const b = browser(); b.connect(); const initial = data(); initial.power_w = 150;
  respond(b.calls[0], initial); await settle();
  b.clock(1000); b.run('poll(generation)'); const zero = data(); zero.power_w = 0;
  respond(b.calls[1], zero); await settle();
  assert.equal(b.nodes.get('power').textContent, '0');
  assert.match(b.nodes.get('chart-status').textContent, /0 W/);
  b.clock(1100); b.run('poll(generation)'); const finished = data(); finished.session.finished_at = finished.captured_at;
  respond(b.calls[2], finished); await settle();
  assert.deepEqual(Array.from(b.run('history.map(p => p.value)')), [150, 0, null]);
  assert.equal(b.nodes.get('power-chart').children.length, 1);
  const points = b.nodes.get('power-chart').children[0].attributes.points.split(' ');
  assert.equal(Number(points.at(-1).split(',')[1]), 142);
  assert.match(b.nodes.get('chart-status').textContent, /tamamlandı/);
});
test('an outage leaves a gap and never fabricates a drop to zero', async () => {
  const b = browser(); b.connect(); const initial = data(); initial.power_w = 150;
  respond(b.calls[0], initial); await settle();
  b.clock(1000); b.run('poll(generation)'); b.calls[1].reject(new Error('network')); await settle();
  assert.deepEqual(Array.from(b.run('history.map(p => p.value)')), [150, null]);
  assert.match(b.nodes.get('chart-status').textContent, /boşluk sıfır watt anlamına gelmez/);
});

test('default FTP zone boundaries, zero and missing readings are explicit', () => {
  const b = browser();
  for (const [watts, zone] of [[0,0],[1,1],[119,1],[120,2],[151,2],[152,3],[179,3],[180,4],[209,4],[210,5],[237,5],[238,6]]) assert.equal(b.run(`powerZone(${watts})`),zone);
  assert.equal(b.run('powerZone(null)'),null);assert.equal(b.run('powerZone(-1)'),null);
  assert.match(b.nodes.get('ftp-status').textContent,/Varsayılan.*200 W/);
});
test('custom FTP recolors display and chart without changing measured watts', async () => {
  const b=browser();b.connect();const snapshot=data();snapshot.power_w=180;respond(b.calls[0],snapshot);await settle();
  b.clock(1000);b.run('poll(generation)');respond(b.calls[1],snapshot);await settle();
  assert.match(b.nodes.get('zone-label').textContent,/Z4/);
  b.context.document.getElementById('ftp').value='300';b.nodes.get('ftp-form').handlers.submit({preventDefault(){}});
  assert.match(b.nodes.get('zone-label').textContent,/Z2/);assert.equal(b.nodes.get('power').textContent,'180');
  assert.equal(b.nodes.get('power-chart').children[0].attributes.stroke,'#55b8f3');
  for(const invalid of ['', '0','-2','2010','12.5','NaN']) {b.nodes.get('ftp').value=invalid;b.nodes.get('ftp-form').handlers.submit({preventDefault(){}});assert.equal(b.run('ftp'),300);}
  b.nodes.get('ftp-reset').handlers.click();assert.equal(b.run('ftp'),200);assert.match(b.nodes.get('ftp-status').textContent,/Varsayılan/);
});
test('zone-colored graph segments still break at missing data', () => {
  const b=browser();b.clock(5000);b.run('history=[{time:0,value:80},{time:1000,value:150},{time:2000,value:null},{time:3000,value:230},{time:4000,value:250}];drawChart()');
  const lines=b.nodes.get('power-chart').children;
  assert.equal(lines.length,2);assert.equal(lines[0].attributes.stroke,'#55b8f3');assert.equal(lines[1].attributes.stroke,'#f17474');
});
