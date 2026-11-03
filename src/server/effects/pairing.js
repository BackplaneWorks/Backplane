// IO boundary for disposable invitations and durable, individually revocable
// sessions. One hub owns the home lock. All operations are synchronous so an
// invitation cannot be redeemed twice across overlapping HTTP requests.
function pairingStore({home, root, now = () => Date.now(), random = n => require('node:crypto').randomBytes(n), fs = require('node:fs')}) {
  const crypto = require('node:crypto'), path = require('node:path');
  const digest = x => crypto.createHash('sha256').update(x).digest('hex');
  const same = (a, b) => typeof a === 'string' && typeof b === 'string' && a.length === b.length && crypto.timingSafeEqual(Buffer.from(a), Buffer.from(b));
  const file = path.join(home, 'devices-v1.json');
  let devices = [], invites = [], attempts = [], clients = new Map(), retiredClients = new Set();
  if (fs.existsSync(file)) {
    const data = JSON.parse(fs.readFileSync(file, 'utf8'));
    if (data.version !== 1 || data.owner !== digest(root) || !Array.isArray(data.devices) || data.devices.some(d => !/^[a-f0-9]{32}$/.test(d.id) || !/^[a-f0-9]{64}$/.test(d.hash) || typeof d.name !== 'string' || !Number.isFinite(d.lastSeen))) throw new Error('Invalid device registry');
    devices = data.devices;
    if (process.platform !== 'win32') fs.chmodSync(file, 0o600);
  }
  function persist(next) {
    const tmp = file + '.' + random(8).toString('hex') + '.tmp';
    let fd;
    try {
      fd = fs.openSync(tmp, 'wx', 0o600);
      fs.writeFileSync(fd, JSON.stringify({version: 1, owner: digest(root), devices: next}));
      fs.fsyncSync(fd);fs.closeSync(fd);fd = undefined;
      fs.renameSync(tmp, file);
      if (process.platform !== 'win32') { const dir = fs.openSync(home, 'r');try { fs.fsyncSync(dir); } finally { fs.closeSync(dir); } }
    } finally { if (fd !== undefined) fs.closeSync(fd);if (fs.existsSync(tmp)) fs.unlinkSync(tmp); }
    devices = next;
  }
  function device(token) { return devices.find(d => !d.revoked && same(d.hash, digest(token))); }
  function resolve(token, client, route) {
    if (!token || same(root, token)) return root;
    if (!/^[a-f0-9]{32}$/.test(token)) return '';
    const d = device(token);if (!d) return '';
    if (now() - d.lastSeen >= 60000) persist(devices.map(x => x.id === d.id ? {...x, lastSeen: now()} : x));
    if (route === '/ws') clients.set(client, d.id);
    return token;
  }
  function create(routes) {
    const at = now();invites = invites.filter(x => x.expiresAt > at);
    if (invites.length >= 16) return {error: 'too_many_invitations'};
    const token = random(16).toString('hex'), alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
    const code = Array.from(random(8), x => alphabet[x & 31]).join('');
    const unique = [...new Set(routes)].filter(x => {
      try { const u = new URL(x);return ['http:', 'https:'].includes(u.protocol) && !u.username && !u.password && !u.hash && !u.search; } catch { return false; }
    });
    if (!unique.length) return {error: 'no_reachable_address'};
    const entry = {id: random(16).toString('hex'), hash: digest(token), codeHash: digest(code), expiresAt: at + 600000, used: false};
    invites.push(entry);
    const fragment = new URLSearchParams({invite: token, routes: JSON.stringify(unique)});
    return {id: entry.id, code, expiresAt: entry.expiresAt, routes: unique, localOnly: unique.every(x => /^https?:\/\/(localhost|127\.|\[::1\])/.test(x)), link: unique[0] + '/pair#' + fragment};
  }
  function enroll(secret, name) {
    const at = now();attempts = attempts.filter(x => x > at - 60000);
    // A hub-wide bucket bounds guessing even if an attacker rotates addresses.
    if (attempts.length >= 10) return {error: 'rate_limited'};
    attempts.push(at);
    const compact = secret.replace(/[ -]/g, '').toUpperCase();
    const invite = invites.find(x => same(x.hash, digest(secret)) || same(x.codeHash, digest(compact)));
    if (!invite) return {error: 'wrong_code'};
    if (invite.expiresAt <= at) return {error: 'expired_code'};
    if (invite.used) return {error: 'used_code'};
    if (devices.filter(d => !d.revoked).length >= 256) return {error: 'device_limit'};
    const token = random(16).toString('hex');
    const entry = {id: random(16).toString('hex'), hash: digest(token), name: String(name || 'New device').replace(/[\u0000-\u001f\u007f]/g, '').slice(0, 80), createdAt: at, lastSeen: at, revoked: false};
    persist([...devices, entry]);
    invite.used = true;
    return {token, id: entry.id};
  }
  function list() { return {devices: devices.map(({hash, ...d}) => d)}; }
  function revoke(id) {
    if (!devices.some(d => d.id === id)) return {error: 'unknown_device'};
    persist(devices.map(d => d.id === id ? {...d, revoked: true} : d));
    const disconnected = [...clients].filter(([, d]) => d === id).map(([c]) => c);
    disconnected.forEach(c => retiredClients.add(c));
    return {ok: true, clients: disconnected};
  }
  return {resolve, create, enroll, list, revoke, active: id => !retiredClients.has(id), forget: id => clients.delete(id)};
}

// A listener advertises its own addresses only after bind/Serve succeeded.
// Strip all credential material before keeping the route hint.
function pairingAddress(link) {
  try { const u = new URL(link);return ['https:', 'http:'].includes(u.protocol) && !u.username && !u.password ? u.origin : ''; } catch { return ''; }
}
let bp_pairing;
function pairing_start(home, root, host, port) {
  try { bp_pairing = {store: pairingStore({home, root}), host, port, routes: []};return ''; }
  catch { return 'Cannot read or initialize device registry; existing pairings were preserved'; }
}
function pairing_address(link) {
  const origin = pairingAddress(link);
  if (origin && bp_pairing && !bp_pairing.routes.includes(origin)) bp_pairing.routes.unshift(origin);
  return {$: 'Unit'};
}
function pairing_resolve(token, client, route) {
  try { return bp_pairing.store.resolve(token, Number(client), route); } catch { return ''; }
}
function pairing_active(client) { return {$: bp_pairing?.store.active(Number(client)) ? 'True' : 'False'}; }
function pairing_forget(client) { bp_pairing?.store.forget(Number(client));return {$: 'Unit'}; }
function pairing_request(op, secret, name, authority, https) {
  try {
    const {store, host, port} = bp_pairing;
    if (op === 'enroll') return JSON.stringify(store.enroll(secret, name));
    if (op === 'list') return JSON.stringify(store.list());
    if (op === 'revoke') return JSON.stringify(store.revoke(secret));
    if (op === 'create') {
      const routes = [...bp_pairing.routes], scheme = https ? 'https://' : 'http://';
      // This exact origin already authenticated the issuing client. Other
      // hints are only local interfaces on which this listener is bound.
      if (/^[a-z0-9.\-\[\]:]+$/i.test(authority)) routes.push(scheme + authority);
      if (host === '0.0.0.0') for (const values of Object.values(require('node:os').networkInterfaces())) for (const v of values || []) {
        if (v.family === 'IPv4' && !v.internal) routes.push('http://' + v.address + ':' + port);
      }
      else if (host !== '127.0.0.1' && host !== 'localhost') routes.push('http://' + host + ':' + port);
      const remote = routes.filter(x => !/^https?:\/\/(localhost|127\.|\[::1\])/.test(x));
      return JSON.stringify(store.create(remote.length ? remote : routes));
    }
    return JSON.stringify({error: 'invalid_request'});
  } catch { return JSON.stringify({error: 'storage_unavailable'}); }
}
io_eff(CID(Pairing.start), pairing_start);
io_eff(CID(Pairing.resolve), pairing_resolve);
io_eff(CID(Pairing.request), pairing_request);
io_eff(CID(Pairing.forget), pairing_forget);

io_eff(CID(Pairing.active), pairing_active);

io_eff(CID(Pairing.address), pairing_address);
