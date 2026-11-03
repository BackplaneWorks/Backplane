// IO boundary for disposable invitations and durable, individually revocable
// sessions. One hub owns the home lock. All operations are synchronous so an
// invitation cannot be redeemed twice across overlapping HTTP requests.
// Every invitation the hub issues is recorded (hashes only) beside the
// devices: when, whether and by which device it was used, and whether it was
// revoked. Only a recorded, unexpired, unused, unrevoked invitation enrolls;
// revoking a used one revokes the device it paired.
function pairingStore({home, root, now = () => Date.now(), random = n => require('node:crypto').randomBytes(n), fs = require('node:fs')}) {
  const crypto = require('node:crypto'), path = require('node:path');
  const digest = x => crypto.createHash('sha256').update(x).digest('hex');
  const same = (a, b) => typeof a === 'string' && typeof b === 'string' && a.length === b.length && crypto.timingSafeEqual(Buffer.from(a), Buffer.from(b));
  const file = path.join(home, 'devices-v1.json');
  const hex = (x, n) => typeof x === 'string' && new RegExp('^[a-f0-9]{' + n + '}$').test(x);
  const validInvite = x => x && hex(x.id, 32) && hex(x.hash, 64) && hex(x.codeHash, 64) && Number.isFinite(x.createdAt) && Number.isFinite(x.expiresAt) && typeof x.used === 'boolean' && typeof x.revoked === 'boolean' && (x.device === undefined || hex(x.device, 32)) && (x.usedAt === undefined || Number.isFinite(x.usedAt));
  let devices = [], invites = [], attempts = [], clients = new Map(), retiredClients = new Set();
  if (fs.existsSync(file)) {
    const data = JSON.parse(fs.readFileSync(file, 'utf8'));
    if (data.version !== 1 || data.owner !== digest(root) || !Array.isArray(data.devices) || data.devices.some(d => !/^[a-f0-9]{32}$/.test(d.id) || !/^[a-f0-9]{64}$/.test(d.hash) || typeof d.name !== 'string' || !Number.isFinite(d.lastSeen))) throw new Error('Invalid device registry');
    // (a registry from before invitations were recorded has none)
    if (data.invitations !== undefined && (!Array.isArray(data.invitations) || !data.invitations.every(validInvite))) throw new Error('Invalid device registry');
    devices = data.devices;invites = data.invitations || [];
    if (process.platform !== 'win32') fs.chmodSync(file, 0o600);
  }
  // the newest invitations stay listed (older ones are dropped once used,
  // expired or revoked; an open one is never dropped)
  const keep = 100;
  function trim(xs) {
    const at = now(), open = x => !x.used && !x.revoked && x.expiresAt > at;
    const old = xs.filter(x => !open(x)).sort((a, b) => b.createdAt - a.createdAt).slice(0, keep);
    return xs.filter(x => open(x) || old.includes(x));
  }
  function persist(nextDevices, nextInvites = invites) {
    const tmp = file + '.' + random(8).toString('hex') + '.tmp';
    let fd;
    try {
      fd = fs.openSync(tmp, 'wx', 0o600);
      fs.writeFileSync(fd, JSON.stringify({version: 1, owner: digest(root), devices: nextDevices, invitations: nextInvites}));
      fs.fsyncSync(fd);fs.closeSync(fd);fd = undefined;
      fs.renameSync(tmp, file);
      if (process.platform !== 'win32') { const dir = fs.openSync(home, 'r');try { fs.fsyncSync(dir); } finally { fs.closeSync(dir); } }
    } finally { if (fd !== undefined) fs.closeSync(fd);if (fs.existsSync(tmp)) fs.unlinkSync(tmp); }
    devices = nextDevices;invites = nextInvites;
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
    const at = now();
    if (invites.filter(x => !x.used && !x.revoked && x.expiresAt > at).length >= 16) return {error: 'too_many_invitations'};
    const token = random(16).toString('hex'), alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
    const code = Array.from(random(8), x => alphabet[x & 31]).join('');
    const unique = [...new Set(routes)].filter(x => {
      try { const u = new URL(x);return ['http:', 'https:'].includes(u.protocol) && !u.username && !u.password && !u.hash && !u.search; } catch { return false; }
    });
    if (!unique.length) return {error: 'no_reachable_address'};
    const entry = {id: random(16).toString('hex'), hash: digest(token), codeHash: digest(code), createdAt: at, expiresAt: at + 600000, used: false, revoked: false};
    // recorded before it is handed out: a link the hub cannot keep a
    // record of is never issued
    persist(devices, trim([...invites, entry]));
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
    if (invite.revoked) return {error: 'revoked_code'};
    if (invite.used) return {error: 'used_code'};
    if (invite.expiresAt <= at) return {error: 'expired_code'};
    if (devices.filter(d => !d.revoked).length >= 256) return {error: 'device_limit'};
    const token = random(16).toString('hex');
    const entry = {id: random(16).toString('hex'), hash: digest(token), name: String(name || 'New device').replace(/[\u0000-\u001f\u007f]/g, '').slice(0, 80), createdAt: at, lastSeen: at, revoked: false};
    // the device and its invitation's use are written together
    persist([...devices, entry], invites.map(x => x === invite ? {...x, used: true, usedAt: at, device: entry.id} : x));
    return {token, id: entry.id};
  }
  function state(x, at) { return x.revoked ? 'revoked' : x.used ? 'used' : x.expiresAt <= at ? 'expired' : 'open'; }
  function list() {
    const at = now(), names = new Map(devices.map(d => [d.id, d]));
    return {devices: devices.map(({hash, ...d}) => d),
      invitations: [...invites].sort((a, b) => b.createdAt - a.createdAt).map(x => ({id: x.id, kind: 'device', createdAt: x.createdAt, expiresAt: x.expiresAt,
        state: state(x, at), ...(x.usedAt ? {usedAt: x.usedAt} : {}), ...(x.device ? {device: x.device, deviceName: names.get(x.device)?.name || '', deviceRevoked: !!names.get(x.device)?.revoked} : {})}))};
  }
  function retire(ids) {
    const disconnected = [...clients].filter(([, d]) => ids.includes(d)).map(([c]) => c);
    disconnected.forEach(c => retiredClients.add(c));
    return disconnected;
  }
  function revoke(id) {
    if (!devices.some(d => d.id === id)) return {error: 'unknown_device'};
    persist(devices.map(d => d.id === id ? {...d, revoked: true} : d));
    return {ok: true, clients: retire([id])};
  }
  // an invitation revoked: unused, it never enrolls; used, the device it
  // paired loses access too (and its open connections close)
  function uninvite(id) {
    const invite = invites.find(x => x.id === id);if (!invite) return {error: 'unknown_invitation'};
    const paired = invite.device && devices.some(d => d.id === invite.device) ? [invite.device] : [];
    persist(devices.map(d => paired.includes(d.id) ? {...d, revoked: true} : d), invites.map(x => x === invite ? {...x, revoked: true, revokedAt: now()} : x));
    return {ok: true, clients: retire(paired)};
  }
  return {resolve, create, enroll, list, revoke, uninvite, active: id => !retiredClients.has(id), forget: id => clients.delete(id)};
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
    if (op === 'uninvite') return JSON.stringify(store.uninvite(secret));
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
