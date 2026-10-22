// Tab identity and metadata events: bun test/controls.ts
// Uses dist/backplane-browser when BACKPLANE_BROWSER is set, else src/main.ts.
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

function check(what: string, ok: unknown): asserts ok {
  if (!ok) throw new Error(what);
  console.log(`ok  ${what}`);
}
async function until(fn: () => unknown, what: string, ms = 15000): Promise<void> {
  const end = Date.now() + ms;
  while (Date.now() < end) { if (await fn()) return; await Bun.sleep(25); }
  throw new Error(`timeout: ${what}`);
}

const root = mkdtempSync(join(tmpdir(), 'bp-controls-'));
const flags = ['--profile', join(root, 'profile'), '--jpeg-dir', join(root, 'shots')];
const cmd = process.env.BACKPLANE_BROWSER ? [process.env.BACKPLANE_BROWSER, ...flags]
  : ['bun', new URL('../src/main.ts', import.meta.url).pathname, ...flags];
const proc = Bun.spawn(cmd, { stdin: 'pipe', stdout: 'pipe', stderr: 'inherit' });
const events: any[] = [];
const waits = new Map<number, (v: any) => void>();
let nextId = 0;
(async () => {
  const dec = new TextDecoder();
  let buf = '';
  for await (const chunk of proc.stdout) {
    buf += dec.decode(chunk);
    let nl;
    while ((nl = buf.indexOf('\n')) >= 0) {
      const msg = JSON.parse(buf.slice(0, nl)); buf = buf.slice(nl + 1);
      if (msg.event) events.push(msg); else waits.get(msg.id)?.(msg);
    }
  }
})();
async function call(op: string, args: Record<string, unknown> = {}): Promise<any> {
  const id = ++nextId;
  const r: any = await new Promise((res, rej) => {
    const t = setTimeout(() => rej(new Error(`timeout ${op}`)), 35000);
    waits.set(id, (v) => { clearTimeout(t); res(v); });
    proc.stdin.write(JSON.stringify({ id, op, ...args }) + '\n'); proc.stdin.flush();
  });
  if (!r.ok) throw new Error(`${op}: ${r.error}`);
  return r.result;
}

const site = Bun.serve({hostname:'127.0.0.1', port:0, fetch(req) {
  const path = new URL(req.url).pathname;
  return new Response(`<!doctype html><title>Site ${path}</title><input id=q><a target=_blank href=/popup>Link popup</a>`, {headers:{'content-type':'text/html'}});
}});
const url = (path: string) => `http://127.0.0.1:${site.port}/${path}`;
try {
  await call('open', {page:'a', url:url('first')});
  await call('type', {page:'a', selector:'#q', text:'preserved', clear:true});
  const first = (await call('tabs',{page:'a'})).tabs[0];
  check('stable tab identity', typeof first.id === 'string');
  await call('tabs', {page:'a', new:url('second')});
  const list = await call('tabs', {page:'a'});
  const second = list.tabs.find((t:any) => t.active);
  await call('tabs', {page:'a', selectId:first.id});
  check('stable selection retains document', (await call('evaluate', {page:'a', expression:'q.value'})) === 'preserved');
  await call('tabs', {page:'a', closeId:second.id});
  let stale = false;
  try { await call('tabs', {page:'a', closeId:second.id}); } catch { stale = true; }
  check('stale identity cannot close another tab', stale && (await call('tabs',{page:'a'})).tabs[0].id === first.id);
  await call('open', {page:'b', url:url('other-owner')});
  let cross = false;
  try { await call('tabs', {page:'b', closeId:first.id}); } catch { cross = true; }
  check('cross-owner identity refused', cross);
  await until(() => events.some(e => e.event === 'tabs' && e.page === 'a' && e.tabs.some((t:any) => t.id === first.id && t.active && t.title === 'Site /first')), 'metadata event');
  const n = events.length;
  await call('cast', {page:'a', ms:0});
  check('new viewer gets cached metadata without a title query', events.slice(n).some(e => e.event === 'tabs' && e.page === 'a' && e.url === url('first')));
  await call('close', {page:'b'});
  check('closed owner clears metadata', events.some(e => e.event === 'tabs' && e.page === 'b' && e.tabs.length === 0));
  console.log('all browser control metadata checks passed');
} finally {
  await call('close').catch(() => {}); proc.stdin.end(); await proc.exited;
  site.stop(true); rmSync(root, { recursive: true, force: true });
}
