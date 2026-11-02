import record from '../../desktop/connection-record.cjs';
const { PREFIX, LIMIT, MAX_BYTES, sanitize, retained } = record;
const KEY = 'bp.connection.log.v1', CONTROL = KEY + '.enabled';
const bytes = value => new TextEncoder().encode(value).length;
// Local metadata ring and an allowlisted desktop console record. No network IO.
export class ConnectionLog {
  constructor({ storage, bundle = '', now = Date.now, visibility = () => 'unknown', online = () => true,
    emit = x => console.info(PREFIX + JSON.stringify(x)) } = {}) {
    this.storage = storage; this.bundle = bundle; this.now = now; this.visibility = visibility; this.online = online; this.emit = emit;
    this.page = Array.from(globalThis.crypto?.getRandomValues(new Uint8Array(8)) || new Uint8Array(8), x => x.toString(16).padStart(2, '0')).join('');
    this.rows = []; this.next = 0;this.bucket = -1;this.writes = 0;this.lastSample = new Map();this.on = true;
    try { this.on = storage?.getItem(CONTROL) !== 'off'; } catch {}
    try { const text = storage?.getItem(KEY); if (text && bytes(text) <= MAX_BYTES) this.rows = retained(JSON.parse(text), now()); } catch {}
  }
  enabled() { return this.on; }
  setEnabled(on) {
    if (!on) this.on = false;
    this.storage?.setItem(CONTROL, on ? 'on' : 'off');
    this.on = on === true;
  }
  clear() { this.rows = [];this.storage?.removeItem(KEY); }
  snapshot() { this.rows = retained(this.rows, this.now());return this.rows; }
  measure(metric, durationMs) {
    if (!['frame.decode','connection.ready','view.reply'].includes(metric) || !Number.isFinite(durationMs) || durationMs < 0) return;
    const last = this.lastSample.get(metric) ?? -Infinity;
    if (this.now() - last < 10000) return;
    this.lastSample.set(metric, this.now());this.record('latency', { metric, durationMs: Math.min(86400000, Math.round(durationMs)) });
  }
  record(event, detail = {}) {
    if (!this.on) return;
    const second = Math.floor(this.now() / 1000);
    if (second !== this.bucket) { this.bucket = second;this.writes = 0; }
    if (++this.writes > 12) return;
    const r = sanitize({ ...detail, event, at: this.now(), page: this.page, bundle: this.bundle, visibility: this.visibility(), online: this.online() });
    if (!r) return;
    this.rows = retained(this.rows, this.now()).concat(r).slice(-LIMIT);
    let text = JSON.stringify(this.rows);
    while (bytes(text) > MAX_BYTES && this.rows.length) { this.rows.shift();text = JSON.stringify(this.rows); }
    try { this.storage?.setItem(KEY, text); } catch {}
    try { this.emit(r); } catch {}
  }
  connection() {
    const connection = ++this.next, began = this.now();
    let frameAt = began, heartbeatAt, lastFrameBytes = 0, cause, generation, epoch;
    const note = (event, extra = {}) => this.record(event, { connection, cause: cause || 'transport', generation, epoch,
      ageMs: Math.max(0, this.now() - began), frameAgeMs: Math.max(0, this.now() - frameAt),
      ...(heartbeatAt === undefined ? {} : { heartbeatAgeMs: Math.max(0, this.now() - heartbeatAt) }), lastFrameBytes, ...extra });
    return {
      note: (event, extra) => { note(event, extra);if (event === 'ready') this.measure('connection.ready', this.now() - began); },
      frame: bytes => { frameAt = this.now(); lastFrameBytes = bytes; },
      heartbeat: () => { heartbeatAt = this.now(); },
      greeting: x => { generation = x.generation; epoch = x.epoch; note('greeting'); },
      initiate: why => { if (!cause) { cause = why; note('closing'); } },
      closed: e => note('closed', { closeCode: e?.code, wasClean: e?.wasClean, closeReason: e?.reason || '', reasonBytes: Math.min(123, (e?.reason || '').length) }),
    };
  }
}
