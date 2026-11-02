// Times a thread's subagent rows (test/subagent_rows_cost_test.bend) over
// the same eight subagents with short and with 32x longer lines on the steps
// no row shows (only the newest Subs.shown() of each are on screen), so the
// rows must not cost more for them (they did: every step's first line was
// taken, then all but six dropped).
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';
const lib=(await import(pathToFileURL(process.argv[2]).href)).default;
const time=(st)=>{let best=Infinity,out='';for(let i=0;i<5;i++){const t=performance.now();out=lib.rows(st);best=Math.min(best,performance.now()-t);}return {best,out};};
const short=lib.state(50n), long=lib.state(1600n);
time(short);
const a=time(short), b=time(long);
const lines=a.out.split('\n');
assert.equal(lines.length, 9, 'eight subagent rows and the finished group: '+a.out.slice(0,200));
assert.ok(lines.some(l=>/^c1\|Review c1\|done\|399\|Step 6 x,Step 5 x,Step 4 x,Step 3 x,Step 2 x,Step 1 x$/.test(l.replace(/x+/g,'x'))), 'a row keeps its newest six steps: '+a.out.slice(0,400));
assert.equal(a.out.replace(/x+/g,'x'), b.out.replace(/x+/g,'x'), 'the same rows for either length');
const ratio=b.best/a.best;
console.log(JSON.stringify({shortMs:+a.best.toFixed(2), longMs:+b.best.toFixed(2), ratio:+ratio.toFixed(2)}));
assert.ok(ratio<2, `rows over 32x longer step lines took ${ratio.toFixed(1)}x as long (${a.best.toFixed(1)} ms -> ${b.best.toFixed(1)} ms)`);
console.log(JSON.stringify({pass:true}));
