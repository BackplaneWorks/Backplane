// Packaged Electron reproduction: private display/home, real copied history,
// deterministic streaming provider. Never attaches to the user's window/home.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {spawn}=require('node:child_process'),{once}=require('node:events');
const {_electron}=require(process.env.PLAYWRIGHT_MODULE||'playwright-core'),{Service}=require('../service.cjs');
const [stage,entries,out]=process.argv.slice(2);assert([stage,entries,out].every(p=>path.isAbsolute(p)));
assert(!fs.existsSync(out));fs.mkdirSync(out,{recursive:true,mode:0o700});
const home=fs.mkdtempSync(path.join(os.tmpdir(),'bp-timeline-electron-')),project=path.join(home,'project'),agents=path.join(home,'agents');
for(const p of [project,agents,path.join(home,'runtime')])fs.mkdirSync(p,{mode:0o700});
for(const name of ['claude','codex','grok','muse','agy','opencode','pi','omp'])fs.writeFileSync(path.join(agents,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
let standin=fs.readFileSync(path.join(__dirname,'stream-agent.cjs'),'utf8').replace('i<120','i<480').replace('await delay(50)','await delay(70)');
const markdown=process.env.TIMELINE_MARKDOWN==='1';
if(markdown)standin=standin.replace('const text=`sample-${i}(${Date.now()}) The board and the reply stay responsive. `;', 'const text=`\\n\\nsample-${i} **bold-${i}**\\n\\n- [link](https://example.com)\\n- inline \\`code\\`\\n\\n\\`\\`\\`js\\nconst value = ${i};\\n\\`\\`\\`\\n`;');
fs.writeFileSync(path.join(agents,'stream.cjs'),standin);fs.writeFileSync(path.join(agents,'claude'),'#!/bin/sh\nexec '+JSON.stringify(process.execPath)+' '+JSON.stringify(path.join(agents,'stream.cjs'))+' "$@"\n',{mode:0o755});
const copied=fs.readFileSync(entries,'utf8').trim().split('\n').map(JSON.parse).map(e=>({...e,thread:'history-copy'}));
if(markdown)for(let i=0;i<5;i++)copied.push({$:'MessagePosted',thread:'history-copy',msg:'markdown-fixture-'+i,role:'assistant',at:1793298000+i,
 text:`### Markdown fixture ${i}\n\n`+Array.from({length:12},(_,n)=>`**bold-${i}-${n}** with [a link](https://example.com), inline \`code\`, and enough text to exercise the negotiated message body preview.\n\n`).join('')+'- first list item\n- second list item\n\n```js\nconst value = 42;\n```\n'});
const events=[{$:'ProjectCreated',id:'fixture',title:'Private timeline regression',root:project,at:1},{$:'ThreadCreated',id:'history-copy',project:'fixture',title:'Recorded conversation copy',env:'local',branch:'',worktree:project,provider:'claude',model:'sonnet',at:2},...copied,
 ...['restart.continue','mail.on','telemetry.share','order.enabled','order.estimates'].map(key=>({$:'SettingSet',key,value:'off',at:1})),
 ...['onboard.done','onboard.uber'].map(key=>({$:'SettingSet',key,value:'1',at:1}))];
fs.writeFileSync(path.join(home,'events.jsonl'),events.map(JSON.stringify).join('\n')+'\n',{mode:0o600});
const info=JSON.parse(fs.readFileSync(path.join(stage,'LOCAL.json'))),result={state:'running',stage,entries,home,copiedEvents:copied.length,errors:[],phases:[],limits:['Packaged Electron on a private X display; no user window/profile modified.','Deterministic streaming provider, actual copied thread content.']};
const binary=process.env.TIMELINE_SERVICE||stage+'/payload/backplane-serve';
const version=process.env.TIMELINE_SERVICE_VERSION||info.version;
result.service={binary,version,sha256:require('node:crypto').createHash('sha256').update(fs.readFileSync(binary)).digest('hex')};
let app,page,xvfb,video,service;
function probe(){
 localStorage.setItem('backplane-selected','history-copy');window.__bpPerf={render:[],act:[],part:[],live:[],trace:[]};
 window.timelineRegression={phase:'startup',frames:[],events:[],last:null};
 const identities=new WeakMap();let nextIdentity=0;
 const identity=node=>{if(!identities.has(node))identities.set(node,++nextIdentity);return identities.get(node);};
 for(const type of ['wheel','scroll'])document.addEventListener(type,e=>{if(e.target.closest?.('#timeline')||e.target.id==='timeline')timelineRegression.events.push({type,ms:performance.now(),dy:e.deltaY,top:document.getElementById('timeline')?.scrollTop});},{capture:true,passive:true});
 function frame(ms){const tl=document.getElementById('timeline');if(tl){const b=tl.getBoundingClientRect(),track=tl.querySelector('.timeline-virtual-track');
 const rows=[...tl.querySelectorAll('.timeline-virtual-row')].map(e=>{const r=e.getBoundingClientRect();return {key:e.dataset.virtualKey,y:r.top-b.top,h:r.height,wrapper:identity(e),node:e.firstChild?identity(e.firstChild):null,text:e.textContent.slice(0,80)};});
 timelineRegression.frames.push({ms,phase:timelineRegression.phase,top:tl.scrollTop,height:tl.scrollHeight,view:tl.clientHeight,track:track?.style.cssText,rows,status:document.getElementById('history-status')?.textContent,live:!!tl.querySelector('.live'),draft:document.getElementById('composer')?.value,
 rawMessageParts:tl.querySelectorAll('.msg pre.body-message-part').length,markdownParts:tl.querySelectorAll('.msg .body-message-part strong,.msg .body-message-part ul,.msg .body-message-part code').length,
 streamMarkdown:tl.querySelectorAll('[data-virtual-key^="@live:"] strong,[data-virtual-key^="live:"] strong').length});}requestAnimationFrame(frame);}
 requestAnimationFrame(frame);
}
async function phase(name,run){result.phases.push({name,at:Date.now()});await page.evaluate(name=>timelineRegression.phase=name,name);await run();}
async function pause(ms){await page.waitForTimeout(ms);}
(async()=>{try{
 xvfb=spawn('/home/h/.local/xvfb-root/usr/bin/Xvfb',['-displayfd','3','-screen','0','2560x1440x24','-nolisten','tcp','-ac'],{stdio:['ignore','pipe','pipe','pipe']});
 const display=await Promise.race([once(xvfb.stdio[3],'data').then(([b])=>':'+b.toString().trim()),new Promise((_,r)=>setTimeout(()=>r(Error('Display startup timeout')),10000).unref())]);result.display=display;
 const env={...process.env,DISPLAY:display,HOME:home,XDG_RUNTIME_DIR:home+'/runtime',XDG_CONFIG_HOME:home+'/config',XDG_CACHE_HOME:home+'/cache',XDG_DATA_HOME:home+'/data',NO_AT_BRIDGE:'1',CHROME_DEVEL_SANDBOX:'/opt/google/chrome/chrome-sandbox',BACKPLANE_HOME:home,BACKPLANE_AGENTS:agents,BACKPLANE_COPY:'1',BACKPLANE_KISTACK_SYNC:'0',BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:home+'/locks',BP_AGENT_TRACE:out+'/agent.jsonl',BACKPLANE_SERVICE:binary,BACKPLANE_WEB:stage+'/payload/web',BACKPLANE_SERVICE_VERSION:version};
 Object.assign(process.env,env);service=new Service({home,binary,web:env.BACKPLANE_WEB,expectedVersion:version});await service.start();
 app=await _electron.launch({executablePath:stage+'/desktop/runtime/backplane-desktop',args:[],env,timeout:60000});page=await app.firstWindow();page.setDefaultTimeout(20000);page.on('pageerror',e=>result.errors.push(String(e)));
 await app.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows()[0].setBounds({x:0,y:0,width:2560,height:1440}));
 await page.addInitScript(probe);await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true');await page.reload();
 await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true'&&document.getElementById('composer'));
 for(let i=0;i<4&&await page.locator('[data-on-click="ob-skip"]').count();i++){await page.locator('[data-on-click="ob-skip"]').click();await pause(100);}
 await page.waitForFunction(()=>!document.querySelector('.app.tour-on'));await pause(300);
 video=spawn('ffmpeg',['-hide_banner','-loglevel','error','-f','x11grab','-framerate','30','-video_size','2560x1440','-i',display,'-c:v','libx264','-preset','ultrafast','-crf','25',out+'/electron.mp4'],{env,stdio:['pipe','ignore','pipe']});
 const box=await page.locator('#timeline').boundingBox();await page.mouse.move(box.x+box.width/2,box.y+box.height/2);
 await phase('idle-initial',()=>pause(1200));
 await page.locator('#composer').fill('Unsent 日本語 draft');await page.evaluate(()=>{window.keptComposer=document.getElementById('composer');keptComposer.setSelectionRange(3,8);});
 await phase('wheel-up',async()=>{for(let i=0;i<100;i++){await page.mouse.wheel(0,-120);await pause(55);}});
 await phase('idle-after-up',()=>pause(1800));
 await phase('wheel-down',async()=>{for(let i=0;i<110;i++){await page.mouse.wheel(0,140);await pause(55);}});
 await phase('idle-after-down',()=>pause(1200));
 result.draftPreserved=await page.evaluate(()=>keptComposer===document.getElementById('composer')&&keptComposer.value==='Unsent 日本語 draft'&&keptComposer.selectionStart===3&&keptComposer.selectionEnd===8);
 await page.locator('#composer').fill('Stream the deterministic test reply');await page.locator('#composer').press('Enter');
 await page.waitForFunction(()=>document.getElementById('timeline')?.textContent.includes('sample-'));
 await phase('stream-bottom',()=>pause(2200));
 await phase('stream-wheel-up',async()=>{for(let i=0;i<35;i++){await page.mouse.wheel(0,-110);await pause(55);}});
 await phase('stream-reading-idle',()=>pause(2200));
 await phase('stream-wheel-down',async()=>{for(let i=0;i<60;i++){await page.mouse.wheel(0,160);await pause(55);}});
 await phase('explicit-latest',async()=>{
   const latest=page.locator('[data-on-click="hold-bottom"]').first();
   if(await latest.count()) await latest.click({force:true});
   await page.locator('#timeline').evaluate(el=>{el.scrollTop=el.scrollHeight;});
   await pause(300);
 });
 await phase('stream-bottom-again',()=>pause(2200));
 result.observed=await page.evaluate(()=>({...timelineRegression,perf:__bpPerf,css:{overflowAnchor:getComputedStyle(document.getElementById('timeline')).overflowAnchor,scrollBehavior:getComputedStyle(document.getElementById('timeline')).scrollBehavior}}));
 await page.screenshot({path:out+'/electron.png'});assert.equal(result.errors.length,0,result.errors.join('\n'));assert(result.draftPreserved);result.state='captured';
 if(markdown){
  const frames=result.observed.frames.filter(f=>f.phase!=='startup');assert(frames.every(f=>f.rawMessageParts===0),'message body must never render a plain-text PRE fallback');assert(frames.some(f=>f.markdownParts>0),'Markdown semantics exercised');
  assert(frames.some(f=>f.phase.startsWith('stream-')&&f.streamMarkdown>0),'streamed Markdown semantics exercised');
  for(let i=1;i<frames.length;i++){const old=new Map(frames[i-1].rows.map(r=>[r.key,r]));for(const row of frames[i].rows){const prior=old.get(row.key);if(prior){assert.equal(row.wrapper,prior.wrapper,'overlapping row wrapper replaced: '+row.key);assert.equal(row.node,prior.node,'overlapping row node replaced: '+row.key);}}}
  result.markdown={plainFallbackFrames:0,structuredFrames:frames.filter(f=>f.markdownParts>0).length,streamStructuredFrames:frames.filter(f=>f.streamMarkdown>0).length,overlappingRowIdentityKept:true};
 }
}catch(e){result.state='failed';result.error=e.stack;process.exitCode=1;if(page){result.observed=await page.evaluate(()=>window.timelineRegression).catch(()=>null);await page.screenshot({path:out+'/failure.png'}).catch(()=>{});}}
finally{if(video&&video.exitCode===null){video.stdin.write('q');await Promise.race([once(video,'exit'),new Promise(r=>setTimeout(()=>{video.kill('SIGTERM');r();},3000).unref())]);}if(app)await app.close();if(service)await service.stop();if(xvfb&&xvfb.exitCode===null)xvfb.kill('SIGTERM');fs.writeFileSync(out+'/RESULT.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({state:result.state,error:result.error,frames:result.observed?.frames?.length,out}));}
})();
