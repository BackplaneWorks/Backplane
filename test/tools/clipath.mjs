// Execute the real probe and merge against an isolated native-installer layout.
// On POSIX, uname/cygpath stand in for Cygwin's Windows path conversion only;
// this does not qualify a native Windows executable or launch a real provider.
import assert from 'node:assert/strict';
import {mkdtempSync, mkdirSync, writeFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';

const lib = (await import(pathToFileURL(resolve(process.argv[2])).href)).default;
if (process.platform === 'win32') {
  console.log('ok POSIX shell fixtures skipped on Windows; Windows path cases run in clipath_test.bend');
  process.exit(0);
}
const root = mkdtempSync(join(tmpdir(), 'bp-clipath-'));
try {
  const home = join(root, 'Person 日本'), local = join(home, '.local/bin');
  mkdirSync(local, {recursive:true});
  writeFileSync(join(local, 'claude'), '#!/bin/sh\nprintf "fixture-claude\\n"\n', {mode:0o700});
  const env = {...process.env, HOME:home, PATH:'/usr/bin:/bin', SHELL:'/bin/sh'};
  const run = (script, childEnv) => {
    const r = spawnSync('sh', ['-c', script], {env:childEnv, encoding:'utf8', timeout:15000});
    assert.equal(r.status, 0, r.stderr || String(r.error));
    return r.stdout;
  };
  const posix = run(lib.probe(), env);
  assert(posix.includes(`dir\t${local}\n`));
  assert(!posix.includes('winhome\t'));
  const path = lib.next(env.PATH, home, posix);
  const found = run('command -v claude; claude --version', {...env, PATH:path});
  assert.equal(found, `${local}/claude\nfixture-claude\n`);
  console.log('ok CLI probe discovers the native-installer .local/bin layout with the user HOME');

  if (process.platform === 'win32' || process.platform === 'cygwin') {
    console.log('ok simulated Cygwin path conversion skipped on Windows');
  } else {
    const fake = join(root, 'fake'); mkdirSync(fake);
    writeFileSync(join(fake, 'uname'), '#!/bin/sh\nprintf "CYGWIN_NT-fixture\\n"\n', {mode:0o700});
    writeFileSync(join(fake, 'cygpath'), `#!${process.execPath}
const [flag, value] = process.argv.slice(2);
const convert = p => p === process.env.HOME || p.startsWith(process.env.HOME + '/')
  ? 'C:/Users/Person 日本' + p.slice(process.env.HOME.length) : 'C:/Tools' + p;
if (flag === '-m') console.log(convert(value));
else if (flag === '-mp') console.log(value.split(':').map(convert).join(';'));
else process.exit(89);
`, {mode:0o700});
    const output = run(lib.probe(), {...env, PATH:fake + ':' + env.PATH});
    assert(output.includes('winhome\tC:/Users/Person 日本\n'));
    assert(output.includes('windir\tC:/Users/Person 日本/.local/bin\n'));
    const current = 'C:\\Runtime\\bin;C:\\Windows\\System32';
    const native = lib.next(current, 'C:\\Users\\Person 日本', output);
    assert(native.startsWith(current + ';'));
    assert(native.split(';').includes('C:/Users/Person 日本/.local/bin'));
    assert(!native.includes('/cygdrive/'));
    assert.equal(lib.next(native, 'C:\\Users\\Person 日本', output), native);
    // The old Cygwin service continues using only the POSIX rows.
    assert(lib.next('/usr/bin:/bin', home, output).split(':').includes(local));
    console.log('ok native Windows PATH keeps drive letters and discovers Claude through the real probe/merge with simulated Cygwin conversion');
  }
} finally {
  rmSync(root, {recursive:true, force:true});
}
