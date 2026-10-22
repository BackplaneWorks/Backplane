'use strict';
// Turn one target's complete release-smoke.cjs evidence into the producer's
// target acceptance (signing, sandbox, lifecycle, runtime acceptance), then run
// produce-release.cjs archive. It never runs the payload, a compiler, a sender
// or a signer. The 0.12.0 helper bound a pinned root policy; this one binds the
// smoke to the freeze's own executionTooling.nativeSmokeSha256 and takes the
// release identity from the stage.
//
//   node desktop/archive-after-smoke.cjs --stage ABS_STAGE --evidence ABS_EVIDENCE --out NEW_ABS --dist NEW_ABS
// ABS_EVIDENCE is release-smoke.cjs's --out folder, as the target host left it.
const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto'), assert = require('node:assert/strict');
const p = require('./produce-release.cjs');
const digest = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const pin = file => ({sha256: digest(file), bytes: fs.statSync(file).size});
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const write = (file, value) => fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n', {flag: 'wx'});
const REQUIRED = ['captured native child starts exact stable service using Unicode/space home', 'wrong credential and foreign-origin pairing refused',
  'actual Unicode current DOM, exact versions, authenticated isolated preload', 'actual renderer OS sandbox',
  'second launcher joins single resident UI/service', 'real socket reconnect and reload restore current content',
  'no unsupported managed-stop/update menu advertised', 'window close leaves exact captured backend resident'];

function main(argv) {
  const keys = ['--stage', '--evidence', '--out', '--dist'], a = {};
  assert.equal(argv.length, 8, 'Use ' + keys.join(' '));
  for (let i = 0; i < argv.length; i += 2) { assert(keys.includes(argv[i]) && !a[argv[i]] && path.isAbsolute(argv[i + 1]), 'Unknown/duplicate/relative argument'); a[argv[i]] = argv[i + 1]; }
  const r = read(path.join(a['--evidence'], 'result.json'));
  assert(r.status === 'PASS' && r.phase === 'done' && !r.error && !r.cleanupError, 'Requires an actual complete PASS; a partial or failed smoke cannot archive');
  const stage = read(path.join(a['--stage'], 'STAGE.json')), inspected = p.inspectStage(a['--stage']);
  assert(p.TARGETS[stage.target], 'Unknown desktop target');
  assert.deepEqual(r.stage, inspected, 'The smoke ran another stage'); assert.equal(stage.payloadManifestSha256, inspected.payloadManifestSha256);
  for (const [key, value] of Object.entries({target: stage.target, version: stage.release.version, commit: stage.release.commit, tree: stage.release.tree}))
    assert.equal(r[key], value, key);
  // the smoke that ran is the one the freeze names
  const freeze = p.readReference({...stage.components.freeze, path: path.join(a['--stage'], stage.components.freeze.path)});
  assert.equal(r.fixture.sha256, freeze.executionTooling?.nativeSmokeSha256, 'The smoke is not the one the release freeze names');
  assert.equal(stage.signingPolicy, 'unsigned-portable-allowed');
  assert.deepEqual(stage.lifecyclePolicy, {mode: 'resident-service', managedStop: false, managedUpdate: false});
  // the client reconnects by itself now ('automatic'); 0.12.0's smoke drove Retry
  assert(['automatic', 'explicit-retry'].includes(r.reconnectMode), 'Unknown reconnect mode'); assert.deepEqual(r.errors, []);
  assert.deepEqual(r.authSession, {matchingWindows: 1, uiPartition: true, cookieCount: 1, httpOnly: true}, 'Missing the named-window HttpOnly cookie witness');
  assert.deepEqual([...r.checks].sort(), [...REQUIRED].sort(), 'Missing or unexpected essential checks');
  assert(r.journal?.prefixExact && r.journal.noSendOrAgentTurn && r.journal.mailOff && r.journal.telemetryOff, 'Missing the no-send isolation journal witness');
  assert.equal(r.cleanup?.nativeChildPid, r.serviceIdentity?.pid); assert(r.serviceIdentity?.birth && r.serviceIdentity?.executable, 'Missing native identity');
  assert(r.cleanup.uiClosed === true && r.cleanup.portClosed === true && (Number.isInteger(r.cleanup.nativeChildExit) || ['SIGTERM', 'SIGKILL'].includes(r.cleanup.signal)), 'Incomplete owned cleanup');
  const service = read(path.join(a['--evidence'], 'service.json')), sandbox = read(path.join(a['--evidence'], 'sandbox.json')), single = read(path.join(a['--evidence'], 'single-instance.json'));
  assert.deepEqual(service.identity, r.serviceIdentity); assert.deepEqual(sandbox, r.sandbox); assert.equal(service.version, stage.release.version);
  assert.equal(single.secondExit, 0); assert.equal(single.windowCount, 1); assert.equal(single.originalUiPid, r.uiPid);
  assert.deepEqual(sandbox.prefs, {sandbox: true, nodeIntegration: false, contextIsolation: true});
  assert(Array.isArray(sandbox.argv) && !sandbox.argv.some(v => ['--no-sandbox', '--disable-seccomp-filter-sandbox', '--disable-namespace-sandbox', '--disable-gpu-sandbox'].includes(v)), 'The smoke ran without the OS sandbox');
  assert.equal(sandbox.arch, stage.target.split('-').at(-1)); assert.equal(sandbox.platform, stage.target.startsWith('windows-') ? 'win32' : stage.target.split('-')[0]);
  if (sandbox.platform === 'linux') { assert.equal(sandbox.kernel?.seccomp, 2); assert.equal(sandbox.kernel?.noNewPrivs, 1); assert(Object.values(sandbox.kernel.namespaces).some(n => n.renderer && n.main && n.renderer !== n.main)); }
  else assert.equal(sandbox.metric?.sandboxed, true);
  const binary = path.join(a['--stage'], p.TARGETS[stage.target].resources, 'backplane-service', stage.target === 'windows-x64' ? 'backplane-serve.exe' : 'backplane-serve');
  assert.deepEqual(pin(binary), {sha256: service.binary.sha256, bytes: service.binary.bytes}, 'The smoke ran another service binary');
  const evidence = {};
  for (const n of ['result.json', 'sandbox.json', 'service.json', 'single-instance.json', 'current.png']) { const f = path.join(a['--evidence'], n); evidence[n] = pin(f); assert(evidence[n].bytes > 0, n); }
  assert(fs.readFileSync(path.join(a['--evidence'], 'current.png')).subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])), 'current.png is no PNG');
  assert(!fs.existsSync(a['--out']) && !fs.existsSync(a['--dist']), 'Acceptance/dist output already exists');
  fs.mkdirSync(a['--out']);
  const common = {schema: 1, status: 'PASS', target: stage.target, version: stage.release.version, commit: stage.release.commit, tree: stage.release.tree,
    payloadManifestSha256: inspected.payloadManifestSha256, freezeReceiptSha256: stage.release.freezeReceipt.sha256, evidence, fixtureSha256: r.fixture.sha256,
    deferredByUser: r.deferredByUser ?? [], reconnectMode: r.reconnectMode};
  const proofs = {
    signing: {kind: 'desktop-signing', signed: false, policy: 'unsigned-portable-allowed', scope: 'Unsigned portable disposition satisfied; no code-signing or notarization claim.'},
    sandbox: {kind: 'desktop-sandbox', osEnforced: true, flags: sandbox.argv, kernel: sandbox.kernel ?? null, metric: sandbox.metric ?? null},
    lifecycle: {kind: 'desktop-lifecycle', startup: true, nativeChildIdentity: true, authBound: true, singleInstance: true, reconnect: true, windowClose: 'resident-service',
      closePreservesService: true, shutdown: 'unsupported', managedStopAdvertised: false, managedUpdateAdvertised: false,
      ...(stage.target === 'windows-x64' ? {nativeToPosixPaths: service.nativePathWithSpacesAndUnicode.includes('日本語') && service.nativePathWithSpacesAndUnicode.includes(' ')} : {})},
    runtimeAcceptance: {kind: 'desktop-runtime-acceptance', scope: 'desktop-release',
      acceptanceScope: 'Essential matching-native-host startup/auth/current-content/sandbox/single-instance/socket reconnect (' + r.reconnectMode + ')/normal-close-resident only. No full UI parity, telemetry, Send/provider, scale, IME or device qualification.',
      checks: r.checks}};
  const acceptance = {schema: 1, kind: 'backplane-desktop-target-acceptance', target: stage.target, stageReceiptSha256: inspected.stageReceiptSha256, payloadManifestSha256: inspected.payloadManifestSha256};
  for (const [k, v] of Object.entries(proofs)) { const f = path.join(a['--out'], k + '.json'); write(f, {...common, ...v}); acceptance[k] = {path: f, ...pin(f)}; }
  const file = path.join(a['--out'], 'ACCEPTANCE.json'); write(file, acceptance);
  const result = p.archive(a['--stage'], file, a['--dist']);
  write(path.join(a['--out'], 'ARCHIVE-RESULT.json'), result);
  console.log(JSON.stringify(result));
}
if (require.main === module) try { main(process.argv.slice(2)); } catch (e) { console.error('Archive after smoke: ' + e.message); process.exitCode = 1; }
module.exports = {main, REQUIRED};
