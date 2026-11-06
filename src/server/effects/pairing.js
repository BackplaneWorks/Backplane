// IO boundary for disposable invitations and durable, individually revocable
// sessions. One hub owns the home lock. All operations are synchronous so an
// invitation cannot be redeemed twice across overlapping HTTP requests.
// Every invitation the hub issues is recorded (hashes only) beside the
// devices: when, whether and by which device it was used, and whether it was
// revoked. Only a recorded, unexpired, unused, unrevoked invitation enrolls;
// revoking a used one revokes the device it paired.
// Hub-to-hub invites (bp1:, Bots > Link a machine, and the ones hubs answer
// each other with when they pair by themselves) are recorded here too, as
// kind "machine": the link they are for (its peer id; the secret stays in
// <home>/secrets/peers/<peer>, botnet.bend's Bn.peer.path), when it expires
// (as a device invitation does) and which machine used it. A machine links
// only with a recorded, unexpired, unused, unrevoked one; revoking a used
// one unlinks the machine. (Their hash fields are random: they enroll
// nothing, and an older hub still reads the registry.)
function pairingStore({home, root, now = () => Date.now(), random = n => require('node:crypto').randomBytes(n), fs = require('node:fs')}) {
  const crypto = require('node:crypto'), path = require('node:path');
  const digest = x => crypto.createHash('sha256').update(x).digest('hex');
  const same = (a, b) => typeof a === 'string' && typeof b === 'string' && a.length === b.length && crypto.timingSafeEqual(Buffer.from(a), Buffer.from(b));
  const file = path.join(home, 'devices-v1.json');
  const hex = (x, n) => typeof x === 'string' && new RegExp('^[a-f0-9]{' + n + '}$').test(x);
  const validMachine = x => x.kind === 'machine' && /^p[a-f0-9]{16}$/.test(x.peer) && (x.name === undefined || typeof x.name === 'string') && (x.unlinked === undefined || typeof x.unlinked === 'boolean');
  const validInvite = x => x && (x.kind === undefined || validMachine(x)) && hex(x.id, 32) && hex(x.hash, 64) && hex(x.codeHash, 64) && Number.isFinite(x.createdAt) && Number.isFinite(x.expiresAt) && typeof x.used === 'boolean' && typeof x.revoked === 'boolean' && (x.device === undefined || hex(x.device, 32)) && (x.usedAt === undefined || Number.isFinite(x.usedAt));
  let devices = [], invites = [], attempts = [], clients = new Map(), retiredClients = new Set(), swept = new Set();
  const isMachine = x => x.kind === 'machine';
  const open = (x, at) => !x.used && !x.revoked && x.expiresAt > at;
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
    const at = now();
    const old = xs.filter(x => !open(x, at)).sort((a, b) => b.createdAt - a.createdAt).slice(0, keep);
    return xs.filter(x => open(x, at) || old.includes(x));
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
  // routes: the default address's (the link, code and routes older
  // clients read); choices: every address the invitation may be shown
  // with ({kind, url, listening, routes}, pairingPlan), each its own link
  // to the same invitation, that address first in its routes
  function create(routes, choices = []) {
    const at = now();
    if (invites.filter(x => !isMachine(x) && open(x, at)).length >= 16) return {error: 'too_many_invitations'};
    const token = random(16).toString('hex'), alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
    const code = Array.from(random(8), x => alphabet[x & 31]).join('');
    const valid = x => {
      try { const u = new URL(x);return ['http:', 'https:'].includes(u.protocol) && !u.username && !u.password && !u.hash && !u.search; } catch { return false; }
    };
    const unique = [...new Set(routes)].filter(valid);
    if (!unique.length) return {error: 'no_reachable_address'};
    const entry = {id: random(16).toString('hex'), hash: digest(token), codeHash: digest(code), createdAt: at, expiresAt: at + 600000, used: false, revoked: false};
    // recorded before it is handed out: a link the hub cannot keep a
    // record of is never issued
    persist(devices, trim([...invites, entry]));
    const link = rs => rs[0] + '/pair#' + new URLSearchParams({invite: token, routes: JSON.stringify(rs)});
    const addresses = choices.filter(c => valid(c.url)).map(c => ({kind: c.kind, url: c.url, link: link([...new Set([c.url, ...(c.routes || [])])].filter(valid)), listening: !!c.listening}));
    return {id: entry.id, code, expiresAt: entry.expiresAt, routes: unique, localOnly: unique.every(x => /^https?:\/\/(localhost|127\.|\[::1\])/.test(x)), link: link(unique), addresses};
  }
  function enroll(secret, name) {
    const at = now();attempts = attempts.filter(x => x > at - 60000);
    // A hub-wide bucket bounds guessing even if an attacker rotates addresses.
    if (attempts.length >= 10) return {error: 'rate_limited'};
    attempts.push(at);
    const compact = secret.replace(/[ -]/g, '').toUpperCase();
    const invite = invites.find(x => !isMachine(x) && (same(x.hash, digest(secret)) || same(x.codeHash, digest(compact))));
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
  // the secret of a machine invite nobody can use any more (expired or
  // revoked before it was used) is removed from the home
  const secretFile = peer => path.join(home, 'secrets', 'peers', peer);
  function sweep() {
    const at = now();
    for (const x of invites) if (isMachine(x) && !x.used && (x.revoked || x.expiresAt <= at) && !swept.has(x.peer)) {
      try { fs.rmSync(secretFile(x.peer), {force: true});swept.add(x.peer); } catch {}
    }
  }
  // a hub-to-hub invite for the link `peer`, recorded before it is handed
  // out (server.bend's BotJob.invite and Pair.invite)
  function machine(peer) {
    const at = now();
    if (!/^p[a-f0-9]{16}$/.test(peer)) return {error: 'invalid_request'};
    if (invites.some(x => isMachine(x) && x.peer === peer)) return {error: 'invalid_request'};
    if (invites.filter(x => isMachine(x) && open(x, at)).length >= 16) return {error: 'too_many_invitations'};
    const entry = {id: random(16).toString('hex'), kind: 'machine', peer, hash: digest(random(32)), codeHash: digest(random(32)), createdAt: at, expiresAt: at + 600000, used: false, revoked: false};
    persist(devices, trim([...invites, entry]));
    sweep();
    return {ok: true, id: entry.id, expiresAt: entry.expiresAt};
  }
  // a machine links with the invite for `peer` (server.bend's Hub.bot.link):
  // once, before it expires, unless revoked
  function link(peer, name) {
    const at = now(), invite = invites.find(x => isMachine(x) && x.peer === peer);
    if (!invite) return {error: 'unknown_invitation'};
    if (invite.revoked) return {error: 'revoked_code'};
    if (invite.used) return {error: 'used_code'};
    if (invite.expiresAt <= at) return {error: 'expired_code'};
    persist(devices, invites.map(x => x === invite ? {...x, used: true, usedAt: at, name: String(name || '').replace(/[\u0000-\u001f\u007f]/g, '').slice(0, 80)} : x));
    return {ok: true};
  }
  // the link a machine invite made was removed (bots.unpeer)
  function unlinked(peer) {
    if (!invites.some(x => isMachine(x) && x.peer === peer && x.used && !x.unlinked)) return {ok: true};
    persist(devices, invites.map(x => isMachine(x) && x.peer === peer ? {...x, unlinked: true} : x));
    return {ok: true};
  }
  function state(x, at) { return x.revoked ? 'revoked' : x.used ? 'used' : x.expiresAt <= at ? 'expired' : 'open'; }
  function listed(x, at, names) {
    const base = {id: x.id, kind: isMachine(x) ? 'machine' : 'device', createdAt: x.createdAt, expiresAt: x.expiresAt, state: state(x, at), ...(x.usedAt ? {usedAt: x.usedAt} : {})};
    if (isMachine(x)) return {...base, ...(x.used ? {device: x.peer, deviceName: x.name || '', deviceRevoked: !!x.unlinked} : {})};
    return {...base, ...(x.device ? {device: x.device, deviceName: names.get(x.device)?.name || '', deviceRevoked: !!names.get(x.device)?.revoked} : {})};
  }
  function list() {
    sweep();
    const at = now(), names = new Map(devices.map(d => [d.id, d]));
    return {devices: devices.map(({hash, ...d}) => d),
      invitations: [...invites].sort((a, b) => b.createdAt - a.createdAt).map(x => listed(x, at, names))};
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
    // (a machine invite: its secret goes, and a link it made is removed by
    // the hub, server.bend's Conn.devices.unpeer)
    if (isMachine(invite)) {
      const linked = invite.used && !invite.unlinked;
      persist(devices, invites.map(x => x === invite ? {...x, revoked: true, revokedAt: now(), ...(linked ? {unlinked: true} : {})} : x));
      try { fs.rmSync(secretFile(invite.peer), {force: true}); } catch {}
      return {ok: true, clients: [], ...(linked ? {peer: invite.peer} : {})};
    }
    const paired = invite.device && devices.some(d => d.id === invite.device) ? [invite.device] : [];
    persist(devices.map(d => paired.includes(d.id) ? {...d, revoked: true} : d), invites.map(x => x === invite ? {...x, revoked: true, revokedAt: now()} : x));
    return {ok: true, clients: retire(paired)};
  }
  return {resolve, create, enroll, list, revoke, uninvite, machine, link, unlinked, active: id => !retiredClients.has(id), forget: id => clients.delete(id)};
}

// A listener advertises its own addresses only after bind/Serve succeeded.
// Strip all credential material before keeping the route hint.
function pairingAddress(link) {
  try { const u = new URL(link);return ['https:', 'http:'].includes(u.protocol) && !u.username && !u.password ? u.origin : ''; } catch { return ''; }
}
// Whether an origin is one of the owner's hubs, as this hub knows them: its
// own listener-confirmed routes, the owner's machines it found (Tailscale's
// same-owner peers and BACKPLANE_PEERS, by address, or by MagicDNS name in
// the owner's tailnet domain) and the linked hubs it verified as the
// owner's (server.bend's Far.own). A desktop app asks before following a
// pairing link (desktop/main.cjs): a link to any other origin is not paired
// without the person saying so.
function pairingKnown(origin, {routes = [], machines = {}, peers = []} = {}) {
  let u;try { u = new URL(origin); } catch { return false; }
  if (!['http:', 'https:'].includes(u.protocol) || u.username || u.password) return false;
  const at = x => { try { const v = new URL(x);return v.protocol + '//' + v.host; } catch { return ''; } };
  const self = machines.self || {}, others = Array.isArray(machines.peers) ? machines.peers : [];
  const urls = [...routes, self.url, ...others.map(m => m && m.url), ...peers].filter(x => typeof x === 'string' && x).map(at).filter(Boolean);
  if (urls.includes(u.protocol + '//' + u.host)) return true;
  const domain = typeof self.domain === 'string' ? self.domain.toLowerCase().replace(/\.$/, '') : '';
  const host = u.hostname.toLowerCase(), names = [self.name, ...others.map(m => m && m.name)].filter(x => typeof x === 'string' && x).map(x => x.toLowerCase());
  return !!domain && host.endsWith('.' + domain) && names.includes(host.slice(0, host.indexOf('.')));
}
// 100.64.0.0/10, the addresses Tailscale hands out
const pairingTailnetIp = ip => { const m = /^100\.(\d+)\.\d+\.\d+$/.exec(ip);return !!m && +m[1] >= 64 && +m[1] <= 127; };
// This machine's local network addresses: IPv4, not loopback or
// link-local, not the tailnet's, and not a container's or a VM's bridge
// (no other device on the network can reach those).
const pairingVirtual = /^(docker|br-|veth|virbr|vboxnet|vmnet|tailscale|podman|cni|flannel|lxcbr|lxdbr|zt)/;
function pairingLan(interfaces) {
  const out = [];
  for (const [name, values] of Object.entries(interfaces || {})) {
    if (pairingVirtual.test(name)) continue;
    for (const v of values || []) if ((v.family === 'IPv4' || v.family === 4) && !v.internal && !pairingTailnetIp(v.address) && !/^169\.254\./.test(v.address) && !out.includes(v.address)) out.push(v.address);
  }
  return out;
}
// Where an invitation may point (Settings > Devices > Add a device): the
// tailnet, each local network address, this computer, in that order, each
// saying whether this hub answers there now. The tailnet's are only
// listener-confirmed origins (routes: the tailnet listener's, Serve's), the
// tailnet address a listener on every interface covers, or the one this
// request came through. A local network address answers when the main
// listener covers it (--host 0.0.0.0 or that address) or the person let the
// hub answer there too (bound: Lan.* in server.bend). The default (the
// fields older clients read) is the first remote address that answers.
// Each address's routes: itself, then the other remote ones that answer;
// loopback is never a hint for another device (it would reach that
// device's own hub).
function pairingPlan({routes = [], host, port, authority = '', https = false, interfaces = {}, bound = []}) {
  const local = x => /^https?:\/\/(localhost|127\.|\[::1\])/.test(x);
  const name = x => { try { return new URL(x).hostname; } catch { return ''; } };
  const tailnet = x => pairingTailnetIp(name(x)) || /\.ts\.net$/i.test(name(x));
  const all = host === '0.0.0.0', at = ip => 'http://' + ip + ':' + port;
  const loopHost = all || ['127.0.0.1', 'localhost', '::1'].includes(host);
  // This exact origin already authenticated the issuing client.
  const asked = /^[a-z0-9.\-\[\]:]+$/i.test(authority) ? (https ? 'https://' : 'http://') + authority : '';
  const lan = pairingLan(interfaces);
  const v4 = Object.values(interfaces || {}).flat().filter(v => v && (v.family === 'IPv4' || v.family === 4)).map(v => v.address);
  const tails = [...new Set([...routes, ...(all ? v4.filter(pairingTailnetIp).map(at) : []), ...(pairingTailnetIp(host) ? [at(host)] : []), ...(asked && tailnet(asked) ? [asked] : [])])];
  const choices = [
    ...(tails.length ? [{kind: 'tailnet', url: tails[0], listening: true}] : []),
    ...lan.map(ip => name(asked) === ip ? {kind: 'lan', url: asked, listening: true} : {kind: 'lan', url: at(ip), listening: all || host === ip || bound.includes(ip)}),
    {kind: 'local', url: local(asked) ? asked : at('127.0.0.1'), listening: loopHost || local(asked)}];
  // other origins this hub answers on, hints after the chosen one: more
  // tailnet ones, the request's own and the main listener's when they are
  // none of the above
  const hints = [...tails.slice(1), ...(asked && !local(asked) && !tailnet(asked) && !lan.includes(name(asked)) ? [asked] : []),
    ...(!loopHost && !pairingTailnetIp(host) && !lan.includes(host) ? [at(host)] : [])];
  const remote = choices.filter(c => c.kind !== 'local' && c.listening).map(c => c.url);
  for (const c of choices) c.routes = [...new Set([c.url, ...remote, ...hints])];
  const first = choices.find(c => c.kind !== 'local' && c.listening);
  return {routes: first ? first.routes : hints.length ? [...new Set(hints)] : choices[choices.length - 1].routes, choices};
}
// The local network addresses the person let the hub answer on too (Add a
// device, Local network), kept in <home>/lan-v1.json so it listens there
// again after a restart (server.bend's Lan.*); private, written whole.
function pairingLanSaved(home, fs = require('node:fs')) {
  try { const d = JSON.parse(fs.readFileSync(require('node:path').join(home, 'lan-v1.json'), 'utf8'));return d && d.version === 1 && Array.isArray(d.ips) ? d.ips.filter(x => typeof x === 'string' && /^\d+\.\d+\.\d+\.\d+$/.test(x)) : []; } catch { return []; }
}
function pairingLanSave(home, ips, fs = require('node:fs')) {
  const path = require('node:path'), file = path.join(home, 'lan-v1.json'), tmp = file + '.' + require('node:crypto').randomBytes(8).toString('hex') + '.tmp';
  try { fs.writeFileSync(tmp, JSON.stringify({version: 1, ips}), {mode: 0o600, flag: 'wx'});fs.renameSync(tmp, file); }
  finally { if (fs.existsSync(tmp)) fs.unlinkSync(tmp); }
}
// The listener the hub just opened on ip:port (loop.js's handle table, the
// newest one there), so turning the address off closes it at once.
function pairingListener(ip, port) {
  let found;
  for (const h of globalThis.BP_H?.rows?.values?.() || []) if (h && h.server && h.server.hostname === ip && Number(h.server.port) === Number(port)) found = h;
  return found;
}
let bp_pairing;
function pairing_start(home, root, host, port) {
  try { bp_pairing = {store: pairingStore({home, root}), home, host, port, routes: [], machines: {}, peers: [], lan: new Map(), lanSaved: pairingLanSaved(home), lanNext: 0};return ''; }
  catch { return 'Cannot read or initialize device registry; existing pairings were preserved'; }
}
// whether the local network listener opened as `base` is still wanted
// (one turned off, or opened again since, ends: server.bend's Lan.loop)
function pairing_lan(ip, base) { return {$: bp_pairing?.lan.get(ip)?.base === Number(base) ? 'True' : 'False'}; }
// Local network listening (op lan-*, secret: the address). lan-on keeps
// the choice and asks the hub to bind (bind: true) unless it answers there
// already; lan-up records the bound listener and the id its connections
// count from (each opening its own 2^24, from 3 * 2^30: loopback counts
// from 1, the tailnet from 2^31); lan-fail drops a choice that could not
// bind (at boot, name "boot", it is kept: the address may come back);
// lan-off forgets it and closes the listener and its connections.
function pairingLanOp(op, ip, name) {
  const {home, host, port, lan} = bp_pairing, covered = host === '0.0.0.0' || host === ip;
  const save = ips => { pairingLanSave(home, ips);bp_pairing.lanSaved = ips; };
  if (op === 'lan-saved') return {ips: host === '0.0.0.0' ? [] : bp_pairing.lanSaved.filter(x => x !== host), port};
  if (op === 'lan-on') {
    if (!pairingLan(require('node:os').networkInterfaces()).includes(ip)) return {error: 'lan_unknown'};
    if (covered || lan.has(ip)) return {ok: true, ip, listening: true};
    if (!bp_pairing.lanSaved.includes(ip)) save([...bp_pairing.lanSaved, ip]);
    return {ok: true, ip, port, bind: true};
  }
  if (op === 'lan-up') {
    const base = (3221225472 + (bp_pairing.lanNext++ % 64) * 16777216) >>> 0;
    lan.set(ip, {base, listener: pairingListener(ip, port)});
    return {ok: true, ip, listening: true, base};
  }
  if (op === 'lan-fail') {
    if (name !== 'boot' && bp_pairing.lanSaved.includes(ip)) save(bp_pairing.lanSaved.filter(x => x !== ip));
    return {error: 'lan_unavailable'};
  }
  if (op === 'lan-off') {
    if (bp_pairing.lanSaved.includes(ip)) save(bp_pairing.lanSaved.filter(x => x !== ip));
    const was = lan.get(ip);lan.delete(ip);
    if (was?.listener) { try { was.listener.server?.stop(true); } catch {} was.listener.close(); }
    return {ok: true, ip, listening: covered};
  }
  return {error: 'invalid_request'};
}
function pairing_address(link) {
  const origin = pairingAddress(link);
  if (origin && bp_pairing && !bp_pairing.routes.includes(origin)) bp_pairing.routes.unshift(origin);
  return {$: 'Unit'};
}
function pairing_resolve(token, client, route) {
  try { return bp_pairing.store.resolve(token, Number(client), route); } catch { return ''; }
}
// (the machines this hub found, and the urls of the linked hubs it verified
// as the owner's: server.bend's Hub.machines)
function pairing_known(machines, peers) {
  if (!bp_pairing) return {$: 'Unit'};
  try { bp_pairing.machines = JSON.parse(machines) || {}; } catch { bp_pairing.machines = {}; }
  try { const p = JSON.parse(peers);bp_pairing.peers = Array.isArray(p) ? p.filter(x => typeof x === 'string') : []; } catch { bp_pairing.peers = []; }
  return {$: 'Unit'};
}
function pairing_active(client) { return {$: bp_pairing?.store.active(Number(client)) ? 'True' : 'False'}; }
function pairing_forget(client) { bp_pairing?.store.forget(Number(client));return {$: 'Unit'}; }
function pairing_request(op, secret, name, authority, https) {
  try {
    const {store, host, port} = bp_pairing;
    if (op === 'enroll') return JSON.stringify(store.enroll(secret, name));
    if (op === 'list') return JSON.stringify({...store.list(), lan: bp_pairing.lanSaved.map(ip => ({ip, url: 'http://' + ip + ':' + bp_pairing.port, listening: bp_pairing.lan.has(ip)}))});
    if (op.startsWith('lan-')) return JSON.stringify(pairingLanOp(op, secret, name));
    if (op === 'revoke') return JSON.stringify(store.revoke(secret));
    if (op === 'uninvite') return JSON.stringify(store.uninvite(secret));
    if (op === 'machine') return JSON.stringify(store.machine(secret));
    if (op === 'link') return JSON.stringify(store.link(secret, name));
    if (op === 'unlinked') return JSON.stringify(store.unlinked(secret));
    if (op === 'known') return JSON.stringify({known: pairingKnown(secret, {routes: [...bp_pairing.routes, 'http://' + host + ':' + port, ...[...bp_pairing.lan.keys()].map(ip => 'http://' + ip + ':' + port)], machines: bp_pairing.machines, peers: bp_pairing.peers})});
    if (op === 'create') {
      const plan = pairingPlan({routes: bp_pairing.routes, host, port, authority, https, interfaces: require('node:os').networkInterfaces(), bound: [...bp_pairing.lan.keys()]});
      return JSON.stringify(store.create(plan.routes, plan.choices));
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

io_eff(CID(Pairing.known), pairing_known);

io_eff(CID(Pairing.lan), pairing_lan);
