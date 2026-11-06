// Native token scan against the retained current pure lexer and parser.
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const os=require('node:os'),cp=require('node:child_process');
const root=path.resolve(__dirname,'../..'),emit=process.env.JSON_SHOW_REFERENCE_EMIT;
assert(emit,'Supply the retained exact pure reference emit');
const source=fs.readFileSync(emit,'utf8'),defs=new Map();
for(const m of source.matchAll(/^function ([\w$]+)\([^\n]*\) \{[\s\S]*?^\}/gm))defs.set(m[1],m[0]);
const name=tail=>{const xs=[...defs.keys()].filter(n=>n.endsWith(tail));assert.equal(xs.length,1,tail);return xs[0]};
const lex=name('core$047json$058Json$lex$'),parse=name('core$047json$058Json$parse$');
const decode=name('core$047utf8$058Utf8$decode$'),feed=name('core$047utf8$058Lines$feed$');
const encode=name('core$047utf8$058Utf8$encode$'),fold=name('core$047search$045fold$058Fold$text$');
const needed=new Set(),todo=[parse,'$String$reverse$go$','$String$to_lower$','$String$take$','$String$drop$',decode,feed,encode,fold];
while(todo.length){const n=todo.pop();if(needed.has(n))continue;needed.add(n);for(const m of defs.get(n).matchAll(/[A-Za-z_$][\w$]*/g))if(defs.has(m[0])&&!needed.has(m[0]))todo.push(m[0])}
const ns=source.match(/\$:\s*"([^"\n]+)\.LIdle"/)[1],ctx=vm.createContext({});
vm.runInContext([...needed].map(n=>defs.get(n)).join('\n')+'\n'+fs.readFileSync(path.join(root,'scripts/js-json-lex.js'),'utf8')+'\n'+fs.readFileSync(path.join(root,'scripts/js-utf8.js'),'utf8')+'\n'+fs.readFileSync(path.join(root,'scripts/js-text.js'),'utf8')+`
globalThis.oracleEncode=${encode};globalThis.oracleFold=${fold};globalThis.oracleLower=$String$to_lower$;globalThis.oracleTake=$String$take$;globalThis.oracleDrop=$String$drop$;globalThis.nativeEncode=__bpUtf8Encode;globalThis.nativeFold=s=>__bpFoldText(s,oracleFold);globalThis.nativeLower=s=>__bpLower(s,oracleLower);globalThis.nativeTake=__bpTake;globalThis.nativeDrop=__bpDrop;
globalThis.oracleDecode=${decode};globalThis.oracleFeed=${feed};globalThis.nativeDecode=__bpUtf8Decode;globalThis.nativeFeed=__bpLinesFeed;
globalThis.oracleLex=${lex};globalThis.oracleParse=${parse};globalThis.reverse=$String$reverse$go$;
globalThis.nativeLex=s=>__bpJsonLex(s,oracleLex,${JSON.stringify(ns)});
globalThis.nativeParse=s=>{${lex}=nativeLex;try{return oracleParse(s)}finally{${lex}=oracleLex}};
`,ctx,{timeout:10000});
const outcome=(f,...args)=>{try{return {value:JSON.parse(JSON.stringify(f(...args)))}}catch(e){return {error:String(e)}}};
function same(s){assert.deepEqual(outcome(ctx.nativeLex,s),outcome(ctx.oracleLex,s),'lex '+JSON.stringify(s));assert.deepEqual(outcome(ctx.nativeParse,s),outcome(ctx.oracleParse,s),'parse '+JSON.stringify(s))}
test('tokens and parsing preserve lexemes, duplicates, malformed grammar, escapes and surrogate repair',()=>{
 const cases=['','null','true false','[1,2}', '{"a":1,"a":2}', '{"2":2,"1":1}', '-0','01','1e+003','9007199254740993123456789','-bad','foo','\0\t[1\r,2]\n','[1,,2]','{"k" 1}', '"unterminated','"trail\\','"\\q"','"\\uZZZZ"','"\\u123"','"raw\ncontrol"','"\\ud800"','"\\udfff"','"\\ud800\\udc00"','"\\ud800x"','"\\ud800\\n"','"\\ud800\\ud801\\udfff"','"\\udfff\\ud800"'];
 for(const s of ['', 'ASCII','日本語λ🙂','"\\\n\r\t\b\f','x'.repeat(1024)])cases.push(JSON.stringify(s),JSON.stringify({s,list:[s,null,true,1.2]}));
 for(const s of ['\ud800','\udfff','a\ud800b','\udfff\ud800'])cases.push('"'+s+'"',s);
 for(const s of cases)same(s);
 let x=12345;const next=()=>x=(Math.imul(x,1664525)+1013904223)>>>0;
 for(let i=0;i<300;i++){
  const s=Array.from({length:next()%30},()=>String.fromCodePoint([0,9,34,92,0x61,0x3bb,0x65e5,0x1f642][next()%8])).join('');
  same(JSON.stringify({text:s,n:[next(),-0,1.25],nested:{yes:true}}));
  same(JSON.stringify(s).replace(/[^\x00-\x7f]/gu,c=>{const cp=c.codePointAt(0);return cp<=65535?'\\u'+cp.toString(16).padStart(4,'0'):JSON.stringify(c).slice(1,-1)}));
 }
});
test('native fast path is used and fallback owns unsupported input',()=>{
 let calls=0;const fallback=s=>{calls++;return ctx.oracleLex(s)};
 const helper=vm.runInContext('__bpJsonLex',ctx);
 helper('{"n":9007199254740993123456789,"s":"λ🙂"}',fallback,ns);assert.equal(calls,0);
 for(const s of ['"\\q"','"unterminated','"\\uZZZZ"'])helper(s,fallback,ns);
 assert.equal(calls,3);
});
test('single-pass reversal preserves code points, raw surrogate boundaries and accumulator',()=>{
 const texts=['','ASCII','λ🙂日','\ud800','\udfff','\udfff\ud800','a\ud800b','🙂'.repeat(1000)];
 for(const s of texts)for(const acc of texts.slice(0,7))assert.deepEqual(outcome((s,a)=>Array.from(s).reverse().join('')+a,s,acc),outcome(ctx.reverse,s,acc));
});
test('server-only transform rejects missing, duplicate and changed lexer/reverse definitions before writes',()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'bp-json-lex-'));
 try{
  const input=path.join(dir,'input.js'),min=path.join(dir,'min.js');fs.writeFileSync(input,source);
  cp.execFileSync(process.env.BUN_BINARY||'bun',['build','--minify-whitespace','--minify-syntax','--keep-names','--target=bun',input,'--outfile',min],{stdio:'pipe',timeout:20000});
  const original=fs.readFileSync(min,'utf8'),run=(s,server=true)=>{fs.writeFileSync(min,s);return cp.spawnSync('python3',[path.join(root,'scripts/js-native.py'),...(server?['--server']:[]),min],{encoding:'utf8',timeout:10000})};
  let result=run(original);assert.equal(result.status,0,result.stderr);const got=fs.readFileSync(min,'utf8');
  assert.equal(got.split('return __bpJsonLex(').length-1,1);
  assert.equal(got.split('return Array.from($0).reverse().join("")+$1').length-1,1);
  assert.equal(got.split('return __bpUtf8Decode(').length-1,1);assert.equal(got.split('return __bpLinesFeed(').length-1,1);
  result=run(original,false);assert.equal(result.status,0,result.stderr);assert(!fs.readFileSync(min,'utf8').includes('__bpJsonLex'));
  for(const fn of [lex,'$String$reverse$go$',decode,feed,encode,fold,'$String$to_lower$','$String$take$','$String$drop$','$Char$to_lower$','$Char$is_upper$']){
   const start=original.indexOf('function '+fn+'('),stop=original.indexOf('function ',start+9);assert(start>=0&&stop>start);const body=original.slice(start,stop);
   for(const bad of [original.replace(body,''),original+body,original.replace(body,body.replace('return ','return void '))]){
    result=run(bad);assert.notEqual(result.status,0);assert.equal(fs.readFileSync(min,'utf8'),bad);
   }
  }
  const lexGo=lex.replace('$058Json$lex$','$058Lex$go$'),start=original.indexOf('function '+lex+'('),stop=original.indexOf('function ',start+9),body=original.slice(start,stop);
  const bad=original.replace(body,body.replace(lexGo+'(','wrong$058Lex$go$('));result=run(bad);assert.notEqual(result.status,0);assert.equal(fs.readFileSync(min,'utf8'),bad);
 }finally{fs.rmSync(dir,{recursive:true,force:true})}
});

const list=xs=>xs.reduceRight((tail,head)=>({$:'Con',head,tail}),{$:'Nil'});
test('UTF-8 and framing match the pure byte oracle across every lead, invalid forms, chunks and retained carry',()=>{
 const tag=source.match(/\$:\s*"([^"\n]+\.Lines)"/)[1];
 const check=bytes=>assert.deepEqual(outcome(ctx.nativeDecode,list(bytes)),outcome(ctx.oracleDecode,list(bytes)),JSON.stringify(bytes));
 for(let a=0;a<256;a++){
  check([a]);for(const b of [0,10,63,127,128,129,159,191,192,224,240,255])check([a,b]);
 }
 for(const bytes of [[0xef,0xbb,0xbf], [0xc0,0x80], [0xed,0xa0,0x80], [0xf4,0x90,0x80,0x80], [0xf7,0xbf,0xbf,0xbf], [0xe0,0x80,0x80], [0xf0,0x80,0x80,0x80], [4294967295,256,1024]])check(bytes);
 let seed=37;const next=()=>seed=(Math.imul(seed,1664525)+1013904223)>>>0;
 for(let i=0;i<500;i++)check(Array.from({length:next()%64},()=>next()>>>24));
 const sequence=[...Buffer.from('a\n\nλ🙂日\r\nb\n'),0xe0,0x80,10,0xff,0xc2,65,10,0xf0,0x9f];
 for(let size=1;size<=sequence.length;size++){
  let state={$:tag,part:list([65,0x80]),done:list(['queued-first','queued-second'])};
  for(let i=0;i<sequence.length;i+=size){
   const before=JSON.stringify(state),bs=list(sequence.slice(i,i+size)),expected=outcome(ctx.oracleFeed,state,bs),got=outcome(ctx.nativeFeed,state,bs,tag);
   assert.deepEqual(got,expected);assert.equal(JSON.stringify(state),before,'input alias changed');state=got.value.fst;
  }
 }
 const empty={$:tag,part:list([1,2]),done:list([])},got=ctx.nativeFeed(empty,list([]),tag);assert.equal(got.fst.part,empty.part);
});

test('search indexing text primitives preserve ASCII-only lowercase, Unicode folding, code-point prefixes and WTF8 bytes',()=>{
 const array=xs=>{const out=[];for(let p=xs;p.$!=='Nil';p=p.tail)out.push(p.head);return out};
 const texts=['', ...Array.from({length:128},(_,i)=>String.fromCharCode(i)), 'ABC xyz!?', 'ÀÉİßΣﬃ日本語🙂', '\ud800','\udfff','a\ud800b', '\udfff\ud800', 'abc DEF '.repeat(100)];
 for(const s of texts){
  assert.deepEqual(outcome(ctx.nativeLower,s),outcome(ctx.oracleLower,s));
  assert.deepEqual(outcome(ctx.nativeFold,s),outcome(ctx.oracleFold,s));
  assert.deepEqual(outcome(s=>array(ctx.nativeEncode(s)),s),outcome(s=>array(ctx.oracleEncode(s)),s));
  for(const n of [0,1,2,3,7,30,1024,2**53-1]){assert.deepEqual(outcome(ctx.nativeTake,s,n),outcome(ctx.oracleTake,s,n));assert.deepEqual(outcome(ctx.nativeDrop,s,n),outcome(ctx.oracleDrop,s,n))}
 }
 let calls=0;const helper=vm.runInContext('__bpFoldText',ctx),fallback=s=>{calls++;return ctx.oracleFold(s)};
 helper('AbC ~!?',fallback);assert.equal(calls,0);helper('日本語',fallback);assert.equal(calls,1);
 let seed=41;const next=()=>seed=(Math.imul(seed,1664525)+1013904223)>>>0;
 for(let i=0;i<300;i++){
  const s=Array.from({length:next()%40},()=>String.fromCodePoint([0,65,90,97,127,0x3a3,0x65e5,0x1f642,0xd800][next()%9])).join('');
  assert.deepEqual(outcome(ctx.nativeLower,s),outcome(ctx.oracleLower,s));
  assert.deepEqual(outcome(ctx.nativeFold,s),outcome(ctx.oracleFold,s));
  assert.deepEqual(outcome(s=>array(ctx.nativeEncode(s)),s),outcome(s=>array(ctx.oracleEncode(s)),s));
  const n=next()%50;assert.deepEqual(outcome(ctx.nativeTake,s,n),outcome(ctx.oracleTake,s,n));assert.deepEqual(outcome(ctx.nativeDrop,s,n),outcome(ctx.oracleDrop,s,n));
 }
});
