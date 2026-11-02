import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
const lib=(await import(pathToFileURL(process.argv[2]).href)).default;
const rows=JSON.parse(lib.native());
const chart=rows.find(r=>r.chart)?.chart;
assert.equal(chart.length,30,'native graph follows UTC period');
assert.equal(chart[0].known,false,'missing native day remains unknown');
assert.equal(chart.find(d=>d.date==='2026-10-15').reading.startsWith('100 recorded tokens'),true,'native reading uses actual provider rollup');
assert.equal(chart.find(d=>d.date==='2026-10-16').fraction,10000,'native bar scale uses maximum recorded day');
assert.deepEqual(rows.find(r=>r.label==='Provider').buttons.map(b=>b.label),['claude','codex','grok'],'native provider controls follow availability');
assert.equal(lib.scope(),'claude/codex','native control changes only named machine');
assert.equal(lib.missing_scope(),'claude','obsolete machine cannot change focused machine');
console.log('ok native usage chart payload, UTC, availability and machine isolation (7 checks; no native binary build)');

for (const [period, days] of [['week',7],['2weeks',14],['4weeks',28],['30days',30]]) {
  const rolling=JSON.parse(lib.rolling(period));
  assert.equal(rolling.find(r=>r.chart).chart.length,days,'native rolling '+period);
  const controls=rolling.find(r=>r.label==='Period').buttons;
  assert.equal(controls.length,6,'native has rolling and calendar controls');
  assert.equal(rolling.find(r=>r.label==='View').buttons.length,3,'metrics remain grouped separately');
}
assert.equal(lib.period_scope(),'30days/2weeks','rolling selection stays on the named machine');
console.log('ok native rolling ranges, controls and machine isolation');
