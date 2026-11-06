// host.js's jt_frame (Sock.send_tree) against the pure frame it replaces: the bytes of
// http.bend's Ws.encode(2, cbor.bend's Cbor.encode(j)) from an exact raw emit.
//   TREE_FRAME_REFERENCE_EMIT=build/js/backplane-serve/all.js [TREE_FRAME_TREES=dir of {a,b} dumps] node --stack-size=20000 --test test/tools/js_tree_frame_test.cjs
const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const root=path.resolve(__dirname,'../..'),emit=process.env.TREE_FRAME_REFERENCE_EMIT;assert(emit,'Exact raw emit required');
const source=fs.readFileSync(emit,'utf8'),defs=new Map();for(const m of source.matchAll(/^function ([\w$]+)\([^\n]*\) \{[\s\S]*?^\}/gm))defs.set(m[1],m[0]);
const name=tail=>{const xs=[...defs.keys()].filter(n=>n.endsWith(tail));assert.equal(xs.length,1,tail);return xs[0]};
const enc=name('core$047cbor$058Cbor$encode$'),ws=name('core$047http$058Ws$encode$'),keys=name('core$047cbor$058Cbor$keys$'),words=name('core$047cbor$058Cbor$words$');
const needed=new Set(),todo=[enc,ws,keys,words];while(todo.length){const n=todo.pop();if(needed.has(n))continue;needed.add(n);for(const m of defs.get(n).matchAll(/[A-Za-z_$][\w$]*/g))if(defs.has(m[0])&&!needed.has(m[0]))todo.push(m[0])}
const host=fs.readFileSync(path.join(root,'src/server/effects/host.js'),'utf8');
const jt=host.slice(host.indexOf('let jt_dicts = null;'),host.indexOf('// the Bun loop (loop.js) sends these bytes'));assert(jt.includes('function jt_frame('));
const prelude=['word_to_u32','u32_to_word','nat_chk','nat_host','cmp_new','nat_divmod','char_new','run_tail','run_clo','run_loop'].map(f=>{const m=source.match(new RegExp('^(?:var )?(?:function )?'+f+'[\\s\\S]*?^\\}','m'));return m?m[0]:''}).join('\n');
const ctx=vm.createContext({});vm.runInContext(prelude+'\n'+[...needed].map(n=>defs.get(n)).join('\n')+'\n'+jt+`
const list=xs=>{const b=[];for(;xs.$==="Con";xs=xs.tail)b.push(xs.head);return b};
const join=xs=>list(xs).join("\\n");
globalThis.K=join(${keys}());globalThis.W=join(${words}());
globalThis.pure=j=>list(${ws}(2,${enc}(j)));globalThis.native=j=>Array.from(jt_frame(j,K,W));`,ctx,{timeout:20000});
const j=(kind,p={})=>({$:'../core/json.'+kind,...p}),end=j('End');
const field=(k,v,t)=>j('Field',{key:k,value:v,tail:t}),item=(h,t)=>j('Item',{head:h,tail:t});
const nums=['0','7','23','24','255','256','65535','65536','4294967295','4294967296','01','-0','-1','-4294967296','-4294967297','1.5','1e+03','-','',"9".repeat(11),'日本'];
const strs=['','t','reply','id','x'.repeat(30),'日本語🚀','quote"\\\n',String.fromCharCode(0xd800),'a'+String.fromCharCode(0xdc00),'é'.repeat(300)];
const check=(x,label,deep=true)=>{const before=deep?JSON.stringify(x):"";assert.deepEqual(ctx.native(x),ctx.pure(x),label);if(deep)assert.equal(JSON.stringify(x),before)};
test('tree frames are the pure frame: scalars, numbers, dictionary keys and words, Unicode, lengths, malformed chains',()=>{
 let n=0;for(const r of nums){check(j('Num',{raw:r}),r);n++}for(const s of strs){check(j('Str',{text:s}),s);check(j('Obj',{fields:field(s,j('Null'),end)}),s);n+=2}
 for(const w of ctx.W.split('\n').slice(0,40))check(j('Str',{text:w}),w);for(const k of ctx.K.split('\n').slice(0,40))check(j('Obj',{fields:field(k,j('Flag',{value:true}),end)}),k);
 for(const len of [0,1,23,24,255,256,65535,65536]){let xs=end;for(let i=0;i<len;i++)xs=item(j('Num',{raw:String(i)}),xs);check(j("Arr",{items:xs}),"len "+len,len<1000);n++}
 check(j('Arr',{items:item(j('Null'),j('Str',{text:'bare tail'}))}),'bare tail');check(j('Obj',{fields:field('a',j('Null'),j('Num',{raw:'3'}))}),'bare field tail');
 check(j('Str',{text:'x'.repeat(70000)}),'64 KiB+ frame');n+=3;
 let seed=5;const rnd=m=>{seed=(seed*1103515245+12345)%2147483648;return seed%m};
 const pick=a=>a[rnd(a.length)];const ks=ctx.K.split('\n'),wd=ctx.W.split('\n');
 const tree=d=>d===0||rnd(4)===0?pick([()=>j('Null'),()=>j('Flag',{value:!!rnd(2)}),()=>j('Num',{raw:pick(nums)}),()=>j('Str',{text:rnd(2)?pick(wd):pick(strs)})])():rnd(2)?j('Arr',{items:Array.from({length:rnd(5)},()=>tree(d-1)).reduceRight((t,x)=>item(x,t),end)}):j('Obj',{fields:Array.from({length:rnd(5)},()=>[rnd(2)?pick(ks):pick(strs),tree(d-1)]).reduceRight((t,[k,v])=>field(k,v,t),end)});
 for(let i=0;i<400;i++){check(tree(6),'random '+i);n++}console.log('treeFrames='+n);
});
test('real view replies encode the same',{skip:!process.env.TREE_FRAME_TREES},()=>{
 let n=0;for(const f of fs.readdirSync(process.env.TREE_FRAME_TREES).filter(f=>f.endsWith('.json'))){const d=JSON.parse(fs.readFileSync(path.join(process.env.TREE_FRAME_TREES,f)));for(const x of [d.a,d.b]){check(x,f);n++}}console.log('realFrames='+n);
});
