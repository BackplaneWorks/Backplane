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
    // Diagnostics retain two epoch aggregates and three control publications,
    // never a per-event log. They do not change clocks, phases or assertions.
    const controls = [], received = [{ count: 0, first: null, last: null }, { count: 0, first: null, last: null }];
    const clockSamples = [];
    const clockSample = stage => { if (clockSamples.length < 4) clockSamples.push({ stage,
      monoMs: now(), performanceMs: performance.now(), performanceOriginMs: performance.timeOrigin, wallMs: Date.now() }); };
    const control = (epoch, mode) => {
      const start = now(); fs.writeFileSync(join(root, 'control.next'), JSON.stringify({ epoch, mode, start, end: start + 10000 }));
      fs.renameSync(join(root, 'control.next'), join(root, 'control.json'));
      if (controls.length < 3) controls.push({ epoch, mode, start, end: start + 10000, publishedMonoMs: now() });
    };
    const receipt = () => fs.existsSync(receiptPath) ? JSON.parse(fs.readFileSync(receiptPath, 'utf8')) : null;
    function diagnostic() {
      let r = null, receiptReadFailed = false;
      try { r = receipt(); } catch { receiptReadFailed = true; }
      let unparsedDeltas = 0, unknownEpochMarkers = 0;
      const observed = [1, 2].map(epoch => ({ epoch, count: 0, utf8Bytes: 0, firstSeq: null, lastSeq: null,
        minSeq: null, maxSeq: null, minTimestampMs: null, maxTimestampMs: null,
        definitelyBeforeWindow: 0, definitelyAfterWindow: 0, boundaryFloorUncertain: 0 }));
      for (const row of rows) {
        if (row.event?.type !== 'content_block_delta') continue;
        const text = row.event.delta?.text, m = typeof text === 'string' && /~ra:012345abcdef:0:(\d+):(\d+):(\d+):(mixed|pure)~/.exec(text);
        if (!m) { unparsedDeltas++; continue; }
        const epoch = Number(m[3]), s = observed[epoch - 1];
        if (!s || epoch < 1 || epoch > 2) { unknownEpochMarkers++; continue; }
        const seq = Number(m[1]), at = Number(m[2]), c = controls.find(c => c.epoch === epoch);
        s.count++; s.utf8Bytes += Buffer.byteLength(text); s.firstSeq ??= seq; s.lastSeq = seq;
        s.minSeq = s.minSeq === null ? seq : Math.min(s.minSeq, seq); s.maxSeq = s.maxSeq === null ? seq : Math.max(s.maxSeq, seq);
        s.minTimestampMs = s.minTimestampMs === null ? at : Math.min(s.minTimestampMs, at);
        s.maxTimestampMs = s.maxTimestampMs === null ? at : Math.max(s.maxTimestampMs, at);
        // Markers floor the producer's unchanged clock. Preserve its <1ms
        // uncertainty instead of guessing which side of a fractional edge it is.
        if (c) { if (at + 1 <= c.start) s.definitelyBeforeWindow++;
          else if (at >= c.end) s.definitelyAfterWindow++;
          else if (at < c.start) s.boundaryFloorUncertain++; }
      }
      return { scope: 'stand-in-cohort-diagnostic', suppressedCycle: suppressed, assertionsCompleted: cases,
        hubStarted: false, outputRows: rows.length, unparsedDeltas, unknownEpochMarkers, controls, clockSamples,
        controllerSnapshotMonoMs: now(), producerReceiptAtMs: r?.at ?? null, receiptReadFailed,
        epochs: observed.map((o, i) => {
          const s = r?.states?.find(s => s.epoch === o.epoch), bucketSum = Array.isArray(s?.buckets) ? s.buckets.reduce((a, b) => a + b, 0) : null;
          return { observed: o, received: received[i], receipt: s ? { epoch: s.epoch, mode: s.mode,
            count: s.count, bytes: s.bytes, bucketSum, first: s.first, last: s.last, firstSeq: s.firstSeq, lastSeq: s.lastSeq } : null,
            comparisons: { receiptPresent: !!s, countEqualsObserved: !!s && s.count === o.count,
              bytesEqualObservedUTF8: !!s && s.bytes === o.utf8Bytes, bytesEqualObservedCountTimes96: !!s && s.bytes === o.count * 96,
              bucketSumEqualsObserved: !!s && bucketSum === o.count, bucketSumEqualsReceipt: !!s && bucketSum === s.count,
              orderedReceiptTimes: !!s && s.last >= s.first,
              firstSeqEqualsObserved: !!s && s.firstSeq === o.minSeq, lastSeqEqualsObserved: !!s && s.lastSeq === o.maxSeq,
              firstTimeMatchesFlooredMarker: !!s && o.minTimestampMs !== null && s.first >= o.minTimestampMs && s.first < o.minTimestampMs + 1,
              lastTimeMatchesFlooredMarker: !!s && o.maxTimestampMs !== null && s.last >= o.maxTimestampMs && s.last < o.maxTimestampMs + 1 } };
        }) };
    }
    async function collect(stream, stdout) {
      const decoder = new TextDecoder(); let errors = '';
      for await (const b of stream) {
        bytes += b.length; assert(bytes <= 512 * 1024, 'stand-in output bound');
        if (!stdout) { errors += decoder.decode(b, { stream: true }); continue; }
        pending += decoder.decode(b, { stream: true });
        for (;;) { const at = pending.indexOf('\n'); if (at < 0) break;
          const row = JSON.parse(pending.slice(0, at)); rows.push(row); pending = pending.slice(at + 1); assert(rows.length <= 1024);
          const m = row.event?.type === 'content_block_delta' && /~ra:012345abcdef:0:(\d+):(\d+):(\d+):(mixed|pure)~/.exec(row.event.delta?.text ?? '');
          if (m && (m[3] === '1' || m[3] === '2')) { const s = received[Number(m[3]) - 1], at = now(); s.count++; s.first ??= at; s.last = at; } }
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
      clockSample('before-spawn');
      child = Bun.spawn([process.execPath, file], { cwd: root,
        env: { HOME: root, REAL_ACTIVE_ROOT: root, REAL_ACTIVE_RUN: run, REAL_ACTIVE_IDS: JSON.stringify([id]), REAL_ACTIVE_POLICY: resolve(__dirname, 'real_store_active_policy.cjs') },
        stdin: 'pipe', stdout: 'pipe', stderr: 'pipe' });
      clockSample('after-spawn');
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
      clockSample('stop-published');
      const timer = setTimeout(() => { if (child.exitCode === null) child.kill('SIGKILL'); }, 2000);
      try { assert.equal(await child.exited, 0); } finally { clearTimeout(timer); }
      finished = true; await output; assert.equal(await errors, ''); assert(!parseFault);
      clockSample('child-exited'); console.log(JSON.stringify(diagnostic()));
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
      console.error(JSON.stringify(diagnostic()));
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
