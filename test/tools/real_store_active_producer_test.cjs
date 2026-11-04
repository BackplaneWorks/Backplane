// Called only by the explicit Boat --self-test path. One synthetic stand-in,
// no hub, real fixture, providers, external network or inherited credentials.
const fs = require('node:fs'), { join, resolve } = require('node:path'), { tmpdir } = require('node:os');
const assert = require('node:assert/strict');
module.exports.run = async function run(source) {
  const root = fs.mkdtempSync(join(tmpdir(), 'bp-real-active-producer-control-')); fs.chmodSync(root, 0o700);
  const now = () => Number(process.hrtime.bigint()) / 1e6, sleep = ms => new Promise(r => setTimeout(r, ms));
  const file = join(root, 'producer.cjs'), run = '012345abcdef', id = 'ra-012345abcdef-0';
  let child, finished = false;
  const control = (epoch, mode) => {
    const start = now(); fs.writeFileSync(join(root, 'control.next'), JSON.stringify({ epoch, mode, start, end: start + 10000 }));
    fs.renameSync(join(root, 'control.next'), join(root, 'control.json'));
  };
  async function bounded(stream) {
    const chunks = []; let size = 0;
    for await (const b of stream) { size += b.length; assert(size <= 512 * 1024, 'stand-in control output bounded'); chunks.push(Buffer.from(b)); }
    return Buffer.concat(chunks).toString('utf8');
  }
  try {
    fs.writeFileSync(file, source); control(1, 'mixed');
    child = Bun.spawn([process.execPath, file], { cwd: root,
      env: { HOME: root, REAL_ACTIVE_ROOT: root, REAL_ACTIVE_RUN: run, REAL_ACTIVE_IDS: JSON.stringify([id]), REAL_ACTIVE_POLICY: resolve(__dirname, 'real_store_active_policy.cjs') },
      stdin: 'pipe', stdout: 'pipe', stderr: 'pipe' });
    const output = bounded(child.stdout), errors = bounded(child.stderr);
    // Rejections are observed immediately, even if the stand-in never closes.
    output.catch(() => { if (child.exitCode === null) child.kill('SIGKILL'); });
    errors.catch(() => { if (child.exitCode === null) child.kill('SIGKILL'); });
    child.stdin.write(JSON.stringify({ type: 'user', message: { content: 'real-active-thread=' + id } }) + '\n');
    const deadline = now() + 3000, receiptPath = join(root, 'producer-0.json');
    while (!fs.existsSync(receiptPath) && now() < deadline && child.exitCode === null) await sleep(10);
    assert(fs.existsSync(receiptPath), 'actual stand-in creates its own receipt');
    await sleep(650); control(2, 'pure'); await sleep(450); control(3, 'stop');
    const timer = setTimeout(() => { if (child.exitCode === null) child.kill('SIGKILL'); }, 2000);
    try { assert.equal(await child.exited, 0); } finally { clearTimeout(timer); }
    finished = true;
    const rows = (await output).trim().split('\n').map(x => JSON.parse(x));
    assert.equal((await errors).trim(), '');
    const deltas = rows.filter(r => r.event?.type === 'content_block_delta').map(r => r.event.delta.text);
    assert(deltas.length > 20); assert(deltas.every(t => Buffer.byteLength(t) === 96));
    const marks = deltas.map(t => /~ra:012345abcdef:0:(\d+):(\d+):(\d+):(mixed|pure)~/.exec(t));
    assert(marks.every((m, i) => m && Number(m[1]) === i + 1));
    assert(marks.some(m => m[3] === '1' && m[4] === 'mixed')); assert(marks.some(m => m[3] === '2' && m[4] === 'pure'));
    const firstPure = rows.findIndex(r => r.event?.delta?.text?.includes(':2:pure~'));
    assert(firstPure >= 0 && rows.slice(firstPure).every(r => r.type === 'stream_event' && r.event.type === 'content_block_delta'), 'pure stream never commits partial text or tool cycles');
    assert(rows.some(r => r.message?.content?.some(c => c.type === 'tool_use')), 'mixed stream commits real synthetic tool cycles');
    const receipt = JSON.parse(fs.readFileSync(receiptPath, 'utf8')); assert.equal(receipt.states.length, 2);
    for (const state of receipt.states) {
      const count = marks.filter(m => Number(m[3]) === state.epoch).length;
      assert.equal(state.count, count); assert.equal(state.bytes, count * 96);
      assert.equal(state.buckets.reduce((a, b) => a + b, 0), count);
    }
    console.log(JSON.stringify({ scope: 'combined-real-store-actual-stand-in-control', ok: true,
      hubStarted: false, syntheticProducers: 1, phasesChecked: 2, bytesPerDelta: 96 }));
  } finally {
    if (child && !finished && child.exitCode === null) { child.kill('SIGKILL'); await child.exited; }
    fs.rmSync(root, { recursive: true, force: true });
  }
};
