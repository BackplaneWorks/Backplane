// One real packaged-shell check of the new passive capture, no disconnect injection.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {_electron}=require('playwright-core'),{Service}=require('../service.cjs');
const stage=path.resolve(process.env.BACKPLANE_LOCAL_STAGE),info=JSON.parse(fs.readFileSync(path.join(stage,'LOCAL.json')));
const out=process.env.CONNECTION_CAPTURE_OUT||path.join(stage,'connection-capture');fs.mkdirSync(out,{recursive:true});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-connection-capture-')),agents=path.join(home,'agents');fs.mkdirSync(agents);
for(const n of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(path.join(agents,n),'#!/bin/sh\nexit 127\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),ev=($,x)=>({$,...x,at});
fs.writeFileSync(path.join(home,'events.jsonl'),[ev('ProjectCreated',{id:'p',title:'Capture fixture',root:home}),ev('ThreadCreated',{id:'t',title:'Capture',project:'p',env:'local',branch:'',worktree:home,provider:'codex',model:'gpt-6.1-sol'}),ev('MessagePosted',{thread:'t',msg:'m',role:'user',text:'PRIVATE_BODY_MUST_NOT_APPEAR_IN_DIAGNOSTIC'}),...['mail.on','telemetry.share','restart.continue'].map(key=>ev('SettingSet',{key,value:'off'})),ev('SettingSet',{key:'onboard.done',value:'1'})].map(JSON.stringify).join('\n')+'\n');
Object.assign(process.env,{BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:path.join(home,'locks')});
const binary=path.join(stage,'payload/backplane-serve'),web=path.join(stage,'payload/web'),service=new Service({home,binary,web,expectedVersion:info.version});
let app;const errors=[],start=performance.now();
(async()=>{try{
 await service.start();app=await _electron.launch({executablePath:path.join(stage,'desktop/runtime/backplane-desktop'),args:[],env:{...process.env,BACKPLANE_HOME:home,BACKPLANE_SERVICE:binary,BACKPLANE_WEB:web,BACKPLANE_SERVICE_VERSION:info.version}});
 const page=await app.firstWindow();page.on('pageerror',e=>errors.push(e.name));
 await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');
 const file=path.join(home,'connection-log.json');
 const rows=await new Promise((resolve,reject)=>{
  let timer;const watch=fs.watch(home,()=>read());
  function read(){try{const r=JSON.parse(fs.readFileSync(file));if(r.some(x=>x.event==='ready')){watch.close();clearTimeout(timer);resolve(r);}}catch{}}
  timer=setTimeout(()=>{watch.close();reject(Error('Native capture did not receive ready record'));},5000);read();
 });
 for(const name of ['created','open','greeting','ready'])assert(rows.some(x=>x.event===name),name+' captured by actual Electron main');
 assert(rows.every(x=>!['closed','error','renderer-gone'].includes(x.event)));assert.equal(fs.statSync(file).mode&0o777,0o600);
 const stored=await page.evaluate(()=>localStorage.getItem('bp.connection.log.v1'));
 assert(stored&&JSON.parse(stored).some(x=>x.event==='ready'));assert(!JSON.stringify(rows).includes('PRIVATE_BODY'));assert.deepEqual(errors,[]);
 const result={state:'passed',source:info.commit,version:info.version,seconds:(performance.now()-start)/1000,events:rows,localRing:true,nativeFileMode:'0600',unintendedCloses:0,home};
 fs.writeFileSync(path.join(out,'RESULT.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}catch(e){console.error(e);process.exitCode=1;}finally{if(app)await app.close();service.stop();}})();
