// Focused source-only physical-row bound controls. No app/helper compilation.
import fs from 'node:fs';import os from 'node:os';import path from 'node:path';import assert from 'node:assert/strict';import {pathToFileURL} from 'node:url';
// Hermetic by default: without the frozen review packet (build/phone-cache), check the repo's own sources.
if(!process.env.BP_NATIVE_THIN_SOURCE_ROOT&&!fs.existsSync(path.join(process.cwd(),'build/phone-cache/native-thin-bridge-proposal-01')))process.env.BP_NATIVE_THIN_SOURCE_ROOT=process.cwd();
const root=process.env.BP_NATIVE_THIN_SOURCE_ROOT||process.cwd(),stage=fs.mkdtempSync(path.join(os.tmpdir(),'bp-native-rows-307265-'));
const donor=process.env.BP_NATIVE_THIN_SOURCE_ROOT?root:path.join(root,'build/phone-cache/native-thin-bridge-proposal-01/qualified-dependencies');
for(const n of ['src/web/thin.js','src/web/thin-results.js','src/web/startup.js','src/web/mail-drafts.js','src/web/terminal-input.js','src/web/model-menu-input.js','src/web/sidebar-region.js']){const q=path.join(stage,n);fs.mkdirSync(path.dirname(q),{recursive:true});fs.copyFileSync(path.join(donor,n),q);}
const helper=process.env.BP_NATIVE_THIN_HELPER_SOURCE||path.join(root,'src/mobile/thin-native-host-307265.js');
fs.mkdirSync(path.join(stage,'src/mobile'),{recursive:true});fs.copyFileSync(helper,path.join(stage,'src/mobile/thin-native-host-307265.js'));fs.writeFileSync(path.join(stage,'package.json'),'{"type":"module"}');
const {validateNativeResult307265:valid}=await import(pathToFileURL(path.join(stage,'src/mobile/thin-native-host-307265.js')));
const clone=x=>JSON.parse(JSON.stringify(x));
function fixture(count=32,live=true){
 const rows=[];for(let i=0;i<count;i++){rows.push({id:'m'+i+':from',kind:'from',text:'linked origin',blocks:[]});rows.push({id:'m'+i,kind:'assistant',text:'current text',blocks:[]});}
 if(live)rows.push({id:'@live:t',kind:'assistant',text:'bounded live',blocks:[]});
 const shown={hub:'h',sel:'h|t',online:true,version:'fixture',error:'',note:'',empty:'',hubs:[],found:[],projects:[],bots:[],rooms:[],thread:{id:'h|t',entries:rows}};
 return{remoteReady:true,session:{sel:'t',drafts:{},rid:1,open:{},info:{},adding:false,panel:false},shown,overlay:{key:'h',name:'hub',j:clone(shown)},window:{ids:Array.from({length:count},(_,i)=>String(i+1)),older:'',hasMore:false,anchor:{id:'m0',offset:0},partial:[]},cmds:[]};
}
const input={hub:'h',target:{kind:'thread',id:'t'}},tests=[
 ['65 physical rows from32 logical entries plus selected live accepted',()=>{const r=fixture();assert.equal(r.shown.thread.entries.length,65);assert(valid(r,input));}],
 ['64 ordinary physical rows need no synthetic live row',()=>assert(valid(fixture(32,false),input))],
 ['17 linked messages may render34 normal physical rows',()=>assert(valid(fixture(17,false),input))],
 ['66 shown physical rows rejected',()=>{const r=fixture();r.shown.thread.entries.push({id:'extra',kind:'act',text:'extra'});assert(!valid(r,input));}],
 ['66 overlay physical rows rejected independently',()=>{const r=fixture();r.overlay.j.thread.entries.push({id:'extra',kind:'act',text:'extra'});assert(!valid(r,input));}],
 ['duplicate selected synthetic live row rejected below row cap',()=>{const r=fixture(1,true);r.shown.thread.entries.push(clone(r.shown.thread.entries.at(-1)));assert(!valid(r,input));}],
 ['foreign synthetic live owner rejected below row cap',()=>{const r=fixture(1,true);r.shown.thread.entries.at(-1).id='@live:other';assert(!valid(r,input));}],
 ['foreign qualified thread owner cannot authorize selected live row',()=>{const r=fixture(1,true);r.shown.thread.id='other|t';assert(!valid(r,input));}],
 ['qualified synthetic id cannot alias raw server live identity',()=>{const r=fixture(1,true);r.shown.thread.entries.at(-1).id='@live:h|t';assert(!valid(r,input));}],
 ['33 logical IDs still rejected',()=>{const r=fixture();r.window.ids.push('33');assert(!valid(r,input));}],
 ['34 partial records still rejected',()=>{const r=fixture();r.window.partial=Array.from({length:34},(_,i)=>({id:'m'+i,version:'1',cursor:'c'+i,reason:'byte-limit',hasMore:true,complete:false}));assert(!valid(r,input));}],
 ['existing nesting budget unchanged',()=>{const r=fixture();let value={};r.shown.extra=value;for(let i=0;i<65;i++)value=value.next={};assert(!valid(r,input));}],
];
let failed=0;try{for(const[name,run]of tests){try{run();console.log('PASS',name);}catch(error){failed++;console.log('FAIL',name,error.message);}}console.log(JSON.stringify({checks:tests.length,failed,scope:'Native physical-row source bound only; logical history32/partial33 unchanged; no emitted/native/service/UI acceptance'}));}finally{fs.rmSync(stage,{recursive:true,force:true});}
if(failed)process.exitCode=1;
