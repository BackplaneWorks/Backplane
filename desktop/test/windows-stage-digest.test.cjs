'use strict';
// A windows-x64 stage made on Linux must inspect to the same payload digest on
// the Windows host it is smoked on, where Node reports every regular file as
// 0666 or 0444 (J4 session 5: "Stage payload changed").
const {test} = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), os = require('node:os'), path = require('node:path');
const p = require('../produce-release.cjs');

function stageDir(modes) {
  const d = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-windigest-'));
  fs.writeFileSync(path.join(d, 'STAGE.json'), JSON.stringify({kind: 'backplane-electron-desktop-stage', target: 'windows-x64', release: {version: '0.12.1'}}));
  for (const [name, mode] of Object.entries(modes)) { fs.mkdirSync(path.dirname(path.join(d, name)), {recursive: true}); fs.writeFileSync(path.join(d, name), name); fs.chmodSync(path.join(d, name), mode); }
  return d;
}

test('a Windows stage digests the same with POSIX modes and with NTFS modes', () => {
  const linux = stageDir({'Backplane.exe': 0o755, 'resources/app/main.cjs': 0o644, 'resources/ro.txt': 0o444});
  const ntfs = stageDir({'Backplane.exe': 0o666, 'resources/app/main.cjs': 0o666, 'resources/ro.txt': 0o444});
  assert.equal(p.inspectStage(linux).payloadManifestSha256, p.inspectStage(ntfs).payloadManifestSha256);
});

test('a byte change still changes it', () => {
  const a = stageDir({'Backplane.exe': 0o755}), b = stageDir({'Backplane.exe': 0o755});
  fs.writeFileSync(path.join(b, 'Backplane.exe'), 'changed');
  assert.notEqual(p.inspectStage(a).payloadManifestSha256, p.inspectStage(b).payloadManifestSha256);
});
