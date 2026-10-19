// Actual compiled helper/CEF through the new temp-prefix deployment path.
// No build, live data or actual installation. Run via bp-admit --mem 6 --cores 2.
import { copyFileSync, existsSync, mkdirSync, readFileSync, readlinkSync, realpathSync, symlinkSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { check, jsonProcess, privateDisplay } from '../../cef/test/harness.ts';
const helper = resolve(process.env.BACKPLANE_BROWSER!);
const host = resolve(process.env.BACKPLANE_CEF!);
const originalRuntime = resolve(process.env.CEF_BASE_RUNTIME!);
const originalHelper = resolve(process.env.CEF_BASE_HELPER!);
if (![helper, host, originalHelper, join(originalRuntime, 'backplane-cef')].every(existsSync)) throw new Error('explicit saved host/helper and original runtime/helper required');
const d = await privateDisplay();
const prefix = join(d.root, 'prefix'), stage = join(d.root, 'package'), cache = join(d.root, 'cache');
const cacheParent = join(cache, 'backplane', 'cef-runtime');
mkdirSync(prefix); mkdirSync(stage); mkdirSync(cacheParent, { recursive: true });
symlinkSync(originalRuntime, join(cacheParent, '154.0.34'));
copyFileSync(originalHelper, join(prefix, 'backplane-browser'));
await Bun.write(join(prefix, 'backplane-browser.stamp'), 'legacy\n');
copyFileSync(helper, join(stage, 'backplane-browser'));
await Bun.write(join(stage, 'backplane-browser.stamp'), 'saved-acceptance\n');
const env = { ...d.env, XDG_CACHE_HOME: cache, BACKPLANE_CEF_NO_GPU: '1' };
delete env.BACKPLANE_CEF;
const hash = (path: string) => new Bun.CryptoHasher('sha256').update(readFileSync(path)).digest('hex');
const originalHash = hash(join(originalRuntime, 'backplane-cef'));
async function python(...args: string[]) {
  const p = Bun.spawn(['python3', 'scripts/cef-runtime.py', ...args], { env, stdout: 'pipe', stderr: 'pipe' });
  const err = await new Response(p.stderr).text();
  if (await p.exited) throw new Error(err);
}
let h: ReturnType<typeof jsonProcess> | undefined;
try {
  await python('stage', stage, originalRuntime, host, helper, join(stage, 'backplane-browser.stamp'), 'saved-acceptance-host');
  await python('install', stage, prefix);
  const bundle = realpathSync(join(prefix, 'browser-current'));
  check('installed helper and host share the selected immutable bundle', realpathSync(join(prefix, 'backplane-browser')) === join(bundle, 'backplane-browser') && realpathSync(join(prefix, 'cef')) === join(bundle, 'cef'));
  h = jsonProcess([join(prefix, 'backplane-browser'), '--profile', join(d.root, 'profile'), '--jpeg-dir', join(d.root, 'shots')], env);
  const e = await h.call('engine', { engine: 'cef', parent: d.parent, display: env.DISPLAY });
  check('compiled helper selects matched sibling host without an override', readlinkSync(`/proc/${e.hostPid}/exe`) === join(bundle, 'cef', 'backplane-cef'));
  check('running host and helper hashes match accepted artifacts', hash(`/proc/${e.hostPid}/exe`) === hash(host) && hash(`/proc/${h.proc.pid}/exe`) === hash(helper));
  await h.call('open', { page: 'install', url: 'data:text/html,<title>Matched runtime</title><input id=q>' });
  await h.call('show', { page: 'install', x: 10, y: 10, w: 600, h: 400 });
  await h.call('type', { page: 'install', selector: '#q', text: 'Matched helper and host' });
  check('installed pair automates its native page', await h.call('evaluate', { page: 'install', expression: "q.value === 'Matched helper and host'" }));
  await h.call('close');
  check('installed pair shuts down its owned host', await h.proc.exited === 0 && !existsSync(`/proc/${e.hostPid}`));
  await python('rollback', prefix);
  const previous = realpathSync(join(prefix, 'browser-current'));
  check('rollback restores original host and helper identities', hash(join(previous, 'cef', 'backplane-cef')) === originalHash && hash(join(previous, 'backplane-browser')) === hash(originalHelper));
  check('original cached runtime remains unchanged', hash(join(originalRuntime, 'backplane-cef')) === originalHash);
  console.log('all actual CEF runtime installation checks passed');
} finally {
  if (h && h.proc.exitCode === null) { h.proc.stdin.end(); await h.proc.exited; }
  await d.cleanup();
}
