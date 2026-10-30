// Actual packaged Electron opening/returning/cached-reopen regression. Uses an
// isolated home/display, copied history, and three labeled large tool outputs.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const {spawn}=require('node:child_process'),{once}=require('node:events');
const {_electron}=require(process.env.PLAYWRIGHT_MODULE||'playwright-core'),{Service}=require('../service.cjs');
const [stage,entries,out]=process.argv.slice(2);assert([stage,entries,out].every(path.isAbsolute));assert(!fs.existsSync(out));fs.mkdirSync(out,{recursive:true,mode:0o700});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-opening-electron-')),project=path.join(home,'project'),agents=path.join(home,'agents');
for(const p of[project,agents,home+'/runtime'])fs.mkdirSync(p,{mode:0o700});
for(const name of['claude','codex','grok','muse','agy','opencode','pi','omp'])fs.writeFileSync(agents+'/'+name,'#!/bin/sh\nexit 127\n',{mode:0o755});
const copied=fs.readFileSync(entries,'utf8').trim().split('\n').map(JSON.parse),ids=['history-copy','opening-b','opening-c'];
const events=[{$:'ProjectCreated',id:'fixture',title:'Private opening regression',root:project,at:1}];
for(const[idIndex,id]of ids.entries()){
 events.push({$:'ThreadCreated',id,project:'fixture',title:'Opening fixture '+id,env:'local',branch:'',worktree:project,provider:'claude',model:'sonnet',at:2});
 for(const e of(idIndex?copied.slice(-100):copied))events.push({...e,thread:id,...(e.msg?{msg:e.msg+':'+id}:{}),...(e.act?{act:e.act+':'+id}:{})});
 for(let i=0;i<3;i++)events.push({$:'ActivityLogged',thread:id,act:'large-tool-'+id+'-'+i,tone:'tool',kind:'shell',summary:'Fixture tool output '+i+'\n'+('bounded output text\n'.repeat(900)),at:1793370000+i});
 events.push({$:'MessagePosted',thread:id,msg:'last-'+id,role:'assistant',text:'**Final message '+id+'**\n\nThere is nothing newer below this entry.',at:1793370010});
}
for(const key of['restart.continue','mail.on','telemetry.share','order.enabled','order.estimates'])events.push({$:'SettingSet',key,value:'off',at:1});
for(const key of['onboard.done','onboard.uber'])events.push({$:'SettingSet',key,value:'1',at:1});
fs.writeFileSync(home+'/events.jsonl',events.map(JSON.stringify).join('\n')+'\n',{mode:0o600});
const info=JSON.parse(fs.readFileSync(stage+'/LOCAL.json')),binary=process.env.TIMELINE_SERVICE||stage+'/payload/backplane-serve',version=process.env.TIMELINE_SERVICE_VERSION||info.version;
const result={state:'running',stage,entries,home,errors:[],runs:[],service:{binary,version,sha256:crypto.createHash('sha256').update(fs.readFileSync(binary)).digest('hex')},limits:['Actual packaged Electron on a private X display; user profile/window untouched.','Real copied history plus explicitly synthetic large tool outputs; server frames delayed 180ms to expose opening pictures deterministically.']};
let app,page,xvfb,video,service;
function probe(){
 localStorage.setItem('backplane-selected','history-copy');window.openingProbe={phase:'startup',frames:[]};
 function frame(ms){const tl=document.getElementById('timeline');if(tl){const box=tl.getBoundingClientRect(),buttons=[...tl.querySelectorAll('button')];
 const footer=buttons.filter(e=>!e.closest('.act,.msg')&&(e.getAttribute('data-on-click')==='thin-message-more'||e.textContent.trim()==='More')).filter(e=>{const b=e.getBoundingClientRect();return b.height>0&&b.width>0&&b.bottom>box.top&&b.top<box.bottom;});
 openingProbe.frames.push({ms,phase:openingProbe.phase,thread:document.querySelector('main[data-thread]')?.dataset.thread,ready:document.documentElement.dataset.backplaneRemoteReady,cached:document.documentElement.dataset.backplaneCached,moreFooter:footer.length,footerText:footer.map(e=>e.textContent),top:tl.scrollTop,height:tl.scrollHeight,view:tl.clientHeight,draft:document.getElementById('composer')?.value});}requestAnimationFrame(frame);}requestAnimationFrame(frame);
}
async function phase(name){await page.evaluate(name=>openingProbe.phase=name,name);}
async function ready(id){await page.waitForFunction(id=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.querySelector('main[data-thread]')?.dataset.thread===id&&document.getElementById('composer'),id);}
(async()=>{try{
 xvfb=spawn('/home/h/.local/xvfb-root/usr/bin/Xvfb',['-displayfd','3','-screen','0','2560x1440x24','-nolisten','tcp','-ac'],{stdio:['ignore','pipe','pipe','pipe']});
 const display=await Promise.race([once(xvfb.stdio[3],'data').then(([b])=>':'+b.toString().trim()),new Promise((_,r)=>setTimeout(()=>r(Error('Display timeout')),10000).unref())]);
 const env={...process.env,DISPLAY:display,HOME:home,XDG_RUNTIME_DIR:home+'/runtime',XDG_CONFIG_HOME:home+'/config',XDG_CACHE_HOME:home+'/cache',XDG_DATA_HOME:home+'/data',NO_AT_BRIDGE:'1',CHROME_DEVEL_SANDBOX:'/opt/google/chrome/chrome-sandbox',BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',BACKPLANE_SERVICE:binary,BACKPLANE_WEB:stage+'/payload/web',BACKPLANE_SERVICE_VERSION:version};
 Object.assign(process.env,env);service=new Service({home,binary,web:env.BACKPLANE_WEB,expectedVersion:version});await service.start();
 app=await _electron.launch({executablePath:stage+'/desktop/runtime/backplane-desktop',args:[],env,timeout:60000});page=await app.firstWindow();page.setDefaultTimeout(20000);page.on('pageerror',e=>result.errors.push(String(e)));
 await app.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows()[0].setBounds({x:0,y:0,width:2560,height:1440}));
 await page.addInitScript(probe);await page.routeWebSocket(/.*/,ws=>{const server=ws.connectToServer();server.onMessage(message=>setTimeout(()=>ws.send(message),180));});
 video=spawn('ffmpeg',['-hide_banner','-loglevel','error','-f','x11grab','-framerate','30','-video_size','2560x1440','-i',display,'-c:v','libx264','-preset','ultrafast','-crf','25',out+'/electron.mp4'],{env,stdio:['pipe','ignore','pipe']});
 await page.reload();await ready('history-copy');
 for(let i=0;i<4&&await page.locator('[data-on-click="ob-skip"]').count();i++){await page.locator('[data-on-click="ob-skip"]').click();await page.waitForTimeout(100);}
 await page.waitForFunction(()=>!document.querySelector('.app.tour-on'));await page.waitForTimeout(700);
 const seen=new Set();result.draftsPreserved=true;
 for(const id of['history-copy','opening-b','opening-c','history-copy','opening-c','opening-b','history-copy']){
  await phase('open:'+id);if(id!=='history-copy'||seen.size){await page.locator('[data-on-click="select"][data-value="'+id+'"]').first().click();await ready(id);}
  if(seen.has(id))assert.equal(await page.locator('#composer').inputValue(),'Unsent '+id);
  await page.locator('#composer').fill('Unsent '+id);seen.add(id);await page.waitForTimeout(500);
 }
 await page.waitForTimeout(700);result.runs.push(await page.evaluate(()=>openingProbe));
 await page.reload();await ready('history-copy');await phase('cached-reopen');await page.waitForTimeout(800);assert.equal(await page.locator('#composer').inputValue(),'Unsent history-copy');result.runs.push(await page.evaluate(()=>openingProbe));
 await page.screenshot({path:out+'/electron.png'});assert.equal(result.errors.length,0,result.errors.join('\n'));
 const frames=result.runs.flatMap(r=>r.frames);result.frames=frames.length;result.maxMoreFooter=Math.max(...frames.map(f=>f.moreFooter));result.cachedFrames=frames.filter(f=>f.cached==='true').length;assert(result.cachedFrames>0,'opening picture path exercised');
 if(process.env.EXPECT_MORE_FLASH==='1'){assert(result.maxMoreFooter>=3,'baseline reproduces three More bars');result.state='baseline-reproduced';}else{assert.equal(result.maxMoreFooter,0,'no More footer rows at any opening frame');result.state='accepted';}
}catch(e){result.state='failed';result.error=e.stack;process.exitCode=1;if(page){result.runs.push(await page.evaluate(()=>window.openingProbe).catch(()=>null));await page.screenshot({path:out+'/failure.png'}).catch(()=>{});}}
finally{if(video&&video.exitCode===null){video.stdin.write('q');await Promise.race([once(video,'exit'),new Promise(r=>setTimeout(()=>{video.kill('SIGTERM');r();},3000).unref())]);}if(app)await app.close();if(service)await service.stop();if(xvfb&&xvfb.exitCode===null)xvfb.kill('SIGTERM');fs.writeFileSync(out+'/RESULT.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({state:result.state,error:result.error,frames:result.frames,maxMoreFooter:result.maxMoreFooter,cachedFrames:result.cachedFrames,out}));}
})();
