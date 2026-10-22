'use strict';
// The desktop producer's prepared shell and Electron components for one frozen
// release commit (0.12.0 rebound a frozen older shell; from 0.12.1 the shell is
// packed from the release's own desktop/ sources). Per target it writes
//   <out>/<target>/shell-payload/   the app: every desktop/ file main.cjs and its
//                                   preloads reach, LICENSE.Backplane, node_modules/ws
//   <out>/<target>/SHELL.json       kind backplane-electron-shell
//   <out>/<target>/payload/         the upstream Electron 44.9.2 runtime, copied
//                                   unchanged from the retained vendor component
//   <out>/<target>/ELECTRON.json    its receipt, bound to this release
//   <out>/<target>/COMPONENT-INPUTS.json   what compose-target-freeze reads
// No download, compile, payload execution, staging, signing or publication.
//
//   node desktop/prepare-components.cjs --source ABS_FROZEN_CHECKOUT --vendor ABS_VENDOR \
//     --out NEW_ABS_DIR [--targets linux-x64,windows-x64] [--direct-browser off]
// ABS_VENDOR holds <target>/{payload/, ELECTRON.json[, UPSTREAM.json]} (0.12.0's retained
// components). The checkout must be clean at its commit with desktop/node_modules/ws
// installed (npm ci --ignore-scripts).
const fs = require('node:fs'), path = require('node:path'), {execFileSync} = require('node:child_process');
const p = require('./produce-release.cjs');
const ELECTRON = '44.9.2', WS = '8.22.0', BUNDLE = 'dev.backplane.desktop';
const check = (ok, msg) => { if (!ok) throw new Error(msg); };
const ref = file => ({path: file, sha256: p.digest(file), bytes: fs.statSync(file).size});

function args(argv) {
  const a = {};
  for (let i = 0; i < argv.length; i += 2) {
    check(['--source', '--vendor', '--out', '--targets', '--direct-browser'].includes(argv[i]) && !(argv[i] in a) && argv[i + 1], 'Use --source --vendor --out [--targets] [--direct-browser on|off]');
    a[argv[i]] = argv[i + 1];
  }
  for (const k of ['--source', '--vendor', '--out']) check(a[k] && path.isAbsolute(a[k]), k + ' must be an absolute path');
  check(['on', 'off', undefined].includes(a['--direct-browser']), '--direct-browser is on or off');
  return a;
}

const git = (src, ...a) => execFileSync('git', a, {cwd: src, encoding: 'utf8'}).trim();

// the app's files: package.json's main and every ./ require, __dirname file and
// html src/href it reaches, transitively; one missing stops the build
function appFiles(desktop) {
  const pkg = JSON.parse(fs.readFileSync(path.join(desktop, 'package.json')));
  check(pkg.name === 'backplane-desktop' && pkg.main === 'main.cjs', 'Unexpected desktop/package.json');
  const seen = new Set(['package.json']), todo = [pkg.main];
  while (todo.length) {
    const name = todo.pop();
    if (seen.has(name)) continue;
    const file = path.join(desktop, name);
    check(fs.existsSync(file) && fs.statSync(file).isFile(), 'The app references a missing file: desktop/' + name);
    seen.add(name);
    const text = fs.readFileSync(file, 'utf8');
    const refs = [...text.matchAll(/require\(\s*['"]\.\/([^'"]+)['"]\s*\)/g), ...text.matchAll(/__dirname\s*,\s*['"]([^'"]+)['"]/g),
      ...(name.endsWith('.html') ? text.matchAll(/(?:src|href)=["']\.?\/?([\w.-]+\.(?:js|css|cjs))["']/g) : [])].map(m => m[1]);
    for (const r of refs) { check(!r.includes('..') && !path.isAbsolute(r), 'Reference leaves desktop/: ' + r); todo.push(r); }
  }
  return [...seen].sort();
}

// what the shell receipt claims, checked against the sources it is made from
function features(desktop) {
  const main = fs.readFileSync(path.join(desktop, 'main.cjs'), 'utf8'), service = fs.readFileSync(path.join(desktop, 'service.cjs'), 'utf8');
  check(/contextIsolation:\s*true/.test(main) && /sandbox:\s*true/.test(main) && /nodeIntegration:\s*false/.test(main), 'main.cjs lacks isolated, sandboxed, node-free windows');
  check(/BACKPLANE_HOME/.test(main), 'main.cjs lacks the packaged default home');
  check(/function launchPaths/.test(service), 'service.cjs lacks Windows startup path mapping');
  check(!/stop\(\)[^]*?managed/.test(main), 'main.cjs gained a managed stop');
  return {defaultPackagedHome: true, contextIsolation: true, sandbox: true, nodeIntegration: false, managedStop: false,
    managedUpdate: false, windowsManagedControl: 'UNSUPPORTED', windowsNativePathMapping: true,
    directBrowser: fs.existsSync(path.join(desktop, 'browser-guest.cjs'))};
}

function copyTree(from, to) { fs.cpSync(from, to, {recursive: true, verbatimSymlinks: true, preserveTimestamps: false}); }

function prepare(a) {
  const src = a['--source'], desktop = path.join(src, 'desktop'), out = a['--out'];
  const commit = git(src, 'rev-parse', 'HEAD'), tree = git(src, 'rev-parse', 'HEAD^{tree}');
  check(!git(src, 'status', '--porcelain', '--untracked-files=no'), 'Frozen checkout is dirty');
  const version = JSON.parse(fs.readFileSync(path.join(desktop, 'package.json'))).version;
  check(/^\d+\.\d+\.\d+$/.test(version), 'desktop/package.json version is not stable: ' + version);
  const wsRoot = path.join(desktop, 'node_modules/ws');
  check(JSON.parse(fs.readFileSync(path.join(wsRoot, 'package.json'))).version === WS, 'desktop/node_modules/ws must be ' + WS + ' (npm ci --ignore-scripts)');
  check(!fs.existsSync(out), 'Output exists');
  // --direct-browser off: the shell claims no in-app browser, so the
  // producer writes no desktopBrowser into DESKTOP-BUILD.json and main.cjs
  // starts without it (the files stay: main.cjs requires them either way)
  const files = appFiles(desktop), feats = {...features(desktop), ...(a['--direct-browser'] === 'off' ? {directBrowser: false} : {})};
  const targets = (a['--targets'] || Object.keys(p.TARGETS).join(',')).split(',');
  for (const t of targets) check(p.TARGETS[t], 'Unknown target ' + t);
  const release = {version, tag: 'v' + version, channel: 'release', commit, tree};
  fs.mkdirSync(out, {recursive: true});
  const results = {};
  for (const target of targets) {
    const dest = path.join(out, target), shell = path.join(dest, 'shell-payload');
    fs.mkdirSync(path.join(shell, 'node_modules'), {recursive: true});
    for (const f of files) fs.copyFileSync(path.join(desktop, f), path.join(shell, f));
    fs.copyFileSync(path.join(src, 'LICENSE'), path.join(shell, 'LICENSE.Backplane'));
    const wsOut = path.join(shell, 'node_modules/ws');
    fs.mkdirSync(wsOut);
    for (const f of ['package.json', 'index.js', 'browser.js', 'wrapper.mjs', 'LICENSE', 'README.md']) fs.copyFileSync(path.join(wsRoot, f), path.join(wsOut, f));
    copyTree(path.join(wsRoot, 'lib'), path.join(wsOut, 'lib'));
    const shellReceipt = {schema: 1, kind: 'backplane-electron-shell', status: 'VERIFIED', target, sourceCommit: commit, sourceTree: tree,
      version, channel: 'release', electronVersion: ELECTRON, bundleIdentifier: BUNDLE, features: feats,
      sourceFiles: Object.fromEntries(files.map(f => ['desktop/' + f, p.digest(path.join(desktop, f))])),
      packages: [
        {name: 'Backplane Electron shell', version, sourceDisposition: 'Source at Backplane commit ' + commit,
          licenses: [{path: 'LICENSE.Backplane', sha256: p.digest(path.join(shell, 'LICENSE.Backplane'))}]},
        {name: 'ws', version: WS, sourceDisposition: 'MIT upstream source at https://github.com/websockets/ws/tree/' + WS + ', the npm archive desktop/package-lock.json pins',
          licenses: [{path: 'node_modules/ws/LICENSE', sha256: p.digest(path.join(wsOut, 'LICENSE'))}]}],
      members: p.inventory(shell, target),
      qualificationScope: 'Packed from the release commit\'s own desktop/ sources (prepare-components.cjs); the features are checked against them. Target runtime acceptance is separate.',
      runtimeAcceptance: 'PENDING matching-host runtime'};
    p.write(path.join(dest, 'SHELL.json'), shellReceipt);
    // Electron: the retained upstream payload, unchanged; its receipt names this release
    const vendor = path.join(a['--vendor'], target);
    const old = p.json(path.join(vendor, 'ELECTRON.json'));
    check(old.kind === 'backplane-electron-runtime' && old.target === target && old.electronVersion === ELECTRON, 'Vendor Electron receipt mismatch: ' + target);
    copyTree(path.join(vendor, 'payload'), path.join(dest, 'payload'));
    check(p.canonical(p.inventory(path.join(dest, 'payload'), target)) === p.canonical(old.members), 'Vendor Electron payload differs from its receipt: ' + target);
    if (fs.existsSync(path.join(vendor, 'UPSTREAM.json'))) fs.copyFileSync(path.join(vendor, 'UPSTREAM.json'), path.join(dest, 'UPSTREAM.json'));
    const electron = {...old, sourceCommit: commit, sourceTree: tree, version, channel: 'release', runtimeAcceptance: 'PENDING matching-host runtime',
      sourceCompositionRebind: {fromCommit: old.sourceCommit, fromVersion: old.version, toCommit: commit, previousReceiptSha256: p.digest(path.join(vendor, 'ELECTRON.json')), upstreamArchiveAndPayloadUnchanged: true}};
    p.write(path.join(dest, 'ELECTRON.json'), electron);
    const inputs = {schema: 1, kind: 'backplane-desktop-prepared-components', target, release,
      shell: {receipt: {...ref(path.join(dest, 'SHELL.json')), path: 'SHELL.json'}, payload: {directory: 'shell-payload'}},
      electron: {receipt: {...ref(path.join(dest, 'ELECTRON.json')), path: 'ELECTRON.json'}, payload: {directory: 'payload'}},
      remainingStageInputs: ['service', 'release.freezeReceipt']};
    p.write(path.join(dest, 'COMPONENT-INPUTS.json'), inputs);
    results[target] = {componentInputs: ref(path.join(dest, 'COMPONENT-INPUTS.json')),
      shellReceiptSha256: p.digest(path.join(dest, 'SHELL.json')), electronReceiptSha256: p.digest(path.join(dest, 'ELECTRON.json'))};
  }
  check(git(src, 'rev-parse', 'HEAD') === commit && !git(src, 'status', '--porcelain', '--untracked-files=no'), 'Frozen checkout changed during preparation');
  const result = {schema: 1, status: 'PREPARED_COMPONENTS_NOT_RUNTIME_ACCEPTANCE', release, appFiles: files, targets: results, published: false};
  p.write(path.join(out, 'RESULT.json'), result);
  return result;
}

if (require.main === module) {
  try { console.log(JSON.stringify(prepare(args(process.argv.slice(2))), null, 2)); }
  catch (e) { console.error('prepare-components: ' + e.message); process.exit(1); }
}
module.exports = {prepare, appFiles, features};
