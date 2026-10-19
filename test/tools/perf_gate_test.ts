import assert from 'node:assert/strict';
import { performanceVerdict } from './perf_gate';
const common={'all agents streaming':true,'frame p99<=8ms':true,'no frame>16ms':true,'agent_line p99<=2ms':true};
const good=[
 {name:'bg',checks:{...common}}, {name:'fg',checks:{...common}},
 {name:'type',checks:{...common,'input p99<=16ms':true,'key>frame p99<=16ms':true}},
 {name:'open',checks:{...common,'all ten opens sampled':true,'open<=50ms':true}},
 {name:'scroll',checks:{...common}}, {name:'plane',checks:{...common,'Status tab observed':true}},
 {name:'normal',checks:{...common,'Status tab observed':true,'input p99<=16ms':true,'key>frame p99<=16ms':true}},
];
assert.equal(performanceVerdict(good,true).accepted,true);
for(let i=0;i<good.length;i++) {
 assert.equal(performanceVerdict(good.filter((_,at)=>at!==i),true).exitCode,1,'each required phase must be present');
 for(const name of Object.keys(good[i].checks)) {
  const omitted=structuredClone(good);delete (omitted[i].checks as any)[name];
  assert.equal(performanceVerdict(omitted,true).exitCode,1,`${good[i].name}/${name} cannot be silently omitted`);
 }
}
for (const phases of [[],[{name:'idle',checks:{}}],[{name:'scroll',checks:{'frame p99<=8ms':false}}],
 [{name:'scroll',checks:{'presentations observed':null}}],[{name:'everything',checks:{'all green':true}}],
 [...good,good[0]]]) {
 const result=performanceVerdict(phases,true);assert.equal(result.accepted,false);assert.equal(result.exitCode,1);
}
assert.equal(performanceVerdict(good,true,false,'unknown' as any).accepted,false);
assert.equal(performanceVerdict(good,false).exitCode,1);
const historical=[{name:'mail',checks:{'frame p99<=8ms':54.741<=8,'agent_line p99<=2ms':4.104<=2}}];
assert.equal(performanceVerdict(historical,true).exitCode,1,'retained failing timings must fail without an opt-in flag');
assert.equal(performanceVerdict(historical,true,true).accepted,false,'diagnostic mode never grants acceptance');
assert.equal(performanceVerdict(historical,true,true).exitCode,0);
assert.equal(performanceVerdict(good,true,true).accepted,false,'even a green diagnostic is not acceptance');
assert.equal(performanceVerdict(good,false,true).exitCode,1,'diagnostic cannot excuse stray or dead agents');
console.log('PASS enforced fixed-schema performance verdict: omitted/unknown/duplicate/failed phases or checks cannot accept');
