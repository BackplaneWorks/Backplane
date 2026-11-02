'use strict';
// A real Windows/Bun child, not a mocked process.platform or a running hub.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {spawnSync} = require('node:child_process');
const {createHash} = require('node:crypto');
const {launchPaths} = require('../service.cjs');

const out = path.resolve('build/windows-home-native-smoke/result.json');
const report = {
  status: 'INCOMPLETE', platform: process.platform, arch: process.arch,
  sourceCommit: process.env.GITHUB_SHA || null,
  serviceSHA256: createHash('sha256').update(fs.readFileSync(path.join(__dirname, '../service.cjs'))).digest('hex'),
  scope: 'Native Windows launchPaths HOME handoff and temporary model-cache read in one Bun child; no hub or provider is started',
};
function save() {
  fs.mkdirSync(path.dirname(out), {recursive:true});
  fs.writeFileSync(out, JSON.stringify(report, null, 2) + '\n');
}
const probe = `
if (process.platform !== 'win32' || !process.versions.bun) process.exit(41);
if (!Object.keys(process.env).some(k => k.toLowerCase() === 'home') || !process.env.HOME) process.exit(42);
const fs = require('node:fs');
const cache = JSON.parse(fs.readFileSync(process.env.HOME + '/.codex/models_cache.json', 'utf8'));
process.stdout.write(JSON.stringify({platform:process.platform, bun:process.versions.bun,
  home:process.env.HOME, appHome:process.env.BACKPLANE_HOME,
  models:cache.models.filter(m => m.visibility === 'list').map(m => m.slug)}));
`;
function child(env) {
  const r = spawnSync('bun', ['-e', probe], {
    env, encoding:'utf8', timeout:15000, windowsHide:true, shell:false,
  });
  if (r.error) throw r.error;
  return r;
}

async function main() {
  if (process.platform !== 'win32') {
    report.status = 'NOT_RUN_NATIVE_WINDOWS'; save();
    console.error('This smoke requires native Windows and Bun.');
    process.exitCode = 77; return;
  }
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-native-home-'));
  const parentHome = process.env.HOME, parentProfile = process.env.USERPROFILE;
  try {
    const profile = path.join(root, 'User Profile 日本');
    const home = path.join(root, 'Separate application data');
    const service = path.join(root, 'Native service');
    fs.mkdirSync(path.join(profile, '.codex'), {recursive:true});
    fs.mkdirSync(service, {recursive:true});
    fs.writeFileSync(path.join(service, 'BUILD.json'), JSON.stringify({serviceRuntime:'bun'}));
    fs.writeFileSync(path.join(profile, '.codex/models_cache.json'), JSON.stringify({models:[
      {slug:'fixture-one', visibility:'list'}, {slug:'fixture-hidden', visibility:'hide'},
      {slug:'fixture-two', visibility:'list'},
    ]}));
    const env = {...process.env};
    for (const key of Object.keys(env)) {
      if (['home', 'userprofile'].includes(key.toLowerCase())) delete env[key];
    }
    env.USERPROFILE = profile;
    assert(!Object.keys(env).some(k => k.toLowerCase() === 'home'));
    const before = child(env);
    assert.equal(before.status, 42, 'Negative control must fail because HOME is absent');
    report.missingHomeNegativeControlExit = before.status;

    const launch = await launchPaths({
      home, binary:path.join(service, 'backplane-serve.exe'),
      web:path.join(service, 'web'), env,
    });
    assert.equal(launch.env.HOME, profile);
    assert.equal(launch.env.BACKPLANE_HOME, home);
    const after = child(launch.env);
    assert.equal(after.status, 0, after.stderr || 'Native Bun child failed');
    const result = JSON.parse(after.stdout);
    assert.equal(result.platform, 'win32');
    assert.equal(result.home, profile);
    assert.equal(result.appHome, home);
    assert.deepEqual(result.models, ['fixture-one', 'fixture-two']);
    assert(!Object.keys(env).some(k => k.toLowerCase() === 'home'));
    assert.equal(process.env.HOME, parentHome);
    assert.equal(process.env.USERPROFILE, parentProfile);
    report.nativeChild = result;
    report.status = 'PASS';
    console.log('ok native Windows: missing HOME fails before launchPaths, then native Bun reads the profile cache with separate app data');
  } finally {
    fs.rmSync(root, {recursive:true, force:true});
    report.tempDirectoryRemoved = !fs.existsSync(root);
  }
}
main().catch(error => {
  report.status = 'FAIL'; report.error = error.message;
  console.error(error.stack); process.exitCode = 1;
}).finally(save);
