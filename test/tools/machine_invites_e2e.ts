// End to end: hub-to-hub invites (bp1:, Bots > link a machine) are recorded
// with the device invitations (effects/pairing.js, docs/pairing.md): listed
// in /auth/devices as kind "machine", used once by the machine that joins,
// revoked on request (unused: its secret goes and it never links; used: the
// link is removed), and an invite the hub has no record of (a secret file
// from before invites were recorded) never links. Two headless hubs on
// loopback with stand-in agents and their own homes; run it in a network
// namespace of its own (unshare -rn) or on free ports.
//
//   bun test/tools/machine_invites_e2e.ts BINARY
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync, chmodSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createHmac, randomBytes} from 'node:crypto';
import {connect} from '../../skills/backplane-e2e/rpc.mjs';
const [binary] = process.argv.slice(2).filter(x => !x.startsWith('--'));
if (!binary) throw Error('BINARY required');
const root = mkdtempSync(join(tmpdir(), 'bp-machine-invites-')), fake = join(root, 'bin');
mkdirSync(fake);
for (const n of ['claude', 'codex', 'grok', 'muse', 'opencode', 'pi', 'omp', 'tailscale']) writeFileSync(join(fake, n), '#!/bin/sh\nexit 127\n', {mode: 0o755});
let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => { console.log(`${ok ? 'ok' : 'FAIL'} ${name}${ok ? '' : ': ' + JSON.stringify(got)?.slice(0, 400)}`); if (!ok) failed = true; };
const sleep = (n: number) => new Promise(r => setTimeout(r, n));
async function until<T>(ms: number, f: () => Promise<T> | T): Promise<T | undefined> { const end = Date.now() + ms; for (;;) { const v = await f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(150); } }
const freePort = () => { const s = Bun.listen({hostname: '127.0.0.1', port: 0, socket: {data() {}}}); const p = s.port; s.stop(true); return p; };
type Hub = {name: string, home: string, port: number, proc?: ReturnType<typeof Bun.spawn>};
const hubs: Hub[] = [];
async function start(name: string): Promise<Hub> {
  const h: Hub = {name, home: join(root, name), port: freePort()};
  mkdirSync(h.home);
  h.proc = Bun.spawn([resolve(binary), '--home', h.home, '--port', String(h.port), '--no-tailscale'], {
    env: {...process.env, HOME: root, PATH: `${fake}:/usr/bin:/bin`, DISPLAY: '', WAYLAND_DISPLAY: '', BACKPLANE_NAME: name, BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: '1',
      BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_TELEMETRY: 'off', BACKPLANE_KEYRING_LOCKED: '0', BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_PEERS: ''},
    stdout: 'ignore', stderr: 'ignore'});
  hubs.push(h);
  if (!await until(60000, async () => { try { return (await fetch(`http://127.0.0.1:${h.port}/hello`)).status === 200; } catch { return false; } })) throw Error(name + ' did not start');
  return h;
}
const token = (h: Hub) => readFileSync(join(h.home, 'token'), 'utf8').trim();
async function rpc(h: Hub, m: string, p: object): Promise<any> { const c = await connect(h.port, token(h)); try { return await c.call(m, p); } catch (e) { return {ok: false, error: String(e)}; } finally { c.close(); } }
async function api(h: Hub, path: string, method = 'GET', headers: Record<string, string> = {}) { const r = await fetch(`http://127.0.0.1:${h.port}${path}`, {method, headers: {Authorization: 'Bearer ' + token(h), ...headers}}); let v: any = {}; try { v = await r.json(); } catch {} return {status: r.status, v}; }
const invites = async (h: Hub) => (await api(h, '/auth/devices')).v.invitations ?? [];
const peerOf = (invite: string) => /^bp1:.*:(p[0-9a-f]+):([0-9a-f]{64})$/.exec(invite);
const secretFile = (h: Hub, peer: string) => join(h.home, 'secrets', 'peers', peer);
const peerSets = (h: Hub, peer: string) => readFileSync(join(h.home, 'events.jsonl'), 'utf8').split('\n').filter(l => l.includes('"PeerSet"') && l.includes(`"${peer}"`)).map(l => JSON.parse(l));
try {
  const a = await start('alpha'), b = await start('beta');
  // made and recorded
  const i1 = await rpc(a, 'bots.invite', {url: `http://127.0.0.1:${a.port}`});
  const m1 = peerOf(i1?.invite ?? '');
  check('an invite is made', !!m1, i1);
  const r1 = (await invites(a)).find((x: any) => x.kind === 'machine');
  check('and listed with the pairing links as a machine invite, open', r1?.state === 'open', await invites(a));
  check('good for ten minutes, as a device invitation', r1 && r1.expiresAt - r1.createdAt === 600000, r1);
  check('the list holds no secret', !JSON.stringify(await invites(a)).includes(m1![2]));
  // used by the machine that joins
  const j1 = await rpc(b, 'bots.join', {invite: i1.invite});
  check('the other hub joins with it', !!j1?.ok, j1);
  const u1 = await until(5000, async () => (await invites(a)).find((x: any) => x.id === r1.id && x.state === 'used'));
  check('it is recorded as used, by beta', u1?.deviceName === 'beta' && u1?.device === m1![1], u1);
  // a secret file the hub never recorded (an invite from before) does not link
  const old = 'p' + randomBytes(8).toString('hex'), oldSecret = randomBytes(32).toString('hex');
  mkdirSync(join(a.home, 'secrets', 'peers'), {recursive: true});
  writeFileSync(secretFile(a, old), JSON.stringify({secret: oldSecret}), {mode: 0o600});
  const body = JSON.stringify({url: 'http://127.0.0.1:1', name: 'old'}), ts = Math.floor(Date.now() / 1000) + '';
  const sig = 'sha256=' + createHmac('sha256', Buffer.from(oldSecret, 'hex')).update(`${ts}.${body}`).digest('hex');
  const l0 = await fetch(`http://127.0.0.1:${a.port}/bots/link`, {method: 'POST', body, headers: {'content-type': 'application/json', 'x-backplane-peer': old, 'x-backplane-timestamp': ts, 'x-backplane-signature': sig}});
  check('an unrecorded invite does not link', l0.status === 401 && (await l0.text()).includes('no record'), l0.status);
  check('no link made for it', peerSets(a, old).length === 0);
  check('and its secret is removed', !!await until(3000, () => !existsSync(secretFile(a, old))));
  // revoked unused: never links, secret gone
  const i2 = await rpc(a, 'bots.invite', {url: `http://127.0.0.1:${a.port}`});
  const m2 = peerOf(i2?.invite ?? '');
  const r2 = (await invites(a)).find((x: any) => x.kind === 'machine' && x.state === 'open');
  const v2 = await api(a, '/auth/revoke', 'POST', {'X-Backplane-Invitation': r2?.id ?? ''});
  check('an unused invite is revoked', v2.status === 200 && (await invites(a)).find((x: any) => x.id === r2.id)?.state === 'revoked', v2);
  check('its secret is gone', !existsSync(secretFile(a, m2![1])));
  const j2 = await rpc(b, 'bots.join', {invite: i2.invite});
  check('and joining with it fails', !j2?.ok, j2);
  check('no link made with it', peerSets(a, m2![1]).length === 0, peerSets(a, m2![1]));
  // revoked used: the link is removed
  const v1 = await api(a, '/auth/revoke', 'POST', {'X-Backplane-Invitation': r1.id});
  check('a used invite is revoked', v1.status === 200, v1);
  check('its link is removed', !!await until(5000, () => peerSets(a, m1![1]).some(x => x.revoked === true)), peerSets(a, m1![1]));
  check('its secret is gone', !existsSync(secretFile(a, m1![1])));
  const l1 = (await invites(a)).find((x: any) => x.id === r1.id);
  check('listed as revoked with the machine unlinked', l1?.state === 'revoked' && l1?.deviceRevoked === true, l1);
  // a link removed in Bots shows as unlinked
  const i3 = await rpc(a, 'bots.invite', {url: `http://127.0.0.1:${a.port}`});
  const m3 = peerOf(i3?.invite ?? '');
  check('a third invite joins', !!(await rpc(b, 'bots.join', {invite: i3.invite}))?.ok);
  const un = await rpc(a, 'bots.unpeer', {peer: m3![1]});
  check('the link is removed in Bots', !!un?.ok, un);
  check('its invite shows the machine unlinked', !!await until(5000, async () => (await invites(a)).find((x: any) => x.device === m3![1] && x.state === 'used' && x.deviceRevoked === true)), await invites(a));
  // records survive a restart
  a.proc!.kill(); await a.proc!.exited;
  a.proc = Bun.spawn([resolve(binary), '--home', a.home, '--port', String(a.port), '--no-tailscale'], {env: {...process.env, HOME: root, PATH: `${fake}:/usr/bin:/bin`, DISPLAY: '', BACKPLANE_NAME: 'alpha', BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: '1', BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BACKPLANE_TELEMETRY: 'off', BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_PEERS: ''}, stdout: 'ignore', stderr: 'ignore'});
  await until(60000, async () => { try { return (await fetch(`http://127.0.0.1:${a.port}/hello`)).status === 200; } catch { return false; } });
  check('the records survive a restart', (await invites(a)).filter((x: any) => x.kind === 'machine').length === 3, await invites(a));
} finally {
  for (const h of hubs) if (h.proc && h.proc.exitCode === null) { h.proc.kill(); await h.proc.exited; }
  rmSync(root, {recursive: true, force: true});
}
console.log(failed ? 'machine invites e2e: FAIL' : 'machine invites e2e: ok');
process.exit(failed ? 1 : 0);
