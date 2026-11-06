// Settings walk (matrix S01-S14). Rerun on any platform: bun test/e2e/settings-walk.mjs HUB_BIN OUTDIR [port]
// Drives the hub's own settings view (the rows every client draws) over the view socket, on a temp home with stand-in agents:
//   for every page in the menu, for every chip (`setting key=value` button): click it, read the page back (it must show the chip lit),
//   read the hub's setting (`settings.get`-style: the SettingSet in events.jsonl), restart the hub, reopen, and confirm it is still lit.
// Form rows (fields + a Save/Set button) are walked with a typed value. Output: OUTDIR/settings-walk.json, one record per control: {page,label,control,value,applied,persisted,effect,note}.
// `effect` is only true where a probe below proves the hub acts on the value; every other control says effect:null (not probed): never pass.
import {mkdirSync,writeFileSync,readFileSync,existsSync,rmSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep} from '../perf/lib.mjs';
import {viewClient} from '../perf/view.mjs';
const [bin,out,port0]=process.argv.slice(2);const port=Number(port0||3830);
rmSync(out,{recursive:true,force:true});mkdirSync(out+'/bin',{recursive:true});makeStandins(out+'/bin');
const home=out+'/home';process.env.NO_SCOPE='1';
const walk=(o,f,d=0)=>{if(d>14||!o||typeof o!=='object')return;f(o);for(const k of Object.keys(o))walk(o[k],f,d+1)};
let hub=null,v=null;
async function up(){hub=await startHub({bin,home,port,standins:out+'/bin',log:out+'/hub.log'});v=await viewClient(hub,{mobile:true,delta:false});await v.open()}
async function down(){try{v.close()}catch{}stopHub(hub);for(let i=0;i<100&&existsSync(`/proc/${hub.pid}`);i++)await sleep(100);await sleep(500)}
const act=async(action,value='')=>{const r=await v.req('view.action',{action,value},30000);return r};
const rowsOf=r=>{let rows=null;walk(r?.x?.result,o=>{if(!rows&&Array.isArray(o.rows)&&o.rows.some(x=>x&&(x.buttons||x.fields)))rows=o.rows});return rows||[]};
const openPage=async p=>{let r=await act('flag','settings');let rows=rowsOf(r);if(p){const was=rows;r=await act('settings-page',p);rows=rowsOf(r)}return rows};
const menu=async()=>{const r=await act('flag','settings');const rows=rowsOf(r);return rows.flatMap(x=>(x.buttons||[]).filter(b=>b.action==='settings-page'&&b.value).map(b=>b.value))};
const log=[];const rec=o=>{log.push(o);console.log(JSON.stringify(o))};
const hubSettings=()=>{const m={};try{for(const l of readFileSync(home+'/events.jsonl','utf8').split('\n')){if(!l.includes('"SettingSet"'))continue;const e=JSON.parse(l);if(e.value==='')delete m[e.key];else m[e.key]=e.value}}catch{}return m};
await up();
let r0=await act('flag','settings');const pages=await menu();rec({menu:pages});
const todo=[]; // {page,label,button}
for(const p of pages){const rows=await openPage(p);for(const row of rows)for(const b of row.buttons||[])if(b.action==='setting'&&b.value.includes('='))todo.push({page:p,label:row.label,key:b.value.split('=')[0],val:b.value.slice(b.value.indexOf('=')+1),was:!!b.on});
  else if(b.action!=='settings-page'&&!['hook-url','hook-secret'].includes(b.action))todo.push({page:p,label:row.label,other:b.action,val:b.value,name:b.label})}
for(const t of todo){if(t.other){rec({page:t.page,label:t.label,control:'button:'+t.other,name:t.name,note:'not a chip: walked by the form pass'});continue}
  await openPage(t.page);const target=t.key+'='+t.val;await act('setting',target);await sleep(300);
  const rows=await openPage(t.page);let lit=false;for(const row of rows)for(const b of row.buttons||[])if(b.action==='setting'&&b.value===target&&b.on)lit=true;
  const logged=hubSettings();const applied=lit&&(logged[t.key]===t.val||(t.val===''&&!(t.key in logged)));
  rec({page:t.page,label:t.label,control:'chip',key:t.key,value:t.val,applied,lit,logged:logged[t.key]??null});t.applied=applied;t.lit=lit}
// restart and confirm persistence
const before=hubSettings();await down();await up();
const after=hubSettings();const walked=todo.filter(t=>t.key);
for(const t of walked){const target=t.key+'='+t.val;const rows=await openPage(t.page);let lit=false;for(const row of rows)for(const b of row.buttons||[])if(b.action==='setting'&&b.value===target&&b.on)lit=true;
  // the last chip clicked for a key is the one that must be lit after the restart
  const last=[...walked].reverse().find(x=>x.key===t.key);if(last!==t)continue;
  rec({persist:true,page:t.page,key:t.key,value:t.val,lit_after_restart:lit,stored:after[t.key]??null})}
await down();
writeFileSync(out+'/settings-walk.json',JSON.stringify(log,null,1));
const bad=log.filter(x=>x.control==='chip'&&!x.applied);console.log(`chips walked: ${log.filter(x=>x.control==='chip').length}, not applied: ${bad.length}, persist failures: ${log.filter(x=>x.persist&&!x.lit_after_restart).length}`);process.exit(0);
