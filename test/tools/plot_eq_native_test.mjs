// Read the built server's actual optimized comparator without starting a hub.
// bun test/tools/plot_eq_native_test.mjs build/js/backplane-serve/stamped.js
import {readFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const source=readFileSync(process.argv[2], 'utf8');
const match=source.match(/function ([\w$]+\$058Eq\$str\$)\(_a_0,_b_0\)\{return _a_0===_b_0\}/);
assert.ok(match, 'exact optimized Eq.str must occur in the actual bundle');
const eq=new Function(match[0]+';return '+match[1])();
function reference(a,b) {
 const x=Array.from(a),y=Array.from(b);
 return x.length===y.length && x.every((c,i)=>c.codePointAt(0)===y[i].codePointAt(0));
}
const atoms=['','a','b','\0','é','中','😀','\ud800','\udc00','e\u0301'];
let seed=73127;
function next(){seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed;}
let checks=0;
for(let i=0;i<2000;i++){
 const make=()=>Array.from({length:next()%30},()=>atoms[next()%atoms.length]).join('');
 const a=make(),b=i%2?a:make();
 assert.equal(eq(a,b),reference(a,b));checks++;
}
const large='abc\0😀'.repeat(400000);
for(const [a,b,want] of [[large,large,true],[large,large.slice(0,-1),false],[large,large+'x',false],['x'+large,'y'+large,false],[large+'x',large+'y',false]]){
 assert.equal(eq(a,b),want);checks++;
}
assert.equal(eq('é','e\u0301'),false);checks++;
// The same emitted pure comparator is the negative control, not a mock.
const raw=readFileSync(process.argv[3] || 'build/js/backplane-serve/all.js', 'utf8');
const start=raw.search(/function [\w$]+\$058Eq\$str\$\(/);
assert.ok(start>=0);
const end=raw.indexOf('\nfunction ',start+10);
const body=raw.slice(start,end);
const name=/function ([\w$]+)\(/.exec(body)[1];
const chr=name.replace('$058Eq$str$', '$058Eq$chr$');
const original=new Function('function $Bool$and$(a,b){return a&&b}function '+chr+'(a,b){return a.codePointAt(0)===b.codePointAt(0)}'+body+';return '+name)();
assert.equal(original('abc😀','abc😀'),true);
assert.throws(()=>original(large,large),RangeError);
console.log(JSON.stringify({state:'passed',checks,long_string_utf16_units:large.length}));
