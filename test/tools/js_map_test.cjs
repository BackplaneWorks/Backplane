// Differential checks against actual unmodified Bend emit, including complete
// map operations, Unicode keys and immutable old versions. No hub is started.
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),os=require('node:os'),cp=require('node:child_process');
const root=path.resolve(__dirname,'../..'),emit=process.env.MAP_REFERENCE_EMIT;
assert(emit,'MAP_REFERENCE_EMIT must name the exact raw server emit');
const source=fs.readFileSync(emit,'utf8'),defs=new Map();
for(const m of source.matchAll(/^function ([\w$]+)\([^\n]*\) \{[\s\S]*?^\}/gm))defs.set(m[1],m[0]);
const names=['new','set','get','has','del','to_list','bit','diff'].map(x=>'$Map$'+x+'$'),needed=new Set(),todo=[...names];
while(todo.length){const n=todo.pop();if(needed.has(n))continue;assert(defs.has(n),n);needed.add(n);for(const m of defs.get(n).matchAll(/[A-Za-z_$][\w$]*/g))if(defs.has(m[0])&&!needed.has(m[0]))todo.push(m[0]);}
const code=[...needed].map(n=>defs.get(n)).join('\n'),helper=fs.readFileSync(path.join(root,'scripts/js-map.js'),'utf8');
function engine(fast){const ctx=vm.createContext({});vm.runInContext(code+helper+(fast?'\n$Map$bit$=__bpMapBit;$Map$diff$=(a,b)=>__bpMapDiff(a,b,nat_chk);$Map$get$=(d,m,k)=>__bpMapRead(m,k,d,false);$Map$has$=(m,k)=>__bpMapRead(m,k,false,true);':'')+'\nglobalThis.f={'+names.map(n=>JSON.stringify(n)+':'+n).join(',')+'};',ctx);return ctx.f;}
const ref=engine(false),fast=engine(true),same=(a,b)=>assert.equal(JSON.stringify(a),JSON.stringify(b));
let seed=719;const rnd=n=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed%n;};
const pool=['a','A','0','é','中','\ud800','\udbff','\udc00','\udfff','😀','𝄞','\0','\uffff'];
const str=()=>Array.from({length:rnd(14)},()=>pool[rnd(pool.length)]).join('');
test('actual Base bit/diff agree at all character/presence boundaries and the 48-bit Nat edge',()=>{
 const strings=['','a','aa','a\0','\0','\ud800','\udc00','😀','a😀z','😀\ud800','\ud800\udc00','same-prefix-'.repeat(40)+'a',...Array.from({length:160},str)];
 let checks=0;
 for(const a of strings){for(const pos of [0,1,11,12,31,32,33,34,64,65,66,127,1023,2**32-1,2**48-1]){same(fast['$Map$bit$'](a,pos),ref['$Map$bit$'](a,pos));checks++;}
  for(const b of strings){same(fast['$Map$diff$'](a,b),ref['$Map$diff$'](a,b));checks++;}}
 console.log('purePrimitiveComparisons='+checks);
});
test('mixed set/get/has/delete preserve actual trie structure, old versions and code-point order',()=>{
 let a=ref['$Map$new$'](),b=fast['$Map$new$']();const keys=['','x','xy','x\0','\ud800','\udc00','\ud800\udc00',...Array.from({length:100},str)];
 for(let i=0;i<600;i++){const key=keys[rnd(keys.length)],op=rnd(5);const before=JSON.stringify([a,b]);
  if(op<2){const oldA=a,oldB=b;a=ref['$Map$set$'](a,key,i);b=fast['$Map$set$'](b,key,i);assert.equal(JSON.stringify([oldA,oldB]),before);}
  else if(op===2){const oldA=a,oldB=b;a=ref['$Map$del$'](a,key);b=fast['$Map$del$'](b,key);assert.equal(JSON.stringify([oldA,oldB]),before);}
  else {const f=op===3?'$Map$get$':'$Map$has$';same(op===3?ref[f](-1,a,key):ref[f](a,key),op===3?fast[f](-1,b,key):fast[f](b,key));assert.equal(JSON.stringify([a,b]),before);}
  same(a,b);same(ref['$Map$to_list$'](a),fast['$Map$to_list$'](b));
 }
});
test('real server transform is exact, web stays unchanged, and every missing/duplicate/changed Base fence stops before writing',()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'bp-map-'));try{
  const raw=path.join(dir,'raw.js'),min=path.join(dir,'min.js');fs.writeFileSync(raw,source);
  cp.execFileSync(process.env.BUN_BINARY||'bun',['build','--minify-whitespace','--minify-syntax','--keep-names','--target=bun',raw,'--outfile',min],{stdio:'pipe',timeout:30000});
  const original=fs.readFileSync(min,'utf8'),transform=path.join(root,'scripts/js-native.py');
  const run=(s,server=true)=>{fs.writeFileSync(min,s);return cp.spawnSync('python3',[transform,...(server?['--server']:[]),min],{encoding:'utf8',timeout:15000})};
  let r=run(original);assert.equal(r.status,0,r.stderr);const changed=fs.readFileSync(min,'utf8');assert.equal(changed.split('{return __bpMapBit(').length-1,1);assert.equal(changed.split('{return __bpMapDiff(').length-1,1);
  r=run(original,false);assert.equal(r.status,0,r.stderr);assert(!fs.readFileSync(min,'utf8').includes('__bpMapBit'));
  const defs=['bit','bit$at','diff','diff$fin','diff$step','diff$chr','msb$u','msb$u$if','get','get$go','get$leaf','has','has$go','has$leaf','lo','hi'];
  for(const n of defs){const name='$Map$'+n+'$',needle='function '+name+'(',start=original.indexOf(needle);assert(start>=0);const end=original.indexOf('function ',start+needle.length),body=original.slice(start,end);
   for(const s of [original.slice(0,start)+original.slice(end),original+body,original.slice(0,start)+body.replace('{','{;')+original.slice(end)]){r=run(s);assert.notEqual(r.status,0,name);assert.equal(fs.readFileSync(min,'utf8'),s,'Failing rewrite wrote partial output');}
  }
  console.log('negativeFences=48');
 }finally{fs.rmSync(dir,{recursive:true,force:true})}
});
