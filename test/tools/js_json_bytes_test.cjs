// Server string counters against distinct, unchanged compiled pure references.
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),vm=require('node:vm'),cp=require('node:child_process');
const root=path.resolve(__dirname,'../..'),emit=process.env.JSON_BYTES_REFERENCE_EMIT;assert(emit,'Supply exact retained pure reference emit');
const source=fs.readFileSync(emit,'utf8'),defs=new Map();for(const m of source.matchAll(/^function ([\w$]+)\([^\n]*\) \{[\s\S]*?^\}/gm))defs.set(m[1],m[0]);
const name=tail=>{const xs=[...defs.keys()].filter(n=>n.endsWith(tail));assert.equal(xs.length,1,tail);return xs[0]};
const esc=name('core$047json$058Json$bytes$esc$'),str=name('core$047json$058Json$bytes$str$'),count=name('core$047json$058Json$bytes$'),needed=new Set(),todo=[count];while(todo.length){const n=todo.pop();if(needed.has(n))continue;needed.add(n);for(const m of defs.get(n).matchAll(/[A-Za-z_$][\w$]*/g))if(defs.has(m[0])&&!needed.has(m[0]))todo.push(m[0])}
const ctx=vm.createContext({});vm.runInContext([...needed].map(n=>defs.get(n)).join('\n')+'\n'+fs.readFileSync(path.join(root,'scripts/js-json-bytes.js'),'utf8')+`;globalThis.ref={esc:${esc},str:${str}};globalThis.native={esc:(s,n)=>__bpJsonBytesEsc(s,n,nat_chk),str:(s,n)=>__bpJsonBytesStr(s,n,nat_chk)};globalThis.count=${count};globalThis.guard=nat_chk;`,ctx,{timeout:10000});
const outcome=(fn,...args)=>{try{return {value:fn(...args)}}catch(e){return {error:String(e)}}};
const corpus=['','ASCII printable','日本語😀','quote"\\','\n\r\t\b\f',String.fromCharCode(0,31,0xd800,0xdc00,0xdfff),'x'.repeat(4096),...Array.from({length:256},(_,i)=>String.fromCodePoint(i)),...[0x7ff,0x800,0xd7ff,0xd800,0xdbff,0xdc00,0xdfff,0xe000,0xffff,0x10000,0x10ffff].map(x=>String.fromCodePoint(x))];
test('distinct native/reference loops preserve escapes, Unicode, raw surrogates, accumulator and overflow errors',()=>{
 let comparisons=0,throws=0;for(const kind of ['esc','str']){assert.notEqual(ctx.ref[kind],ctx.native[kind]);for(const text of corpus)for(const n of [0,17,281474976710653,281474976710655]){const a=outcome(ctx.ref[kind],text,n),b=outcome(ctx.native[kind],text,n);assert.deepEqual(b,a);comparisons++;if(a.error)throws++}}assert(throws>0);assert.notDeepEqual(outcome((s,n)=>ctx.native.esc(s,n)-1,'"',0),outcome(ctx.ref.esc,'"',0));console.log(JSON.stringify({oracleComparisons:comparisons,overflowCases:throws,negativeControlRejected:true}));
});
const json=(kind,fields={})=>({$:'../core/json.'+kind,...fields}),end=json('End');
const strings=corpus.slice(0,7),trees=[end,json('Null'),json('Flag',{value:false}),json('Flag',{value:true}),...['01','-0','1e+03','日本語\n',String.fromCharCode(0xd800)].map(raw=>json('Num',{raw})),...strings.map(text=>json('Str',{text}))];
const field=(key,value,tail)=>json('Field',{key,value,tail}),item=(head,tail)=>json('Item',{head,tail});
trees.push(item(trees[2],json('Str',{text:'bare tail'})),field('"\n日本語',trees[5],end),json('Obj',{fields:field('same',trees[2],field('same',trees[3],end))}),json('Arr',{items:item(trees[2],item(trees[2],end))}),json('Obj',{fields:json('Str',{text:'unusual typed chain'})}));
test('whole JSON traversal keeps constructors, raw numeric text, duplicate field order and aliases exact',()=>{
 let cases=0;for(const j of trees){const before=JSON.stringify(j);ctx.selected=ctx.ref;vm.runInContext(`${esc}=selected.esc;${str}=selected.str;`,ctx);const a=outcome(ctx.count,j);ctx.selected=ctx.native;vm.runInContext(`${esc}=selected.esc;${str}=selected.str;`,ctx);assert.deepEqual(outcome(ctx.count,j),a);assert.equal(JSON.stringify(j),before);cases++}console.log('wholeTreeComparisons='+cases);
});
test('actual server transform lands twice; web pure; missing/duplicate/body/callee/source drift fails before writes',()=>{
 const temp=fs.mkdtempSync(path.join(os.tmpdir(),'bp-json-bytes-'));try{
  const scripts=path.join(temp,'scripts');fs.mkdirSync(scripts,{recursive:true});for(const file of ['js-native.py','js-content-eq.js','js-json-show.js','js-cbor-text.js','js-json-bytes.js','js-map.js'])fs.copyFileSync(path.join(root,'scripts',file),path.join(scripts,file));
  for(const file of ['src/server/thin-content-eq.bend','src/core/json.bend','src/core/secret.bend']){const to=path.join(temp,file);fs.mkdirSync(path.dirname(to),{recursive:true});fs.copyFileSync(path.join(root,file),to)}
  const raw=path.join(temp,'input.js'),min=path.join(temp,'min.js');fs.writeFileSync(raw,source);cp.execFileSync(process.env.BUN_BINARY||'bun',['build','--minify-whitespace','--minify-syntax','--keep-names','--target=bun',raw,'--outfile',min],{stdio:'pipe',timeout:20000});const original=fs.readFileSync(min,'utf8');
  const run=(s,server=true)=>{fs.writeFileSync(min,s);return cp.spawnSync('python3',[path.join(scripts,'js-native.py'),...(server?['--server']:[]),min],{encoding:'utf8',timeout:10000})};let result=run(original);assert.equal(result.status,0,result.stderr);const got=fs.readFileSync(min,'utf8');assert.equal(got.split('return __bpJsonBytesEsc(').length-1,1);assert.equal(got.split('return __bpJsonBytesStr(').length-1,1);
  result=cp.spawnSync(process.execPath,['--input-type=module','--check'],{input:got,encoding:'utf8',timeout:10000});assert.equal(result.status,0,result.stderr.slice(0,200));
  result=run(original,false);assert.equal(result.status,0,result.stderr);assert(!fs.readFileSync(min,'utf8').includes('__bpJsonBytes'));
  let fences=0;for(const fn of [esc,str]){const start=original.indexOf('function '+fn+'('),stop=original.indexOf('function ',start+9);assert(start>=0&&stop>start);const body=original.slice(start,stop),callee=fn.replace(/\$(esc|str)\$$/,'$'+(fn===esc?'chr':'raw')+'$');assert(body.includes(callee+'('));
   const bad=[original.replace(body,''),original+body,original.replace(body,body.replace('nat_chk(','changedGuard(')),original.replace(body,body.replace(callee+'(','wrong$058Json$bytes$'+(fn===esc?'chr':'raw')+'$(')),original.replace('function '+callee+'(','function removedCallee(')];
   for(const s of bad){result=run(s);assert.notEqual(result.status,0);assert.equal(fs.readFileSync(min,'utf8'),s,'Failed transform wrote partial output');fences++}
  }
  fs.appendFileSync(path.join(temp,'src/core/json.bend'),'\n# deliberate source drift\n');result=run(original);assert.notEqual(result.status,0);assert.equal(fs.readFileSync(min,'utf8'),original);console.log('negativeFences='+(fences+1));
 }finally{fs.rmSync(temp,{recursive:true,force:true})}
});
