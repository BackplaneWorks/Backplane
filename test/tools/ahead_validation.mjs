// Deterministic generated sequences through the actual emitted Bend UI.
// node test/tools/ahead_validation.mjs MODULE [--steps N] [--out DIR]
import assert from 'node:assert/strict';
import {mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {json, plain, learnJson, kind, whole} from '../../src/web/wire.js';

export const list = xs => {const a=[];for(let x=xs;kind(x)==='Con';x=x.tail)a.push(x.head);return a;};
export function* nodes(n) {yield n;for(const k of n.kids||[])yield* nodes(k);}
export const attrs = n => Object.fromEntries((n.attrs||[]).filter(a=>a.name).map(a=>[a.name,a.value]));
const message=(thread,msg,text,role='assistant',at=10)=>({$:'MessagePosted',thread,msg,role,text,at});
const changes=items=>({t:'changes',items});
const action=(a,v='')=>({kind:'act',action:a,value:v});
const receive=frame=>({kind:'recv',frame});
const thread=id=>({$:'ThreadCreated',id,project:'p',title:id,env:'local',branch:'',worktree:'',provider:'claude',model:'',at:2});
const baseline=()=>[{$:'ProjectCreated',id:'p',title:'Synthetic validation',root:'/synthetic',at:1},thread('A'),thread('B'),
  {$:'SettingSet',key:'onboard.done',value:'1',at:1},{$:'SettingSet',key:'onboard.uber',value:'1',at:1},
  {$:'BotSet',id:'bot',name:'Synthetic bot',thread:'B',bits:0,soul:'',at:3},message('A','m0','initial','user')];

export function setup(F) {
  learnJson(F.json_tags());
  return F.ui(F.received(F.init(),json({t:'log',origin:'synthetic',since:0,info:{},items:baseline()})));
}

// One-deletion fixed point: 1-minimal, not claimed globally shortest.
export function shrink(sequence, fails) {
  let out=sequence.slice(),i=0;
  while(i<out.length){const next=out.toSpliced(i,1);if(fails(next)){out=next;i=0;}else i++;}
  return out;
}

export function replay(F, sequence, {render=true, check=true}={}) {
  let u=setup(F),selected='',drafts=new Map(), known=new Map([['A',new Set(['m0'])],['B',new Set()]]),steps=0;
  const sends=new Map();let lastTree;
  for(const e of sequence){
    let a;
    if(e.kind==='act'){
      a=F.step(u,e.action,e.value);
      if(e.action==='select')selected=e.value;
      if(e.action==='bot')selected='B';
      if(e.action==='draft')drafts.set(selected,e.value);
      if(e.action==='send'&&(drafts.get(selected)||'').trim())drafts.set(selected,'');
    }else if(e.kind==='recv'){
      a=F.received(u,json(e.frame));
      for(const c of e.frame.items||[])if(c.$==='MessagePosted'){
        if(!known.has(c.thread))known.set(c.thread,new Set());known.get(c.thread).add(c.msg);
      }
    }else throw Error('unknown simulator event');
    u=F.ui(a);steps++;
    if(!check)continue;
    assert.equal(F.selected(u),selected,'selection remains usable');
    assert.equal(F.draft(u),drafts.get(selected)||'','draft belongs to selection');
    const entries=list(F.entries(u)),ids=entries.filter(e=>kind(e)==='Msg').map(e=>e.id);
    assert.equal(new Set(ids).size,ids.length,'replayed ids never duplicate');
    for(const id of known.get(selected)||[])assert(ids.includes(id),'committed entry lost: '+id);
    for(const c of list(F.commands(a)))if(kind(c)==='Send'){
      const r=JSON.parse(c.text);
      if(r.m==='turn.start'){
        const old=sends.get(r.id);if(old)assert.deepEqual(r,old,'retries preserve send identity');
        sends.set(r.id,r);
      }
    }
    if(render){
      lastTree=plain(F.tree(u));
      const text=[...nodes(lastTree)].filter(n=>n.type==='text').map(n=>n.text).join(' ');
      assert(!/view too large|conversation unavailable/i.test(text),'refusal appeared in view');
      if(selected){assert([...nodes(lastTree)].some(n=>attrs(n).id==='composer'),'selected conversation lost composer');}
      // A read boundary is source-owned. Arrivals below the latest viewport
      // do not create an unread badge when no hold-away action was issued.
      if(!sequence.slice(0,steps).some(e=>e.kind==='act'&&e.action==='hold-away'))assert.equal(F.pill(u),'','at-end unread');
    }
  }
  return {u,steps,sends:sends.size,tree:lastTree};
}

export function generated(seed, count){
  let s=seed>>>0,n=0;const rand=k=>{s=(Math.imul(s,1664525)+1013904223)>>>0;return s%k;};
  const out=[action('select','A')],seen=[];
  for(let i=0;i<count;i++){
    const th=rand(2)?'A':'B';
    switch(rand(11)){
      case 0:out.push(action('select',th));break;
      case 1:out.push(action('draft','draft '+i+' 日本語 🙂'));break;
      case 2:out.push(action('send'));break;
      case 3:out.push(receive({t:'delta',thread:th,text:'stream '+i+' 🙂'}));break;
      case 4: {const m=message(th,'m'+(++n),'reply '+i,'assistant',i+20);seen.push(m);out.push(receive(changes([m])));break;}
      // Canonical changes are exactly once. Reconnect can replay the whole
      // canonical log; duplicate/out-of-order view REPLIES are tested below
      // the UI fold, at ThinHost's actual de-duplication boundary.
      case 5:out.push(receive({t:'log',origin:'synthetic',since:0,items:[...baseline(),...seen]}));break;
      case 6:out.push(action('hold-end',th));break;
      case 7:out.push(receive({t:'log',origin:'synthetic',since:1,items:[]}));break;
      case 8:out.push(receive({t:'reply',id:99999,ok:false,error:'synthetic error'}));break;
      case 9:out.push(action('bot','bot'));break;
      case 10:out.push(receive(changes([{$:'TurnChanged',thread:th,state:'running',at:i+20}])));break;
    }
  }return out;
}

export function properties(F){
  const result={};
  const double=replay(F,[action('select','A'),action('draft','one send'),action('send'),action('send')]);
  assert.equal(double.sends,1,'two send presses cannot resend an emptied draft');result.doubleSendRequests=1;
  let u=replay(F,[action('select','B'),receive(changes([message('B','seen','visible')])),action('hold-away','B')]).u;
  u=F.ui(F.received(u,json(changes([message('B','hidden','','assistant'),message('B','hidden2',' \n','assistant')]))));
  assert.equal(F.pill(u),'','hidden replies cannot produce phantom unread');result.hiddenUnread=true;
  const huge='日本語 pasted article. '.repeat(4000);
  const sent=replay(F,[action('select','A'),action('draft',huge),action('send')],{render:false});
  const preview=list(F.pending(F.fitted(sent.u))).join('');
  assert(preview.length<2048,'fitted pending text must fit view budget');
  result.pendingPreview={originalChars:huge.length,previewChars:preview.length};
  const seq=[action('select','A'),receive(changes(Array.from({length:1500},(_,i)=>message('A','huge'+i,'history '+i))))];
  const big=replay(F,seq);
  assert.equal(list(F.entries(big.u)).length,1501);result.hugeHistoryEntries=1501;
  // Negative controls test the invariant harness, not an alternate implementation.
  assert.throws(()=>replay({...F,selected:()=> 'WRONG'},[action('select','A')]));
  assert.throws(()=>replay({...F,draft:()=> 'LOST'},[action('select','A'),action('draft','kept')]));
  const repro=shrink([action('select','B'),action('select','A'),action('draft','kept')],s=>{
    try{replay({...F,draft:()=> 'LOST'},s);return false;}catch{return true;}
  });assert.equal(repro.length,1);result.negativeControls=3;result.shrunkEvents=repro.length;
  return result;
}

export function fuzz(F){
  const corpus=['','😀','👩🏽‍💻','\ud800','\udc00','\ud800\ud800','\udc00\ud800','x\udbff','\ud800a\udc00',
    '\u0000\n\t','日本語','é é','x'.repeat(131072)];
  for(let i=0;i<64;i++)corpus.push(String.fromCharCode(0xd800+(i*97)%2048)+'🙂'+String.fromCharCode(0xdc00+i));
  let n=0;
  for(const s of corpus){
    const got=plain(F.parse(JSON.stringify({text:s})));assert.equal(got.text,whole(s),'JSON scalar normalization');
    const bytes=F.encode(json({text:s})),decoded=plain(F.decode(bytes));
    assert.equal(decoded.text,whole(s),'CBOR production round trip');n++;
  }
  for(const text of ['{','[1,','{"a":']){
    assert.equal(plain(F.parse(text)),null,'malformed JSON is rejected');n++;
  }
  // Truncated wire inputs are total; no service is started and no live data read.
  for(let i=0;i<24;i++){
    const raw=list(F.encode(json({text:'emoji 😀',n:i}))).slice(0,i%8);
    let xs={$:'Nil'};for(const b of raw.toReversed())xs={$:'Con',head:b,tail:xs};
    F.decode(xs);n++;
  }return n;
}

// Additional malformed-input discovery lane. A finding is a failing result,
// never a baseline exception or eligible simulator receipt.
export function malformedSurvey(F){
  const findings=[];
  for(const text of ['"\\uZZZZ"','"\\u123"','[true false]']){
    try{const got=plain(F.parse(text));if(got!==null)findings.push({input:text,kind:'accepted malformed JSON'});}
    catch{findings.push({input:text,kind:'parser threw'});}
  }return {pass:findings.length===0,findings};
}

async function main(){
  const start=performance.now(),args=process.argv.slice(2),module=args[0];
  assert(module,'usage: ahead_validation.mjs MODULE [--steps N] [--out DIR]');
  const opt=(k,d)=>args.includes(k)?args[args.indexOf(k)+1]:d;
  const F=(await import(pathToFileURL(resolve(module)).href)).default;
  const iterations=Number(opt('--steps','160')),results=[];
  for(const seed of [1,0xdeadbeef,41821]){
    const sequence=generated(seed,iterations);
    try{results.push({seed,...replay(F,sequence),u:undefined,tree:undefined});}
    catch(error){const minimal=shrink(sequence,s=>{try{replay(F,s);return false;}catch(e){return e.message===error.message;}});
      throw Error(error.message+'\nminimal synthetic repro: '+JSON.stringify(minimal));}
  }
  const detail=properties(F),parserCases=fuzz(F);
  const out=opt('--out',null);
  if(out){mkdirSync(out,{recursive:true});let u=replay(F,[action('select','A'),action('draft','Synthetic draft')]).u;
    writeFileSync(resolve(out,'composer.json'),JSON.stringify(plain(F.composer(u)),null,2));
    writeFileSync(resolve(out,'screen.json'),JSON.stringify(plain(F.tree(u)),null,2));}
  const result={pass:true,approach:'simulator',seeds:results.map(r=>({seed:r.seed,steps:r.steps,sends:r.sends})),
    properties:detail,parserCases,seconds:(performance.now()-start)/1000,
    scope:'production Ui.act/recv_act, View.page and JSON/CBOR; transport paging separately; finite generated cases, not exhaustive proof'};
  if(out)writeFileSync(resolve(out,'simulator.json'),JSON.stringify(result,null,2));
  console.log(JSON.stringify(result));
  if(args.includes('--malformed')){const survey=malformedSurvey(F);console.log(JSON.stringify({approach:'malformed-parser-fuzz',...survey}));if(!survey.pass)process.exitCode=1;}
}
if(process.argv[1]&&resolve(process.argv[1])===resolve(new URL(import.meta.url).pathname))await main();
