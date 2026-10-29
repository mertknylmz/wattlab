const $ = (id) => document.getElementById(id);
let token = '', snapshot = null, sampledAt = 0, controller = null, timer = null, generation = 0;
let history = [], sessionId = null, online = false;
let ftp = 200, customFtp = false;
const zoneColors = ['#b3bcb2', '#9aa6b2', '#55b8f3', '#79d68b', '#ebd35e', '#f5a15e', '#f17474'];
const zoneNames = ['Boşta', 'Toparlanma', 'Dayanıklılık', 'Tempo', 'Eşik', 'Yüksek yoğunluk', 'Anaerobik'];
function powerZone(watts) {
  if (watts == null || !Number.isFinite(watts) || watts < 0) return null;
  if (watts === 0) return 0;
  const percent = watts / ftp * 100;
  return percent < 60 ? 1 : percent < 76 ? 2 : percent < 90 ? 3 : percent < 105 ? 4 : percent < 119 ? 5 : 6;
}
function renderZones(watts) {
  const zone = powerZone(watts);
  $('power-card').className = `metric power${zone == null ? '' : ` zone-${zone}`}`;
  $('zone-label').textContent = zone == null ? 'Zon —' : `Z${zone} · ${zoneNames[zone]}`;
  $('zone-percent').textContent = watts == null || watts < 0 ? 'Güncel zon yok' : `%${format.format(watts / ftp * 100)} FTP`;
  $('ftp-status').textContent = `${customFtp ? 'Bu sekmenin ayarı' : 'Varsayılan referans'} · ${ftp} W${customFtp ? '' : ' · kişisel ölçüm değildir'}`;
}
const format = new Intl.NumberFormat('tr-TR', { maximumFractionDigits: 1 });
const states = { connecting: 'Bağlanıyor', subscribing: 'Veri kanalları hazırlanıyor', streaming: 'Veri bekleniyor', retry_wait: 'Yeniden bağlantı bekleniyor', stopped: 'Trainer dinleyicisi durdu' };
function badge(text, type = '') { $('api-status').textContent = text; $('api-status').className = `badge ${type}`; }
function currentMetric(key) {
  if (!online || !snapshot?.session || snapshot.session.finished_at || snapshot.connection_state !== 'streaming') return null;
  const receipt = key === 'power_w' && snapshot.power_source === 'cycling_power' ? snapshot.cycling_power_received_at : snapshot.ftms_received_at;
  if (!receipt || !snapshot.captured_at) return null;
  const age = Date.parse(snapshot.captured_at) - Date.parse(receipt) + performance.now() - sampledAt;
  return age >= 0 && age <= 5000 ? snapshot[key] : null;
}
function render() {
  const values = { power: currentMetric('power_w'), cadence: currentMetric('cadence_rpm'), speed: currentMetric('speed_kmh') };
  for (const [id, value] of Object.entries(values)) $(id).textContent = value == null ? '—' : format.format(value);
  renderZones(values.power);
  $('power-source').textContent = values.power == null ? 'Güncel güç ölçümü yok' : snapshot.power_source === 'cycling_power' ? 'Cycling Power kaynağı' : 'FTMS kaynağı';
  $('cadence-note').textContent = values.cadence == null ? 'Güncel ölçüm yok' : 'FTMS · devir / dakika';
  $('speed-note').textContent = values.speed == null ? 'Güncel ölçüm yok' : 'FTMS · kilometre / saat';
  const live = Object.values(values).some((value) => value != null);
  $('trainer-dot').className = `status-dot${live ? ' live' : ''}`;
  const session = snapshot?.session;
  $('trainer-status').textContent = !online ? 'Ekran bağlantısı yok' : !session ? 'Trainer oturumu bekleniyor' : session.finished_at ? 'Oturum tamamlandı' : live ? 'Canlı veri alınıyor' : states[snapshot.connection_state] || 'Bağlantı bilgisi bekleniyor';
  $('session-status').textContent = !session ? 'Henüz oturum yok' : session.finished_at ? 'Son oturum' : 'Aktif oturum';
  if (session) {
    const end = session.finished_at ? Date.parse(session.finished_at) : Date.now();
    const seconds = Math.max(0, Math.floor((end - Date.parse(session.started_at)) / 1000));
    $('duration').textContent = `${Math.floor(seconds / 60).toString().padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`;
  } else $('duration').textContent = '—:—';
}
function recordPowerSample() {
  history.push({ time: performance.now(), value: currentMetric('power_w') });
}
function drawChart() {
  const now = performance.now(); history = history.filter((p) => now - p.time <= 60000);
  const valid = history.filter((p) => p.value != null);
  const power = currentMetric('power_w');
  $('chart-status').textContent = power === 0 ? '0 W ölçülüyor · çizgi sıfır seviyesinde devam eder.'
    : power != null ? 'Canlı güç örnekleri · kesintiler birleştirilmez.'
    : snapshot?.session?.finished_at ? 'Oturum tamamlandı · son gerçek ölçümde biter.'
    : valid.length ? 'Güncel güç ölçümü yok · boşluk sıfır watt anlamına gelmez.'
    : 'Güç ölçümü bekleniyor · yalnızca alınan değerler çizilir.';
  $('chart-empty').hidden = valid.length >= 2;
  const svg = $('power-chart'); svg.replaceChildren();
  if (valid.length < 2) return;
  const max = Math.max(50, ...valid.map((p) => p.value)), min = Math.min(0, ...valid.map((p) => p.value));
  $('chart-scale').textContent = `${Math.floor(min)}–${Math.ceil(max)} W`;
  // Color each segment by its ending sample; nulls break the line.
  // Recolor visible history when the display FTP changes; raw watts stay intact.
  let points = [], color = null;
  const flush = () => {
    if (points.length > 1) {
      const path = document.createElementNS('http://www.w3.org/2000/svg', 'polyline');
      path.setAttribute('points', points.join(' ')); path.setAttribute('fill', 'none');
      path.setAttribute('stroke', color); path.setAttribute('stroke-width', '2');
      path.setAttribute('vector-effect', 'non-scaling-stroke'); svg.append(path);
    }
    points = [];
  };
  let previous = null;
  for (const point of history) {
    if (point.value == null) { flush(); previous = null; color = null; continue; }
    const position = `${900 * (1 - (now - point.time) / 60000)},${142 - (point.value - min) / (max - min) * 134}`;
    const nextColor = zoneColors[powerZone(point.value)] || '#b3bcb2';
    if (nextColor !== color) { flush(); if (previous) points.push(previous); color = nextColor; }
    points.push(position); previous = position;
  }
  flush();
}
async function poll(run) {
  if (run !== generation || !token) return;
  const requestController = new AbortController(); controller = requestController;
  const timeout = setTimeout(() => requestController.abort(), 3000);
  try {
    const response = await fetch('/api/v1/live', { headers: { Authorization: `Bearer ${token}` }, cache: 'no-store', credentials: 'omit', signal: requestController.signal });
    if (run !== generation) return;
    if (response.status === 401) { disconnect('Erişim anahtarı kabul edilmedi.'); return; }
    if (!response.ok) throw new Error('API unavailable');
    const data = await response.json();
    if (run !== generation) return;
    snapshot = data; sampledAt = performance.now(); online = true;
    if (sessionId !== data.session?.id) { sessionId = data.session?.id; history = []; }
    // Capture each reply immediately, including zero just before finish/outage.
    recordPowerSample(); drawChart();
    badge('Yerel API bağlı', 'good');
    $('access').hidden = true; $('disconnect').hidden = false;
    $('update-status').textContent = 'Her saniye güncellenir · 5 saniyeyi aşan ölçümler gizlenir.';
  } catch (error) {
    if (run !== generation) return;
    online = false; recordPowerSample(); drawChart(); badge('API’ye ulaşılamıyor', 'error');
    $('update-status').textContent = 'Yerel API bağlantısı kesildi. Otomatik olarak yeniden deneniyor.';
  } finally {
    clearTimeout(timeout);
    if (run === generation && token) { render(); timer = setTimeout(() => poll(run), 1000); }
  }
}
function disconnect(message = 'Başlamak için ekranı yerel API’ye bağla.') {
  generation++; token = ''; controller?.abort(); clearTimeout(timer); snapshot = null; online = false; sessionId = null; history = [];
  $('access').hidden = false; $('disconnect').hidden = true; $('token').value = ''; $('update-status').textContent = message;
  badge('Ekran bağlı değil'); render(); drawChart();
}
$('connect-form').addEventListener('submit', (event) => {
  event.preventDefault(); const candidate = $('token').value.trim();
  if (!/^[A-Za-z0-9_-]{32,64}$/.test(candidate)) { $('update-status').textContent = '32–64 karakterlik geçerli yerel API anahtarını gir.'; return; }
  generation++; controller?.abort(); clearTimeout(timer); token = candidate; $('token').value = ''; badge('Bağlanıyor'); poll(generation);
});
$('disconnect').addEventListener('click', () => disconnect());
$('ftp-form').addEventListener('submit', (event) => {
  event.preventDefault(); const candidate = Number($('ftp').value);
  if (!Number.isInteger(candidate) || candidate < 1 || candidate > 2000) {
    $('ftp-status').textContent = 'FTP için 1–2000 arasında tam sayı watt gir.'; return;
  }
  ftp = candidate; customFtp = true; render(); drawChart();
});
$('ftp-reset').addEventListener('click', () => {
  ftp = 200; customFtp = false; $('ftp').value = '200'; render(); drawChart();
});
setInterval(() => {
  render();
  // A timer marks expiry as a gap; it must not invent repeated measurements.
  if (currentMetric('power_w') == null && history.at(-1)?.value != null) recordPowerSample();
  drawChart();
}, 1000);
window.addEventListener('pagehide', () => { generation++; token = ''; controller?.abort(); clearTimeout(timer); });
render();
