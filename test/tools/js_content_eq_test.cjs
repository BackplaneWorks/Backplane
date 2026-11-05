const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),vm=require('node:vm'),cp=require('node:child_process');
const root=path.resolve(__dirname,'../..'),emit=process.env.CONTENT_EQ_REFERENCE_EMIT;assert(emit,'Exact candidate raw emit required');
const source=fs.readFileSync(emit,'utf8'),defs=new Map();for(const m of source.matchAll(/^function ([\w$]+)\([^\n]*\) \{[\s\S]*?^\}/gm))defs.set(m[1],m[0]);
const name=tail=>{const xs=[...defs.keys()].filter(n=>n.endsWith(tail));assert.equal(xs.length,1,tail);return xs[0]};
const same=name('thin$045content$045eq$058ContentEq$same$'),go=name('thin$045content$045eq$058ContentEq$go$'),needed=new Set(),todo=[same];while(todo.length){const n=todo.pop();if(needed.has(n))continue;needed.add(n);for(const m of defs.get(n).matchAll(/[A-Za-z_$][\w$]*/g))if(defs.has(m[0])&&!needed.has(m[0]))todo.push(m[0])}
const ctx=vm.createContext({});vm.runInContext([...needed].map(n=>defs.get(n)).join('\n')+'\n'+fs.readFileSync(path.join(root,'scripts/js-content-eq.js'),'utf8')+`;globalThis.oracle=${same};globalThis.native=(a,b)=>__bpContentEq(a,b,${go});`,ctx);
const j=(kind,props={})=>({$:'../core/json.'+kind,...props}),end=j('End');const atom=[j('Null'),j('Flag',{value:false}),j('Flag',{value:true}),j('Num',{raw:'1'}),j('Num',{raw:'01'}),j('Str',{text:'日本語🚀\u0000'}),j('Arr',{items:end}),j('Obj',{fields:end}),j('Item',{head:j('Null'),tail:end}),j('Field',{key:'id',value:j('Num',{raw:'1'}),tail:end}),end];
let comparisons=0;const check=(a,b)=>{const before=JSON.stringify([a,b]);assert.equal(ctx.native(a,b),ctx.oracle(a,b));assert.equal(JSON.stringify([a,b]),before);comparisons++};
test('native structural comparison matches pure constructors, field order, duplicate keys, raw numbers and valid Unicode',()=>{
 for(const a of atom)for(const b of atom){check(a,b);check(a,JSON.parse(JSON.stringify(b)))}
 const field=(k,v,t)=>j('Field',{key:k,value:v,tail:t});const a=j('Obj',{fields:field('id',atom[3],field('id',atom[4],end))}),b=j('Obj',{fields:field('id',atom[4],field('id',atom[3],end))});check(a,b);assert.equal(ctx.native(a,b),false);check(a,JSON.parse(JSON.stringify(a)));
 const shared=j('Str',{text:'valid 日本語🚀'.repeat(32)}),arr=j('Arr',{items:j('Item',{head:shared,tail:j('Item',{head:shared,tail:end})})});check(arr,arr);check(arr,JSON.parse(JSON.stringify(arr)));assert.equal(ctx.native(atom[3],atom[5]),false);console.log('oracleComparisons='+comparisons);
});
test('remembered answers stay the full comparison over shared, rebuilt and edited trees asked in any order',()=>{
 // random trees with shared subtrees, their deep copies and copies with one leaf edited, compared pairwise in
 // a shuffled order several times over (each answer can come from the memo of an earlier pair)
 let seed=7;const rnd=n=>{seed=(seed*1103515245+12345)%2147483648;return seed%n};
 const leaf=()=>[j('Null'),j('Flag',{value:!!rnd(2)}),j('Num',{raw:String(rnd(4))}),j('Str',{text:'s'+rnd(5)})][rnd(4)];
 const chain=(kind,xs)=>xs.reduceRight((t,x)=>kind==='Arr'?j('Item',{head:x,tail:t}):j('Field',{key:'k'+x[0],value:x[1],tail:t}),end);
 const tree=d=>d===0||rnd(4)===0?leaf():rnd(2)?j('Arr',{items:chain('Arr',Array.from({length:rnd(4)},()=>tree(d-1)))}):j('Obj',{fields:chain('Obj',Array.from({length:rnd(4)},(_,i)=>[rnd(3),tree(d-1)]))});
 const copy=x=>JSON.parse(JSON.stringify(x));
 const edit=x=>{const y=copy(x);const nodes=[];const walk=n=>{for(const k of ['head','value','items','fields','tail'])if(n[k]){if(['Null','Flag','Num','Str'].includes(n[k].$.split('.').pop()))nodes.push([n,k]);walk(n[k])}};walk(y);if(!nodes.length)return y;const [n,k]=nodes[rnd(nodes.length)];n[k]=j('Str',{text:'edited'+rnd(1000)});return y};
 const pool=[];for(let i=0;i<40;i++){const t=tree(5);pool.push(t,copy(t),edit(t));if(i>0){const s=pool[rnd(pool.length)];pool.push(j('Arr',{items:j('Item',{head:s,tail:j('Item',{head:t,tail:end})})}),j('Arr',{items:j('Item',{head:copy(s),tail:j('Item',{head:t,tail:end})})}))}}
 let asked=0,equal=0;for(let round=0;round<3;round++)for(let k=0;k<4000;k++){const a=pool[rnd(pool.length)],b=pool[rnd(pool.length)];const want=ctx.oracle(a,b);assert.equal(ctx.native(a,b),want);assert.equal(ctx.native(b,a),want);asked++;if(want)equal++}
 assert(equal>100&&equal<asked-100,'both answers exercised');console.log(JSON.stringify({memoComparisons:asked,equal}));
});
test('actual transform is server-only and missing/duplicate/body/callee/source drift fails before writes',()=>{
 const temp=fs.mkdtempSync(path.join(os.tmpdir(),'bp-content-eq-'));try{
  const scripts=path.join(temp,'scripts'),src=path.join(temp,'src/server');fs.mkdirSync(scripts,{recursive:true});fs.mkdirSync(src,{recursive:true});for(const file of ['js-native.py','js-content-eq.js','js-json-show.js','js-cbor-text.js','js-json-bytes.js','js-json-lex.js','js-utf8.js','js-text.js','js-map.js','js-tree-children.js','js-memo.js'])fs.copyFileSync(path.join(root,'scripts',file),path.join(scripts,file));fs.mkdirSync(path.join(temp,'src/core'),{recursive:true});fs.copyFileSync(path.join(root,'src/core/json.bend'),path.join(temp,'src/core/json.bend'));fs.copyFileSync(path.join(root,'src/core/secret.bend'),path.join(temp,'src/core/secret.bend'));fs.copyFileSync(path.join(root,'src/core/utf8.bend'),path.join(temp,'src/core/utf8.bend'));fs.copyFileSync(path.join(root,'src/core/search-fold.bend'),path.join(temp,'src/core/search-fold.bend'));const pure=path.join(src,'thin-content-eq.bend');fs.copyFileSync(path.join(root,'src/server/thin-content-eq.bend'),pure);
  const raw=path.join(temp,'input.js'),min=path.join(temp,'min.js');fs.writeFileSync(raw,source);cp.execFileSync(process.env.BUN_BINARY||'bun',['build','--minify-whitespace','--minify-syntax','--keep-names','--target=bun',raw,'--outfile',min],{stdio:'pipe',timeout:20000});const original=fs.readFileSync(min,'utf8');
  const rx=new RegExp('function '+same.replace(/[.*+?^${}()|[\]\\]/g,'\\$&')+'\\(([^,]+),([^)]*)\\)\\{return '+go.replace(/[.*+?^${}()|[\]\\]/g,'\\$&')+'\\(\\1,\\2\\)\\}');const m=original.match(rx);assert(m,'Exact wrapper absent');const definition=m[0];
  const run=(text,server=true)=>{fs.writeFileSync(min,text);return cp.spawnSync('python3',[path.join(scripts,'js-native.py'),...(server?['--server']:[]),min],{encoding:'utf8',timeout:10000})};let res=run(original);assert.equal(res.status,0,res.stderr);assert.equal(fs.readFileSync(min,'utf8').split('return __bpContentEq(').length-1,1);
  res=run(original,false);assert.equal(res.status,0,res.stderr);assert(!fs.readFileSync(min,'utf8').includes('__bpContentEq'));
  const wrongGo=go.replace('thin$045content$045eq','wrong');for(const text of [original.replace(definition,definition.replace('return ','return !')),original+definition,original.replace(definition,definition.replace(go,wrongGo)),original.replace('function '+go+'(','function removedGo(')]){res=run(text);assert.notEqual(res.status,0);assert.equal(fs.readFileSync(min,'utf8'),text,'Failed transform wrote partial output')}
  fs.appendFileSync(pure,'\n# deliberate source drift\n');res=run(original);assert.notEqual(res.status,0);assert.equal(fs.readFileSync(min,'utf8'),original);console.log('negativeFences=5');
 }finally{fs.rmSync(temp,{recursive:true,force:true})}
});
