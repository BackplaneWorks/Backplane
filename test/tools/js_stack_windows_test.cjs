'use strict';
// Native Windows regression for #420. Compile only a tiny fixture with the
// real prelude, never the hub; no home, sockets, installed binaries or updates.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

test('compiled Windows startup raises JSC before deep work and preserves the command', { skip: process.platform !== 'win32' }, () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bp stack 日本 '));
  try {
    const source = path.join(dir, 'fixture.cjs');
    const binary = path.join(dir, 'backplane-serve.exe');
    fs.writeFileSync(source, fs.readFileSync(path.join(__dirname, '../../scripts/js-stack-prelude.js'), 'utf8') + `
function deep(n) { return n === 0 ? 0 : 1 + deep(n - 1); }
console.log(JSON.stringify({bound: process.env.BUN_JSC_maxPerThreadStackUsage,
  args: process.argv.slice(2), cwd: process.cwd(), depth: deep(100000)}));
console.error('fixture stderr');
process.exit(Number(process.env.BP_STACK_FIXTURE_EXIT || 0));
`);
    const built = spawnSync('bun', ['build', '--compile', source, '--outfile', binary], { encoding: 'utf8', timeout: 60000 });
    assert.equal(built.status, 0, built.stderr || String(built.error));
    const env = { ...process.env, BACKPLANE_STACK_SET: String(process.pid) };
    delete env.BUN_JSC_maxPerThreadStackUsage;
    for (const [args, status] of [[['--plot', '0', 'C:\\user data 日本\\board.kicad_sch'], 0], [['--no-such-flag'], 2]]) {
      const run = spawnSync(binary, args, { cwd: dir, env: { ...env, BP_STACK_FIXTURE_EXIT: String(status) }, encoding: 'utf8', timeout: 30000 });
      assert.equal(run.status, status, run.stderr || String(run.error));
      const result = JSON.parse(run.stdout);
      assert.equal(result.bound, '16000000');
      assert.equal(result.depth, 100000);
      assert.deepEqual(result.args, args);
      assert.equal(result.cwd, dir);
      assert.match(run.stderr, /fixture stderr/);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
