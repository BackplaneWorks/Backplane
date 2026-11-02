// Executes production picker scripts. The Cygwin conversion contract is
// simulated on POSIX; on Windows the same scripts also run with real cygpath.
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import {runInNewContext} from 'node:vm';

const F = (await import(pathToFileURL(path.resolve(process.argv[2])).href)).default;
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-folder-'));
const home = path.join(root, 'home');
fs.mkdirSync(path.join(home, 'hw development 日本'), {recursive:true});
const run = (script, input, env = process.env) => spawnSync('sh', ['-c', script, 'sh', input], {
  env, cwd:root, encoding:'utf8', timeout:10000,
});
const ok = (name, script, input, expected, env) => {
  const r = run(script, input, env);
  assert.equal(r.status, 0, `${name}: ${r.stdout} ${r.stderr}`);
  assert.equal(r.stdout.trimEnd(), expected, name);
  console.log(`ok ${name}`);
};
const bad = (name, input, part, env) => {
  const r = run(F.resolve_script(), input, env);
  assert.notEqual(r.status, 0, name);
  assert(r.stdout.includes(part), `${name}: ${r.stdout} ${r.stderr}`);
  console.log(`ok ${name}`);
};
try {
  // Exercise FS.stat and the production Windows errno mapper together:
  // otherwise ENOENT=4058 looks like an access failure to the Bend caller.
  const loop = fs.readFileSync('src/server/effects/loop.js', 'utf8');
  const errnoCode = loop.slice(loop.indexOf('const BP_LINUX_ERRNO'), loop.indexOf('\nif (BP_WIN) {\n  io_strerror'));
  const errno = runInNewContext(`${errnoCode}\nbp_errno`, {BP_WIN:true});
  const effects = new Map();
  let error;
  const host = fs.readFileSync('src/server/effects/host.js', 'utf8').replace(/CID\(([^)]+)\)/g, '"$1"');
  runInNewContext(host, {
    process:{platform:'linux'},
    require: name => { assert.equal(name, 'fs'); return {statSync: () => { throw error; }}; },
    io_eff: (id, fn) => effects.set(id, fn), io_fail: code => code,
    BP_H:{errno},
  });
  for (const [code, uv, expected] of [['ENOENT', -4058, 2], ['ENOTDIR', -4052, 20], ['EACCES', -4092, 13]]) {
    error = {code, errno:uv};
    assert.equal(effects.get('FS.stat')('C:/missing'), expected);
  }
  console.log('ok native Windows stat errno distinguishes missing, non-directory and inaccessible paths');

  const env = {...process.env, HOME:home};
  if (process.platform === 'win32') {
    // No mock: validate the saved result with the native Windows filesystem.
    const native = home.replaceAll('\\', '/');
    for (const input of [native, home.replaceAll('/', '\\'), '/'+native]) {
      const r = run(F.resolve_script(), input, env);
      assert.equal(r.status, 0, `${input}: ${r.stdout} ${r.stderr}`);
      assert(fs.statSync(r.stdout.trimEnd()).isDirectory());
      assert(!r.stdout.startsWith('/'));
    }
    console.log('ok native Windows drive, backslashes and /C:/ resolve to existing native directories');
  } else {
    const native = fs.realpathSync(home);
    for (const input of ['', '~', home]) ok(`POSIX home ${JSON.stringify(input)}`, F.resolve_script(), input, native, env);
    ok('relative paths and spaces', F.resolve_script(), 'hw development 日本', `${native}/hw development 日本`, env);
    ok('tilde paths', F.resolve_script(), '~/hw development 日本', `${native}/hw development 日本`, env);
    fs.symlinkSync(home, path.join(root, 'alias'));
    ok('canonical symlinks', F.resolve_script(), path.join(root, 'alias'), native, env);
    const literal = '$(touch injected); `touch injected`';
    fs.mkdirSync(path.join(home, literal));
    ok('shell metacharacters remain literal', F.resolve_script(), literal, `${native}/${literal}`, env);
    assert(!fs.existsSync(path.join(root, 'injected')));
    bad('missing folder refused', `${home}/absent`, 'no such folder:', env);
    fs.writeFileSync(path.join(home, 'file'), 'x');
    bad('file refused', `${home}/file`, 'not a folder:', env);

    // A real directory tree behind fake drive/UNC mounts lets cd/pwd run
    // normally; only cygpath's OS mapping is replaced.
    const bin = path.join(root, 'bin'); fs.mkdirSync(bin);
    const drive = path.join(root, 'drive');
    const unc = path.join(root, 'unc');
    const rel = 'Users/test/hw development 日本';
    fs.mkdirSync(path.join(drive, rel), {recursive:true});
    fs.mkdirSync(path.join(unc, 'server/share/hw development 日本'), {recursive:true});
    const cygpath = `#!/usr/bin/env python3
import os, re, sys
mode, sep, p = sys.argv[1:]
assert sep == '--'
p = p.replace('\\\\', '/')
drive, unc = os.environ['TEST_DRIVE'], os.environ['TEST_UNC']
if mode == '-au':
    if re.match(r'^[a-zA-Z]:/', p): p = drive + p[2:]
    elif p.startswith('//'): p = unc + p[1:]
    else: sys.exit(2)
elif mode == '-am':
    if p == drive or p.startswith(drive+'/'): p = 'C:/' + p[len(drive):].lstrip('/')
    elif p.startswith(unc+'/'): p = '/' + p[len(unc):]
else: sys.exit(2)
print(p)
`;
    fs.writeFileSync(path.join(bin, 'cygpath'), cygpath, {mode:0o755});
    const win = {...env, PATH:`${bin}:${process.env.PATH}`, TEST_DRIVE:drive, TEST_UNC:unc};
    for (const input of [`C:/${rel}`, `c:/${rel}`, `C:/${rel}`.replaceAll('/', '\\'), `/C:/${rel}`, `/C:/${rel}`.replaceAll('/', '\\').replace(/^\\/, '/')]) {
      ok(`Windows input ${JSON.stringify(input)}`, F.resolve_script(), input, `C:/${rel}`, win);
    }
    for (const input of ['//server/share/hw development 日本', '\\\\server\\share\\hw development 日本']) {
      ok(`UNC input ${JSON.stringify(input)}`, F.resolve_script(), input, '//server/share/hw development 日本', win);
    }
    ok('drive root remains absolute', F.resolve_script(), 'C:/', 'C:/', win);
    ok('folder picker selection survives re-registration', F.resolve_script(), `C:/${rel}`, `C:/${rel}`, win);
    bad('drive-relative path rejected', 'C:relative', 'invalid drive path:', win);
    bad('missing drive folder rejected', 'C:/absent', 'no such folder:', win);
    bad('malformed HOME prefix is rejected', `${home}/C:/${rel}`, 'no such folder:', win);
    // Conversion failure must not fall through to HOME or print a success.
    fs.writeFileSync(path.join(bin, 'cygpath'), '#!/bin/sh\nexit 1\n', {mode:0o755});
    bad('conversion failure rejected', `C:/${rel}`, 'cannot resolve folder:', win);
  }
} finally {
  fs.rmSync(root, {recursive:true, force:true});
}
