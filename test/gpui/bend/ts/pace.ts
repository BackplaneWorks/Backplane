// Plain-TS mirror of native/bend/pace.bend: same generator, same hash, same
// views. Uses the repo's reviewed TS equivalents (test/perf/bend-vs-ts).
import * as ts from '../../../perf/bend-vs-ts/equivalents';
const [nArg = '200000', repsArg = '5'] = process.argv.slice(2);
const n = Number(nArg), reps = Number(repsArg), M = 4294967291;
const h1 = (x: number) => (x * 48271) % 2147483647;
const h = (i: number) => h1(h1(i + 12345));
function ev(i: number): ts.Change {
  const r = h(i), k = Math.floor(r / 16) % 12;
  const th = 't' + (Math.floor(r / 1024) % 64), other = 't' + (Math.floor(r / 65536) % 64);
  switch (k) {
    case 0: return { $: 'MessagePosted', thread: th, msg: r % 5 === 0 ? 't:' + other + ':' + i : 'm' + i };
    case 1: return { $: 'TurnChanged', thread: th };
    case 2: return { $: 'ActivityLogged', thread: th };
    case 3: return { $: 'SettingSet', key: 'liveness.' + th };
    case 4: return { $: 'SettingSet', key: 'autosettle.quiet.' + th };
    case 5: return { $: 'SettingSet', key: 'launch.' + i };
    case 6: return { $: 'SettingSet', key: 'ui.theme' };
    case 7: return { $: 'TaskDelegated', parent: th, child: other };
    case 8: return { $: 'PendingChanged', thread: th };
    case 9: return { $: 'ThreadForked', id: th, source: other };
    case 10: return { $: 'ThreadSettled', id: th };
    default: return { $: 'RoomPosted' };
  }
}
let t0 = performance.now();
const evs: ts.Change[] = []; for (let i = 0; i < n; i++) evs.push(ev(i));
console.log('events ' + evs.length); console.log('gen_ms ' + Math.round(performance.now() - t0));
function run(): number[] {
  const ps = [0, 1, 2, 3].map(() => ({ last: 0, owed: 0 })), acc = [0, 0, 0, 0];
  const ths = ['t0', 't1', 't2', 't3'], cids = ['1', '2', '3', '4'];
  for (let i = 0; i < evs.length; i++) {
    const ids = ts.touched(evs[i]), now = i * 13;
    for (let v = 0; v < 4; v++) {
      const level = ts.level(ids, ths[v], cids[v]);
      const p = ts.pace(ps[v], now, level); ps[v] = p.pace;
      acc[v] = (acc[v] * 31337 + (level + 3 * (p.send ? 1 : 0) + 7 * p.wait + 11 * p.pace.last + 13 * p.pace.owed)) % M;
    }
  }
  return acc;
}
for (let r = 0; r < reps; r++) {
  t0 = performance.now(); const a = run(); const ms = performance.now() - t0;
  console.log('digest ' + a.join(' ')); console.log('ms ' + Math.round(ms * 1000) / 1000);
}
