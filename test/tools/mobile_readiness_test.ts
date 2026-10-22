import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {foregroundPending, presentationUpdates} from '../../src/mobile/recent.js';
let checks=0;const check=(name:string,f:()=>void)=>{f();checks++;console.log('ok '+name);};
check('background pending hub never blocks selected hub',()=>assert.equal(foregroundPending(new Set(['B']),'A'),false));
check('focused pending hub stays pending',()=>assert.equal(foregroundPending(new Set(['A','B']),'A'),true));
check('current presentation has distinct readiness marker',()=>assert.deepEqual(presentationUpdates('A',{current:true}),[{hub:'A',phase:'presented',detail:'current-presentation'}]));
check('superseded presentation has no readiness marker',()=>assert.deepEqual(presentationUpdates('A',{current:false}),[]));
check('missing current proof cannot become presented',()=>assert.deepEqual(presentationUpdates('A',{}),[]));
check('failure cannot emit presented even with current true',()=>assert.deepEqual(presentationUpdates('A',{current:true,failed:'refused'}),[{hub:'A',phase:'failed',detail:'refused'}]));
check('canonical terminal keeps distinct live meaning',()=>assert.deepEqual(presentationUpdates('A',{live:true,current:true}),[{hub:'A',phase:'live'}]));

// Execute the actual bridge transport functions with narrow Bend/IO stubs.
// This verifies barrier/marker wiring, not an emitted bridge or native UI.
const source=readFileSync('src/mobile/bridge.js','utf8');
const between=(a:string,b:string)=>{const i=source.indexOf(a);assert.ok(i>=0,a);const j=source.indexOf(b,i+a.length);assert.ok(j>i,b);return source.slice(i,j);};
const focus=between('const foregroundHub =','function field(');
const screen=between('function screenText() {','// a frame that could change');
const out=between('function out(cmds, quiet, alerts) {','// a step\'s answer:');
const recvStart=source.indexOf('  recv(k, data, epochOrQuiet, quietOrOwe, owe) {');assert.ok(recvStart>0);
const recvEnd=source.indexOf('\n  },',recvStart);assert.ok(recvEnd>recvStart);
const recv='function '+source.slice(recvStart+2,recvEnd+4)+'\n';
const ended=between('    if (ended) {','    const shows = App.shows');
const box:any=vm.createContext({foregroundPending,presentationUpdates,console});
vm.runInContext(`
var previewCur='A',hubs={cur:'A'},booting=new Set(),transportEpoch=new Map([['A','epoch-new']]);
var lastPresentation=null,lastPresentationHub='',half=null,focusOf=null,focusText='null',nativeLocale='en',owed=false,syncUpdates=[];
var App={current:h=>h.cur,lean:h=>h,screen_list_in:()=>({hub:'A',sel:'A-thread'}),focus:()=>({text:'actual selected cached body'}),live_text:()=>'',alerts:()=> '[]'};
var plain=x=>x,same=(a,b)=>a===b,liveNodes=()=>[];
var recent={links:new Map(),overlays:()=>[],end:()=>[]};
var commandList=cs=>cs;
// a hub that is not on the thin view lane (the transport these checks cover)
var thin={thin:()=>false,views:()=>null,drain:()=>({cmds:[],sync:[]}),inert:()=>false,receive(){},fail(){},disconnect(){}};
var direct=(cmds,draw,updates=[])=>({cmds,draw,updates,screen:draw?JSON.parse(screenText()):null});
${focus}
${screen}
${out}
${recv}
function finish(k,r){var ended=true;${ended}}
`,box);
const run=(s:string)=>vm.runInContext(s,box);
check('actual bridge out draws A while only B bootstraps',()=>{run("booting=new Set(['B'])");assert.equal(run('out([],false,[]).draw'),true);});
check('actual selected content remains in drawn screen',()=>assert.equal(run('out([],false,[]).screen.thread.text'),'actual selected cached body'));
check('actual bridge hides partial A even with background B overlay',()=>{run("booting=new Set(['A','B']);recent.overlays=()=>[{key:'B'}];recent.links=new Map([['B',{shown:{key:'B'}}]])");assert.equal(run('out([],false,[]).draw'),false);});
check('actual foreground cached presentation remains usable',()=>{run("lastPresentationHub='A';lastPresentation=JSON.stringify({hub:'A',sel:'A-thread',cached:true})");assert.equal(run('out([],false,[]).screen.cached'),true);});
check('actual cached presentation never crosses hub identity',()=>{run("lastPresentationHub='B'");assert.equal(run('out([],false,[]).draw'),false);});
check('actual A canonical terminal renders before B terminal',()=>{run("recent.overlays=()=>[];recent.links=new Map();lastPresentation=null;lastPresentationHub='';booting=new Set(['A','B'])");assert.equal(run("finish('A',{hubs:{cur:'A'},cmds:[]}).draw"),true);assert.equal(run("booting.has('B')"),true);});
check('actual canonical terminal reports live only for its own hub',()=>assert.equal(run("JSON.stringify(syncUpdates)"),JSON.stringify([{hub:'A',phase:'live'}])));
check('actual obsolete native connection epoch returns before decode',()=>{box.decode=()=>{throw Error('obsolete frame was decoded')};assert.equal(run("recv('A','bad','epoch-old',false,false).draw"),false);});
const reply=between('      if (answer) {','      recent.ack(k,');
vm.runInContext(`function finishReply(k,answer){${reply}}`,box);
check('actual successful reply wires presented marker',()=>{const v:any=run("finishReply('A',{current:true,cmds:[]})");assert.equal(v.updates[0].phase,'presented');assert.equal(v.draw,true);});
check('actual superseded reply wires no marker and no screen',()=>{const v:any=run("finishReply('A',{current:false,cmds:[]})");assert.equal(v.updates.length,0);assert.equal(v.draw,false);});
check('actual background reply cannot replace selected native screen',()=>{const v:any=run("finishReply('B',{current:true,cmds:[]})");assert.equal(v.draw,false);assert.equal(v.updates[0].hub,'B');});
console.log('PASS '+checks+' focused transport readiness/source-adapter checks; not emitted/native runtime acceptance');
