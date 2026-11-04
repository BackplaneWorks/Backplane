// Synthetic processes only. No hub, store, auth, network or fixed-delay phase switch.
const fs = require('node:fs'), { join, resolve } = require('node:path'), { tmpdir } = require('node:os');
const assert = require('node:assert/strict');
const now = () => Number(process.hrtime.bigint()) / 1e6, sleep = ms => new Promise(r => setTimeout(r, ms));
// Readiness requires independently observed output and the producer's receipt.
function mixedReady(rows, receipt) {
  const s = receipt?.states?.find(s => s.epoch === 1);
  return !!s && s.mode === 'mixed' && s.count >= 20 && s.completed >= 1 && s.tools >= 1 && s.results >= 1 &&
    rows.some(r => r.type === 'assistant' && r.message?.content?.some(c => c.type === 'text')) &&
    rows.some(r => r.message?.content?.some(c => c.type === 'tool_use')) &&
    rows.some(r => r.message?.content?.some(c => c.type === 'tool_result'));
}
module.exports.run = async function run(source) {
  let cases = 0;
  const pass = (v, label) => { assert(v, label); cases++; };
  pass(!mixedReady([], null), 'missing output and receipt cannot be ready');
  const forged = { states: [{ epoch: 1, mode: 'mixed', count: 40, completed: 1, tools: 1, results: 1 }] };
  pass(!mixedReady([], forged), 'receipt counters alone cannot qualify actual output');
  const fakeRows = [{ type: 'assistant', message: { content: [{ type: 'text' }, { type: 'tool_use' }, { type: 'tool_result' }] } }];
  pass(!mixedReady(fakeRows, { states: [{ epoch: 1, mode: 'mixed', count: 40 }] }), 'output alone cannot qualify missing receipt counters');
  const spans = [];
  for (const suppressed of [false, true]) {
    const root = fs.mkdtempSync(join(tmpdir(), 'bp-real-active-producer-control-')); fs.chmodSync(root, 0o700);
    const file = join(root, 'producer.cjs'), run = '012345abcdef', id = 'ra-012345abcdef-0';
    const receiptPath = join(root, 'producer-0.json'); let child, finished = false, parseFault = null, bytes = 0, pending = '';
    const rows = [];
    const control = (epoch, mode) => {
      const start = now(); fs.writeFileSync(join(root, 'control.next'), JSON.stringify({ epoch, mode, start, end: start + 10000 }));
      fs.renameSync(join(root, 'control.next'), join(root, 'control.json'));
    };
    const receipt = () => fs.existsSync(receiptPath) ? JSON.parse(fs.readFileSync(receiptPath, 'utf8')) : null;
    async function collect(stream, stdout) {
      const decoder = new TextDecoder(); let errors = '';
      for await (const b of stream) {
        bytes += b.length; assert(bytes <= 512 * 1024, 'stand-in output bound');
        if (!stdout) { errors += decoder.decode(b, { stream: true }); continue; }
        pending += decoder.decode(b, { stream: true });
        for (;;) { const at = pending.indexOf('\n'); if (at < 0) break;
          rows.push(JSON.parse(pending.slice(0, at))); pending = pending.slice(at + 1); assert(rows.length <= 1024); }
      }
      if (stdout) assert.equal(pending, '', 'complete output lines'); return errors;
    }
    async function wait(check, ms) {
      const end = now() + ms;
      while (!check()) { assert(!parseFault && child.exitCode === null, 'stand-in remains alive'); assert(now() < end, 'observed readiness deadline'); await sleep(10); }
    }
    try {
      let input = source;
      if (suppressed) {
        const gate = "if (c.mode === 'mixed' && at - cycle >= 500)";
        assert.equal(input.split(gate).length, 2, 'negative changes only the cycle gate'); input = input.replace(gate, 'if (false)');
      }
      fs.writeFileSync(file, input); control(1, 'mixed');
      child = Bun.spawn([process.execPath, file], { cwd: root,
        env: { HOME: root, REAL_ACTIVE_ROOT: root, REAL_ACTIVE_RUN: run, REAL_ACTIVE_IDS: JSON.stringify([id]), REAL_ACTIVE_POLICY: resolve(__dirname, 'real_store_active_policy.cjs') },
        stdin: 'pipe', stdout: 'pipe', stderr: 'pipe' });
      const output = collect(child.stdout, true), errors = collect(child.stderr, false);
      for (const p of [output, errors]) p.catch(e => { parseFault = e; if (child.exitCode === null) child.kill('SIGKILL'); });
      child.stdin.write(JSON.stringify({ type: 'user', message: { content: 'real-active-thread=' + id } }) + '\n');
      if (suppressed) {
        await wait(() => receipt()?.states?.[0]?.count >= 32, 5000);
        pass(!mixedReady(rows, receipt()), 'suppressed cycle cannot pass despite fresh deltas');
        pass(!rows.some(r => r.message?.content?.some(c => c.type === 'tool_use')), 'suppressed cycle actually emits no tool');
      } else {
        // The first packet used a 650ms sleep and observed no tool cycle. Its
        // cause is unknown. This control waits for actual output AND counters.
        await wait(() => mixedReady(rows, receipt()), 5000); cases++;
        control(2, 'pure');
        await wait(() => receipt()?.states?.find(s => s.epoch === 2)?.count >= 10, 3000);
      }
      control(3, 'stop');
      const timer = setTimeout(() => { if (child.exitCode === null) child.kill('SIGKILL'); }, 2000);
      try { assert.equal(await child.exited, 0); } finally { clearTimeout(timer); }
      finished = true; await output; assert.equal(await errors, ''); assert(!parseFault);
      const deltas = rows.filter(r => r.event?.type === 'content_block_delta').map(r => r.event.delta.text);
      pass(deltas.length >= 30 && deltas.every(t => Buffer.byteLength(t) === 96), 'actual deltas have correct bytes/count');
      const marks = deltas.map(t => /~ra:012345abcdef:0:(\d+):(\d+):(\d+):(mixed|pure)~/.exec(t));
      pass(marks.every((m, i) => m && Number(m[1]) === i + 1), 'sequence is continuous');
      if (!suppressed) {
        const firstPure = rows.findIndex(r => r.event?.delta?.text?.includes(':2:pure~'));
        pass(firstPure >= 0 && rows.slice(firstPure).every(r => r.type === 'stream_event' && r.event.type === 'content_block_delta'), 'pure mode remains static pending');
      }
      const r = receipt(); pass(r.states.length === (suppressed ? 1 : 2), 'exact receipt phases');
      for (const s of r.states) {
        const count = marks.filter(m => Number(m[3]) === s.epoch).length;
        pass(s.count === count && s.bytes === count * 96 && s.buckets.reduce((a, b) => a + b, 0) === count && s.last >= s.first,
          'actual receipt matches observed emission cohort');
      }
      spans.push({ suppressedCycle: suppressed, phases: r.states.map(s => ({ epoch: s.epoch, mode: s.mode,
        emitted: s.count, spanMs: s.last - s.first, completed: s.completed, tools: s.tools, results: s.results })) });
    } catch (e) {
      // Aggregate diagnostics only; no body, input text, argv or private path.
      console.error(JSON.stringify({ scope: 'stand-in-control-failure', suppressedCycle: suppressed, outputRows: rows.length,
        completed: rows.filter(r => r.type === 'assistant' && r.message?.content?.some(c => c.type === 'text')).length,
        tools: rows.filter(r => r.message?.content?.some(c => c.type === 'tool_use')).length,
        results: rows.filter(r => r.message?.content?.some(c => c.type === 'tool_result')).length,
        states: receipt()?.states?.map(s => ({ epoch: s.epoch, count: s.count, spanMs: s.last - s.first, completed: s.completed, tools: s.tools, results: s.results })) }));
      throw e;
    } finally {
      if (child && !finished && child.exitCode === null) { child.kill('SIGKILL'); await child.exited; }
      fs.rmSync(root, { recursive: true, force: true });
    }
  }
  assert.equal(cases, 16);
  console.log(JSON.stringify({ scope: 'combined-real-store-actual-stand-in-control', ok: true, cases,
    hubStarted: false, syntheticProducers: 2, phasesChecked: 2, suppressedCycleRejected: true, bytesPerDelta: 96, spans }));
};
