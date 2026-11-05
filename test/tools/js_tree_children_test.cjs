// scripts/js-tree-children.js (server Tree.children) against the pure walk it replaces, and its js-native fences.
//   TREE_CHILDREN_REFERENCE_EMIT=build/js/backplane-serve/all.js node --test test/tools/js_tree_children_test.cjs
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path'),vm=require('node:vm'),cp=require('node:child_process');
const root=path.resolve(__dirname,'../..'),emit=process.env.TREE_CHILDREN_REFERENCE_EMIT;assert(emit,'Exact raw emit required');
test('one index per state answers every id as the pure in-order filter then sort does',()=>{
 const ctx=vm.createContext({});vm.runInContext(fs.readFileSync(path.join(root,'scripts/js-tree-children.js'),'utf8')+';globalThis.kids=__bpTreeChildren;',ctx);
 let seed=3;const rnd=n=>{seed=(seed*1103515245+12345)%2147483648;return seed%n};
 const list=a=>a.reduceRight((t,x)=>({$:'Con',head:x,tail:t}),{$:'Nil'}),arr=l=>{const r=[];for(;l.$==='Con';l=l.tail)r.push(l.head);return r};
 const sort=l=>list(arr(l).sort((a,b)=>a.c-b.c||(a.id<b.id?-1:a.id>b.id?1:0)));
 let asked=0,parents=0;for(let k=0;k<200;k++){
  const n=rnd(40),ts=Array.from({length:n},(_,i)=>({id:'t'+i,c:rnd(5)})),par=new Map(ts.map(t=>[t.id,rnd(3)?'':'t'+rnd(n)]));
  const s={threads:list(ts)};let calls=0;const all=x=>x.threads,parent=(x,id)=>{calls++;assert.equal(x,s);return par.get(id)},tid=t=>t.id;
  const pure=id=>sort(list(ts.filter(t=>parent(s,tid(t))===id)));
  for(const id of ['',...ts.map(t=>t.id),'none']){const before=calls;const got=arr(ctx.kids(s,id,all,parent,tid,sort));calls=before;assert.deepEqual(got,arr(pure(id)));asked++}
  const c0=calls;ctx.kids(s,'t0',all,parent,tid,sort);assert.equal(calls,c0,'answered from the index');parents+=n}
 console.log(JSON.stringify({asked,parents}));
});
test('the transform lands once on a real emit; drift in the entry, walk or add fails before writing',()=>{
 const temp=fs.mkdtempSync(path.join(os.tmpdir(),'bp-tree-kids-'));try{
  const scripts=path.join(temp,'scripts');fs.mkdirSync(scripts,{recursive:true});for(const f of fs.readdirSync(path.join(root,'scripts')).filter(f=>f==='js-native.py'||/^js-.*\.js$/.test(f)))fs.copyFileSync(path.join(root,'scripts',f),path.join(scripts,f));
  for(const f of ['src/server/thin-content-eq.bend','src/core/json.bend','src/core/secret.bend','src/core/utf8.bend','src/core/search-fold.bend']){const to=path.join(temp,f);fs.mkdirSync(path.dirname(to),{recursive:true});fs.copyFileSync(path.join(root,f),to)}
  const min=path.join(temp,'min.js');cp.execFileSync(process.env.BUN_BINARY||'bun',['build','--minify-whitespace','--minify-syntax','--keep-names','--target=bun',emit,'--outfile',min],{stdio:'pipe',timeout:60000});const original=fs.readFileSync(min,'utf8');
  const run=t=>{fs.writeFileSync(min,t);return cp.spawnSync('python3',[path.join(scripts,'js-native.py'),'--server',min],{encoding:'utf8',timeout:60000})};
  let r=run(original);assert.equal(r.status,0,r.stderr);const got=fs.readFileSync(min,'utf8');assert.equal(got.split('){return __bpTreeChildren(_s_0,_id_0,').length-1,1);assert.equal(got.split('const __bpTreeKids = new WeakMap').length-1,1);
  const entry=original.match(/function [\w$]*\$058Tree\$children\$\(_s_0,_id_0\)\{[^}]*\}/)[0],go=original.match(/function [\w$]*\$058Tree\$children\$go\$\(_ts_0,_s_0,_id_0\)\{[^}]*\}[^}]*\}/)[0],add=original.match(/function [\w$]*\$058Tree\$roots\$add\$\(_skip_0,_t_0,_rest_0\)\{[^}]*\}/)[0];
  let fences=0;for(const t of [original.replace(entry,''),original+entry,original.replace(go,go.replace('$Bool$not$(','(')),original.replace(add,add.replace('return _rest_0;else','return{$:"Nil"};else')),original.replace(go,go.replace('display_parent$(','display_raw$('))]){r=run(t);assert.notEqual(r.status,0);assert.equal(fs.readFileSync(min,'utf8'),t,'Failed transform wrote partial output');fences++}
  console.log('negativeFences='+fences);
 }finally{fs.rmSync(temp,{recursive:true,force:true})}
});
test('identity memo: the same object and String answer what the pure def answered, once',()=>{
 const ctx=vm.createContext({});vm.runInContext(fs.readFileSync(path.join(root,'scripts/js-memo.js'),'utf8')+';globalThis.memo=__bpMemo2;',ctx);
 let calls=0;const f=(a,b)=>{calls++;return {of:a.k+':'+b}};const a={k:1},a2={k:1};
 const r1=ctx.memo('t',a,'x',f),r2=ctx.memo('t',a,'x',f),r3=ctx.memo('t',a,'y',f),r4=ctx.memo('t',a2,'x',f),r5=ctx.memo('u',a,'x',f);
 assert.equal(r1,r2);assert.deepEqual(r3,{of:'1:y'});assert.notEqual(r4,r1);assert.deepEqual(r4,r1);assert.notEqual(r5,r1);assert.equal(calls,4);
});
test('four-argument memo: the same two objects and equal Strings/numbers answer once; no object is asked again',()=>{
 const ctx=vm.createContext({});vm.runInContext(fs.readFileSync(path.join(root,'scripts/js-memo.js'),'utf8')+';globalThis.memo4=__bpMemo4;',ctx);
 let calls=0;const f=(a,b,c,d)=>{calls++;return {of:[a&&a.k,b.k,c,String(d)].join(':')}};const st={k:1},nf={k:2},nf2={k:2};
 const r1=ctx.memo4('rows',st,nf,'t',5n,f),r2=ctx.memo4('rows',st,nf,'t',5n,f),r3=ctx.memo4('rows',st,nf,'t',6n,f),r4=ctx.memo4('rows',st,nf2,'t',5n,f),r5=ctx.memo4('rows',st,nf,'u',5n,f),r6=ctx.memo4('rows',st,nf,'t','5',f);
 assert.equal(r1,r2);assert.notEqual(r3,r1);assert.notEqual(r4,r1);assert.deepEqual(r4,r1);assert.notEqual(r5,r1);assert.notEqual(r6,r1);assert.equal(calls,5);
 ctx.memo4('rows',null,nf,'t',5n,f);ctx.memo4('rows',null,nf,'t',5n,f);assert.equal(calls,7);
});
test('one-argument identity memo: the very same object answers once, an equal copy is asked again',()=>{
 const ctx=vm.createContext({});vm.runInContext(fs.readFileSync(path.join(root,'scripts/js-memo.js'),'utf8')+';globalThis.memo1=__bpMemo1;',ctx);
 let calls=0;const f=a=>{calls++;return {n:a.length}};const rows=[1,2],copy=[1,2];
 const r1=ctx.memo1('own',rows,f),r2=ctx.memo1('own',rows,f),r3=ctx.memo1('own',copy,f),r4=ctx.memo1('other',rows,f);
 assert.equal(r1,r2);assert.notEqual(r3,r1);assert.deepEqual(r3,r1);assert.notEqual(r4,r1);assert.equal(calls,3);
 ctx.memo1('own',null,()=>{calls++;return 1});ctx.memo1('own',null,()=>{calls++;return 1});assert.equal(calls,5);
});
