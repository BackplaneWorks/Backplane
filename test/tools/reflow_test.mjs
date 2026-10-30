// Run against the actual emitted pure planner: bend src/core/reflow.bend -o build/viewer/reflow.mjs
// bun test/tools/reflow_test.mjs build/viewer/reflow.mjs
import assert from 'node:assert/strict';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const {default:R}=await import(pathToFileURL(resolve(process.argv[2]||'build/viewer/reflow.mjs')));
const run=(points,alloy='SAC305')=>JSON.parse(R.plan(typeof points==='string'?points:JSON.stringify(points),alloy));
const base=run(R.defaults());
assert.equal(base.duration,330);assert.equal(base.samples.length,331);assert.deepEqual(base.samples[0],[0,25,25,0]);
assert.equal(base.samples.at(-1)[0],330);assert.equal(base.samples.at(-1)[1],25);assert(base.samples.at(-1)[2]>25);
assert.equal(base.solidus,217);assert.equal(base.liquidus,220);
assert(base.samples.some(s=>s[3]===1));assert(base.samples.some(s=>s[3]>0&&s[3]<1));
for(const [i,s] of base.samples.entries()){assert.equal(s[0],i);assert(s.every(Number.isFinite));assert(s[3]>=0&&s[3]<=1);}
const steady=run([[0,25],[300,25]]);assert(steady.samples.every(s=>s[1]===25&&s[2]===25&&s[3]===0));
const hot=run([[0,245],[60,245]]);assert(hot.samples.every(s=>s[1]===245&&s[2]===245&&s[3]===1));
const halfway=run([[0,218.5],[1,218.5]]);assert.equal(halfway.samples[0][3],.5);
for(const alloy of ['Sn63Pb37','Sn42Bi58']){const p=run([[0,25],[1,25]],alloy);assert.equal(p.solidus,p.liquidus);assert(p.samples.every(s=>s[3]===0));}
for(const invalid of [[],[[0,25]],[[1,25],[10,30]],[[0,25],[0,30]],[[0,25],[10,30],[9,20]],[[0,25],[1201,30]],[[0,-1],[10,30]],[[0,25],[10,351]],[[0,25],[1.5,30]],[[0,25],[null,30]],[[0,25],[10,'30']],[[0,25],[10,30,40]],Array.from({length:33},(_,i)=>[i,25]),'not JSON']) assert(run(invalid).error,JSON.stringify(invalid));
assert(run([[0,25],[10,30]],'unknown').error);
assert.equal(run([[0,25],[1200,30]]).samples.length,1201);
console.log('ok actual Bend reflow planner: bounded profiles, lag, phase boundaries, endpoint, three sourced alloys, malformed input');
