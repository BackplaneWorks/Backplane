// Settings walk (matrix S01-S14). Rerun on any platform: bun test/e2e/settings-walk.mjs HUB_BIN OUTDIR [port]
// Drives the hub's own settings view (the rows every client draws) over the view socket, on a temp home with stand-in agents:
//   for every page in the menu, for every chip (`setting key=value` button): click it, read the page back (it must show the chip lit),
//   read the hub's setting (`settings.get`-style: the SettingSet in events.jsonl), restart the hub, reopen, and confirm it is still lit.
// Form rows (fields + a Save/Set button) are walked with a typed value. Output: OUTDIR/settings-walk.json, one record per control: {page,label,control,value,applied,persisted,effect,note}.
// `effect` is only true where a probe below proves the hub acts on the value; every other control says effect:null (not probed): never pass.
import {mkdirSync,writeFileSync,readFileSync,existsSync,rmSync} from 'node:fs';
import {makeStandins,startHub,stopHub,sleep,pidAlive} from '../perf/lib.mjs';
import {viewClient} from '../perf/view.mjs';
import {encode} from '../wire.mjs';
const [bin,out,port0]=process.argv.slice(2);const port=Number(port0||3830);
rmSync(out,{recursive:true,force:true});mkdirSync(out+'/bin',{recursive:true});makeStandins(out+'/bin');
const home=out+'/home';process.env.NO_SCOPE='1';
const walk=(o,f,d=0)=>{if(d>14||!o||typeof o!=='object')return;f(o);for(const k of Object.keys(o))walk(o[k],f,d+1)};
let hub=null,v=null;
async function up(){hub=await startHub({bin,home,port,standins:out+'/bin',log:out+'/hub.log'});v=await viewClient(hub,{mobile:true,delta:false,native:true});await v.open();await act('flag','settings')}
async function down(){try{v.close()}catch{}stopHub(hub);for(let i=0;i<100&&pidAlive(hub.pid);i++)await sleep(100);await sleep(500)}
// the presenter answers with `cmds` the client carries out (type send: an RPC to the hub); do what the client does
const act=async(action,value='')=>{const r=await v.req('view.action',{action,value},30000);
  for(const c of r?.x?.result?.cmds||[])if(c.type==='send'){try{v.ws.send(encode(JSON.parse(c.text)))}catch(e){console.error('cmd send failed',e)}await sleep(150)}
  return r};
const rowsOf=r=>r?.x?.result?.shown?.settings?.rows||[];
const openPage=async p=>rowsOf(await act('settings-page',p||''));
const menu=async()=>(await openPage('')).flatMap(x=>(x.buttons||[]).filter(b=>b.action==='settings-page'&&b.value).map(b=>b.value));
const log=[];const rec=o=>{log.push(o);console.log(JSON.stringify(o))};
const hubSettings=()=>{const m={};try{for(const l of readFileSync(home+'/events.jsonl','utf8').split('\n')){if(!l.includes('"SettingSet"'))continue;const e=JSON.parse(l);if(e.value==='')delete m[e.key];else m[e.key]=e.value}}catch{}return m};
await up();
const pages=await menu();rec({menu:pages});
const todo=[]; // {page,label,button}
for(const p of pages){const rows=await openPage(p);for(const row of rows)for(const b of row.buttons||[])if(b.action==='telemetry-on')todo.push({page:p,label:row.label,key:'telemetry.share',val:'on',was:!!b.on,via:'telemetry-on'});
  else if(b.action==='setting'&&b.value.includes('='))todo.push({page:p,label:row.label,key:b.value.split('=')[0],val:b.value.slice(b.value.indexOf('=')+1),was:!!b.on});
  else if(b.action!=='settings-page'&&!['hook-url','hook-secret'].includes(b.action))todo.push({page:p,label:row.label,other:b.action,val:b.value,name:b.label})}
const first=[];const lastOn=[];
for(const t of todo.filter(x=>x.key&&x.was))lastOn.push(t);
const gate=x=>x.key==='order.enabled'; // turning Ordering off hides the chips after it
todo.splice(0,todo.length,...todo.filter(x=>(!x.was||!x.key)&&!gate(x)),...todo.filter(x=>(!x.was||!x.key)&&gate(x)),...lastOn);
for(const t of todo){if(t.other){rec({page:t.page,label:t.label,control:'button:'+t.other,name:t.name,note:'not a chip: walked by the form pass'});continue}
  const pre=await openPage(t.page);const target=t.key+'='+t.val;
  if(t.via){let tv='';for(const row of pre)for(const b of row.buttons||[])if(b.action===t.via)tv=b.value;await act(t.via,tv)}else await act('setting',target);await sleep(300);
  const rows=await openPage(t.page);let lit=false;for(const row of rows)for(const b of row.buttons||[])if(((b.action==='setting'&&b.value===target)||(t.via&&b.action===t.via))&&b.on)lit=true;
  const logged=hubSettings();const actionChip=['telemetry.reset','fallback.chain'].includes(t.key)||t.key.startsWith('fallback.add.'); // an action, not a toggle: never lit; it must be stored
  const applied=(actionChip&&logged[t.key]===t.val)||lit&&(logged[t.key]===t.val||(t.val===''&&!(t.key in logged))||(t.was&&!(t.key in logged))); // a value that was already the default is not logged
  rec({page:t.page,label:t.label,control:'chip',key:t.key,value:t.val,applied,lit,logged:logged[t.key]??null});t.applied=applied;t.lit=lit}

// form controls: type into the draft the client keeps, press the button, read the page and the hub back; verified again after the restart
const forms=[
 {page:'timezone',field:'tzname',text:'Europe/Helsinki',button:'tz-set',key:'person.tz',want:'Europe/Helsinki',shows:'Europe/Helsinki',label:'Time zone'},
 {page:'cad',field:'kpath',text:'/usr/bin/kicad-cli',button:'kicad-path',key:'kicad.path',want:'/usr/bin/kicad-cli',shows:'',label:'KiCad path'},
 {page:'threads',field:'whurl',text:'https://example.com/hook',button:'hook-url',key:'push.webhook.url',want:'https://example.com/hook',shows:'https://example.com/hook',label:'Webhook address'}];
for(const f of forms){await openPage(f.page);await act('bfield',f.field+'\u001f'+f.text); // what the client sends as the field changes
  await act(f.button,'');await sleep(400);
  const st=hubSettings();const rows=await openPage(f.page);const shown=JSON.stringify(rows);
  rec({page:f.page,label:f.label,control:'form',key:f.key,value:f.want,applied:st[f.key]===f.want&&(!f.shows||shown.includes(f.shows)),stored:st[f.key]??null})}
{const rows=await openPage('devices');const b=rows.flatMap(r=>r.buttons||[]).find(b=>b.action==='device-invite');if(b){await act('device-invite','');await sleep(800);const after=JSON.stringify(await openPage('devices'));rec({page:'devices',label:'Add a device',control:'form',applied:/Pairing link|pair/i.test(after)&&after!==JSON.stringify(rows),note:'an invitation row appears'})}}
// restart and confirm persistence
const before=hubSettings();await down();await up();
const after=hubSettings();const walked=todo.filter(t=>t.key);
for(const t of walked){const target=t.key+'='+t.val;const rows=await openPage(t.page);let lit=false;for(const row of rows)for(const b of row.buttons||[])if(((b.action==='setting'&&b.value===target)||(t.via&&b.action===t.via))&&b.on)lit=true;
  // the last chip clicked for a key is the one that must be lit after the restart
  const last=[...walked].reverse().find(x=>x.key===t.key);if(last!==t)continue;
  const act2=['telemetry.reset','fallback.chain'].includes(t.key)||t.key.startsWith('fallback.add.');if(act2)lit=after[t.key]===t.val;
  rec({persist:true,page:t.page,key:t.key,value:t.val,lit_after_restart:lit,stored:after[t.key]??null})}
for(const f of forms){await openPage(f.page);const st=hubSettings();const shown=JSON.stringify(await openPage(f.page));rec({persist:true,page:f.page,key:f.key,value:f.want,lit_after_restart:st[f.key]===f.want&&(!f.shows||shown.includes(f.shows)),stored:st[f.key]??null})}
await down();
writeFileSync(out+'/settings-walk.json',JSON.stringify(log,null,1));
const bad=log.filter(x=>(x.control==='chip'||x.control==='form')&&!x.applied);console.log(`controls walked: ${log.filter(x=>x.control==='chip'||x.control==='form').length}, not applied: ${bad.length}, persist failures: ${log.filter(x=>x.persist&&!x.lit_after_restart).length}`);process.exit(0);
