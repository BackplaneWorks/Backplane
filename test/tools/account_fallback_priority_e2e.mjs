// Run against a supplied installed/candidate binary. No service build or wire artifact needed.
// bun test/tools/account_fallback_priority_e2e.mjs /absolute/backplane-serve
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {join, resolve, dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {randomBytes} from 'node:crypto';
import {cbor, plain} from '../../src/web/wire.js';

const binary = resolve(process.argv[2] || '/home/h/.local/share/backplane/backplane-serve');
const source = resolve(import.meta.dirname, '../..');
const dictionary = name => {
  const text = fs.readFileSync(join(source, 'src/core/cbor.bend'), 'utf8'), at = text.indexOf(`def Cbor.${name}()`);
  return JSON.parse(text.slice(text.indexOf('[', at), text.indexOf(']', at) + 1));
};
const keys = dictionary('keys'), words = dictionary('words');
function encode(value) {
  const head = (m, n) => n < 24 ? [m * 32 + n] : n < 256 ? [m * 32 + 24, n] : n < 65536 ? [m * 32 + 25, n >> 8, n & 255] : [m * 32 + 26, n >>> 24 & 255, n >>> 16 & 255, n >>> 8 & 255, n & 255];
  if (value === null) return [246];
  if (typeof value === 'boolean') return [value ? 245 : 244];
  if (typeof value === 'number') return head(0, value);
  if (typeof value === 'string') { const b = [...new TextEncoder().encode(value)]; return [...head(3, b.length), ...b]; }
  if (Array.isArray(value)) return [...head(4, value.length), ...value.flatMap(encode)];
  const pairs = Object.entries(value); return [...head(5, pairs.length), ...pairs.flatMap(([k, v]) => [...encode(k), ...encode(v)])];
}
const delay = ms => new Promise(r => setTimeout(r, ms));
async function until(check, label, limit = 20000) {
  const end = performance.now() + limit;
  do { const result = check(); if (result) return result; await delay(40); } while (performance.now() < end);
  throw Error('Timed out: ' + label);
}
const root = fs.mkdtempSync(join(tmpdir(), 'bp-account-priority-'));
console.log('evidence ' + root);
const profile = '00000000-0000-4000-8000-000000000001';
const quote = value => "'" + value.replaceAll("'", "'\\''") + "'";

async function scenario(cached) {
  const folder = join(root, cached ? 'cached' : 'stream'), home = join(folder, 'home'), fake = join(folder, 'agents');
  for (const dir of [home, fake, join(folder, 'project'), join(folder, 'web'), join(home, 'providers/accounts-v1', profile)]) fs.mkdirSync(dir, {recursive: true, mode: 0o700});
  const at = Math.floor(Date.now() / 1000), reset = at + 300;
  const token = randomBytes(32).toString('hex'); // Synthetic fixture token; never reads a live credential.
  fs.writeFileSync(join(home, 'token'), token, {mode: 0o600});
  fs.writeFileSync(join(home, 'providers/accounts-v1/profiles.json'), JSON.stringify({version: 1, profiles: [{id: profile, provider: 'claude', label: 'Claude_Backplane', epoch: 1, fingerprint: ''}]}), {mode: 0o600});
  const seed = [{$: 'ProjectCreated', id: 'fixture', title: 'Synthetic', root: join(folder, 'project'), at},
    {$: 'ThreadCreated', id: 't', project: 'fixture', title: 'Synthetic account recovery', env: 'local', branch: '', worktree: '', provider: 'claude', model: '', at},
    ...[['fallback.on', 'on'], ['fallback.chain', 'claude/default,codex/gpt-6.1-sol'], ['telemetry.enabled', 'off'], ['restart.continue', 'off'], ['onboard.done', '1'], ['onboard.uber', '1']].map(([key, value]) => ({$: 'SettingSet', key, value, at}))];
  if (cached) seed.push({$: 'SettingSet', key: 'fallback.outage.claude', value: JSON.stringify({scope: 'claude', reason: 'Weekly usage limit reached', reset, at}), at});
  fs.writeFileSync(join(home, 'events.jsonl'), seed.map(x => JSON.stringify(x)).join('\n') + '\n');
  const control = join(folder, 'control.json'), audit = join(folder, 'audit.jsonl');
  fs.writeFileSync(control, JSON.stringify({allClaudeLimited: false, reset}));
  for (const provider of ['claude', 'codex', 'grok', 'muse', 'opencode', 'pi', 'omp', 'agy']) {
    const command = ['claude', 'codex'].includes(provider) ? `exec ${quote(process.execPath)} ${quote(join(source, 'test/tools/account_fallback_priority_standin.cjs'))} ${provider} "$@"` : 'exit 127';
    fs.writeFileSync(join(fake, provider), '#!/bin/sh\n' + command + '\n', {mode: 0o755});
  }
  const lease = Bun.listen({hostname: '127.0.0.1', port: 0, socket: {data() {}}}), port = lease.port; lease.stop(true);
  assert(![3787, 3788].includes(port));
  const inherited = {...process.env};
  for (const key of Object.keys(inherited)) if (/^(OPENAI_|ANTHROPIC_|CODEX_|CLAUDE_)/.test(key)) delete inherited[key];
  const child = Bun.spawn([binary, '--home', home, '--port', String(port), '--host', '127.0.0.1', '--web', join(folder, 'web'), '--headless', '--foreground', '--no-tailscale'], {
    env: {...inherited, DISPLAY: '', WAYLAND_DISPLAY: '', BACKPLANE_AGENTS: fake, PATH: fake + ':' + dirname(process.execPath) + ':/usr/bin:/bin',
      BACKPLANE_NO_UPDATE: '1', BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_TELEMETRY: 'off', BACKPLANE_TELEMETRY_BAKE: '0', BACKPLANE_PEERS: '',
      BACKPLANE_ORDER_SERVER: 'http://127.0.0.1:9', BP_ACCOUNT_FALLBACK_CONTROL: control, BP_ACCOUNT_FALLBACK_AUDIT: audit},
    stdout: Bun.file(join(folder, 'hub.stdout')), stderr: Bun.file(join(folder, 'hub.stderr'))});
  let socket;
  const events = () => fs.readFileSync(join(home, 'events.jsonl'), 'utf8').split('\n').filter(Boolean).map(x => JSON.parse(x));
  const turns = () => fs.existsSync(audit) ? fs.readFileSync(audit, 'utf8').split('\n').filter(Boolean).map(x => JSON.parse(x)).filter(x => x.kind === 'turn') : [];
  try {
    const deadline = performance.now() + 30000;
    while (true) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`, {signal: AbortSignal.timeout(1000)})).ok) break; } catch {} if (performance.now() > deadline) throw Error('synthetic hub startup'); await delay(50); }
    const replies = new Map(); let seq = 0;
    socket = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}&view=1&frontend=1`);
    socket.binaryType = 'arraybuffer';
    socket.onmessage = event => { const value = plain(cbor(new Uint8Array(event.data), keys, words)); if (value.t === 'reply') replies.set(String(value.id), value); };
    await new Promise((r, j) => { socket.onopen = r; socket.onerror = j; });
    const rpc = async (m, p) => { const id = ++seq; socket.send(Uint8Array.from(encode({id, m, p}))); return until(() => replies.get(String(id)), m); };
    // Account operations must not silently proceed before capability negotiation.
    const unsupported = await rpc('accounts', {op: 'list'});
    assert.equal(unsupported.ok, false, 'unnegotiated account operation must be rejected');
    // Match the supported web client's explicit account-profile capability offer.
    const listed = await rpc('accounts', {op: 'list', accountProfiles: 1});
    assert.equal(listed.ok, true, 'synthetic account list rejected: ' + JSON.stringify(listed));
    await until(() => events().filter(e => e.$ === 'SettingSet' && e.key === 'accounts.inventory').some(e => JSON.parse(e.value).profiles?.some(p => p.id === profile && p.state === 'connected')), 'synthetic account status');
    const text = 'Synthetic original request: do this exactly once';
    const response = await rpc('turn.start', {thread: 't', text, msg: 'original', mode: 'queue'}); assert(response.ok, JSON.stringify(response));
    await until(() => turns().some(t => t.provider === 'claude' && t.owner === profile), 'same-provider retry');
    await until(() => events().some(e => e.$ === 'TurnChanged' && e.thread === 't' && e.state === 'completed'), 'same-provider completion');
    const first = turns();
    assert.equal(first.filter(t => t.provider === 'codex').length, 0, 'Codex must not run while Claude_Backplane is available');
    assert.equal(first.filter(t => t.provider === 'claude' && t.owner === profile).length, 1, 'replacement request must run exactly once');
    assert.equal(first.filter(t => t.provider === 'claude' && t.owner === 'Default').length, cached ? 0 : 1);
    assert(first.find(t => t.owner === profile).text.includes(text));
    assert.equal(first.find(t => t.owner === profile).resumed, '', 'replacement must not resume the Default session');
    assert.equal(events().filter(e => e.$ === 'MessagePosted' && e.msg === 'original').length, 1);
    const binding = JSON.parse(events().filter(e => e.$ === 'SettingSet' && e.key === 'account.thread.t.claude').at(-1).value);
    assert.equal(binding.id, profile); assert(binding.epoch > 0);
    assert(events().some(e => e.$ === 'ActivityLogged' && e.kind === 'account-switch' && e.summary.includes('Claude_Backplane')));
    console.log('ok ' + (cached ? 'cached legacy hold' : 'live synthetic exhaustion') + ' -> Claude_Backplane exactly once with owned session and visible note');
    if (!cached) {
      fs.writeFileSync(control, JSON.stringify({allClaudeLimited: true, reset}));
      const next = await rpc('turn.start', {thread: 't', text: 'Synthetic all exhausted request', msg: 'all-exhausted', mode: 'queue', accountBinding: JSON.stringify(binding)}); assert(next.ok, JSON.stringify(next));
      await until(() => turns().some(t => t.provider === 'codex'), 'cross-provider after every Claude account held');
      assert.equal(turns().filter(t => t.provider === 'codex').length, 1);
      console.log('ok Codex runs only after Default and Claude_Backplane both report exhaustion');
    }
  } finally { socket?.close(); child.kill(); await child.exited; }
}
await scenario(false);
await scenario(true);
console.log('ok installed-binary account fallback acceptance; evidence retained at ' + root);
