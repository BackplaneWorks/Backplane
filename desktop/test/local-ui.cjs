// One real app acceptance run, with generated data and only captured children.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {execFileSync}=require('node:child_process'),{createHash}=require('node:crypto');
const {_electron}=require('playwright-core');const {Service}=require('../service.cjs');
const stage=process.env.BACKPLANE_LOCAL_STAGE;
assert(stage&&path.isAbsolute(stage));
const out=path.join(stage,fs.existsSync(path.join(stage,'evidence'))?'evidence-'+Date.now():'evidence');fs.mkdirSync(out);
const home=fs.mkdtempSync(path.join(os.tmpdir(),'backplane-polish-'));
const project=path.join(home,'project');fs.mkdirSync(project);
const fake=path.join(home,'agents');fs.mkdirSync(fake);
for(const name of ['claude','codex','grok','muse','agy','opencode'])fs.writeFileSync(path.join(fake,name),'#!/bin/sh\nexit 127\n',{mode:0o755});
const at=Math.floor(Date.now()/1000),event=($,fields)=>({$,...fields,at});
const tid='t1792700806-414704';
const thread=(id,title)=>event('ThreadCreated',{id,title,project:'polish',env:'local',branch:'',worktree:project,provider:'codex',model:'gpt-6.1-sol'});
const link='[Backplane polish — fast local testing, faster CI, icons and app chrome](backplane://thread/'+tid+')';
const events=[event('ProjectCreated',{id:'polish',title:'Local polish fixture',root:project}),
 thread(tid,'Backplane polish'),thread('other','Other conversation'),thread('bub-thread','Bub Chat'),
 event('BotSet',{id:'bub',name:'Bub',thread:'bub-thread',look:0,persona:''}),event('UberSet',{bot:'bub',uid:'local-polish-bub'}),
 ...['restart.continue','mail.on','telemetry.share'].map(key=>event('SettingSet',{key,value:'off'})),
 event('SettingSet',{key:'onboard.done',value:'1'}),
 event('MessagePosted',{thread:tid,msg:'welcome',role:'assistant',text:'Destination thread. **Local fixture only.**'}),
 event('MessagePosted',{thread:'other',msg:'other-msg',role:'assistant',text:'A second conversation with a separate draft.'}),
 event('MessagePosted',{thread:'bub-thread',msg:'plain-link',role:'assistant',text:link}),
 event('MessagePosted',{thread:'bub-thread',msg:'bold-link',role:'assistant',text:link.replace('[Backplane','[**Backplane').replace('chrome]','chrome**]')}),
 event('MessagePosted',{thread:'bub-thread',msg:'literal',role:'assistant',text:'[unsafe](javascript:alert(1)) and <script>bad</script>; [External](https://example.com) is still a web link.'})];
fs.writeFileSync(path.join(home,'events.jsonl'),events.map(JSON.stringify).join('\n')+'\n');
Object.assign(process.env,{BACKPLANE_AGENTS:fake,BACKPLANE_NO_UPDATE:'1',BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9',BACKPLANE_TELEMETRY:'off',BACKPLANE_COPY:'1',BACKPLANE_PEERS:'',BACKPLANE_LOCK_DIR:path.join(home,'locks')});
const service=new Service({home,binary:process.env.BACKPLANE_SERVICE,web:process.env.BACKPLANE_WEB,expectedVersion:process.env.BACKPLANE_SERVICE_VERSION});
let app,page;const errors=[],observations=[],traffic=[],start=performance.now();
(async()=>{
 try{
  await service.start();
  app=await _electron.launch({executablePath:path.join(stage,'desktop/runtime/backplane-desktop'),args:[],env:{...process.env,BACKPLANE_HOME:home},timeout:60000});
  page=await app.firstWindow();page.on('pageerror',e=>errors.push(e.message));
  page.setDefaultTimeout(15000);
  const record=(kind,value)=>{if(traffic.length<300)traffic.push({kind,value});};
  page.on('websocket',ws=>{ws.on('framesent',f=>record('sent',String(f.payload)));ws.on('framereceived',f=>record('received',String(f.payload)));});
  page.on('request',r=>{if(r.url().includes('/attach?'))record('upload',r.url());});
  page.on('response',r=>{if(r.url().includes('/attach?'))record('uploaded',{url:r.url(),status:r.status()});});
  await page.waitForFunction(()=>document.documentElement.dataset.backplaneRemoteReady==='true',{timeout:30000});
  const brand=page.locator('.brand-mark');
  assert.equal(await brand.count(),1);
  const brandBox=await brand.boundingBox();assert.equal(brandBox.width,18);assert.equal(brandBox.height,18);
  assert(brandBox.x<(await page.locator('.topbar .name').boundingBox()).x);
  const indicator=brand.locator('.conn');assert.equal(await indicator.getAttribute('role'),'img');
  assert.equal(await indicator.getAttribute('aria-label'),'Connected');
  await brand.evaluate(async el=>{const url=getComputedStyle(el).backgroundImage.slice(5,-2);const img=new Image();img.src=url;await img.decode();if(!img.naturalWidth)throw Error('Official header logo failed to load');});
  const chrome=await app.evaluate(({Menu,BrowserWindow})=>({menu:Menu.getApplicationMenu(),visible:BrowserWindow.getAllWindows()[0].isMenuBarVisible()}));
  assert.equal(chrome.menu,null);assert.equal(chrome.visible,false);
  if(process.platform==='linux'&&process.env.DISPLAY&&fs.existsSync('/usr/bin/xprop')){
    const expected=await app.evaluate(({BrowserWindow,nativeImage},filename)=>{
      const w=BrowserWindow.getAllWindows()[0],handle=w.getNativeWindowHandle();
      const logo=nativeImage.createFromPath(filename);
      return {xid:'0x'+handle.readUInt32LE(0).toString(16),size:logo.getSize()};
    },path.join(stage,'desktop/runtime/resources/app/backplane.png'));
    // Canvas gives straight RGBA; NativeImage's platform bitmap may be
    // premultiplied BGRA. Keep alpha/opaque pixels exact and permit only one
    // channel unit on translucent pixels for the X11 alpha round trip.
    const rgba=Buffer.from(await page.evaluate(async encoded=>{
      const image=await createImageBitmap(new Blob([Uint8Array.from(atob(encoded),c=>c.charCodeAt(0))],{type:'image/png'}));
      const canvas=document.createElement('canvas');canvas.width=image.width;canvas.height=image.height;
      const ctx=canvas.getContext('2d');ctx.drawImage(image,0,0);const bytes=ctx.getImageData(0,0,image.width,image.height).data;
      const chunks=[];for(let i=0;i<bytes.length;i+=16384)chunks.push(String.fromCharCode(...bytes.subarray(i,i+16384)));
      image.close();return btoa(chunks.join(''));
    },fs.readFileSync(path.join(stage,'desktop/runtime/resources/app/backplane.png')).toString('base64')),'base64');
    expected.rgba_sha256=createHash('sha256').update(rgba).digest('hex');
    const props=execFileSync('/usr/bin/xprop',['-id',expected.xid,'-len','4194304','-f','_NET_WM_ICON','32c',' = $0+\\n','WM_CLASS','_NET_WM_ICON'],{encoding:'utf8',maxBuffer:16*1024*1024});
    fs.writeFileSync(path.join(out,'native-icon.xprop'),props);
    assert(/WM_CLASS.*"backplane"/.test(props),'window class matches the desktop launcher');
    const values=props.slice(props.indexOf('_NET_WM_ICON')).split('=').slice(1).join('=').match(/\d+/g)?.map(Number)||[];
    let matched=false;
    for(let at=0;at+2<values.length;){
      const width=values[at++],height=values[at++],count=width*height;
      assert(count>0&&count<=1024*1024&&at+count<=values.length,'valid native icon dimensions: '+JSON.stringify({width,height,remaining:values.length-at,prefix:props.slice(0,200)}));
      if(width===expected.size.width&&height===expected.size.height){
        matched=rgba.length===count*4;let rounded=0;
        for(let i=0;i<count&&matched;i++){
          const pixel=values[at+i]>>>0,a=pixel>>>24,tolerance=a>0&&a<255?1:0;
          const differences=[Math.abs((pixel>>>16&255)-rgba[i*4]),Math.abs((pixel>>>8&255)-rgba[i*4+1]),Math.abs((pixel&255)-rgba[i*4+2])];
          matched=a===rgba[i*4+3]&&differences.every(d=>d<=tolerance);
          if(differences.some(d=>d>0))rounded++;
        }
        expected.alpha_rounding_pixels=rounded;
      }
      at+=count;
    }
    assert(matched,'X11 window/taskbar icon pixels match the established Backplane logo');
    fs.writeFileSync(path.join(out,'native-icon.json'),JSON.stringify({...expected,verified:true},null,2));
  }
  const select=async id=>{await page.locator(`[data-on-click="select"][data-value="${id}"]`).first().click();await page.waitForFunction(id=>document.querySelector('main')?.dataset.thread===id&&document.documentElement.dataset.backplaneRemoteReady==='true',id);};
  // The local uberbot's Chat tab uses the same thread markdown path as the user's video.
  await page.locator('[data-on-click="select"][data-value="bub-thread"]').first().click();
  const chat=page.locator('[data-on-click="bot-tab"][data-value="chat"]');if(await chat.count())await chat.first().click();
  await page.locator(`a[data-ref="backplane://thread/${tid}"]`).first().waitFor();
  assert.equal(await page.locator(`a[data-ref="backplane://thread/${tid}"]`).count(),2);
  assert.equal(await page.locator('a[href^="javascript:"]').count(),0);assert(await page.locator('a[href="https://example.com"]').count());
  await page.locator(`a[data-ref="backplane://thread/${tid}"]`).first().click();
  await page.waitForFunction(id=>document.querySelector('main')?.dataset.thread===id&&document.documentElement.dataset.backplaneRemoteReady==='true',tid);
  const composer=page.locator('textarea[data-paste="attach"]');await composer.fill('Draft stays with Backplane polish');
  // Paste two actual PNG File objects; no Send is issued and no agent can run.
  const png=fs.readFileSync(path.resolve(__dirname,'../../assets/icon/backplane-64.png')).toString('base64');
  await composer.evaluate((el,png)=>{const bytes=Uint8Array.from(atob(png),c=>c.charCodeAt(0)),data=new DataTransfer();for(const name of ['one.png','two.png'])data.items.add(new File([bytes],name,{type:'image/png'}));el.dispatchEvent(new ClipboardEvent('paste',{clipboardData:data,bubbles:true,cancelable:true}));},png);
  await page.waitForFunction(()=>document.querySelectorAll('.attachment-preview').length===2&&[...document.querySelectorAll('.attachment-preview')].every(x=>x.complete&&x.naturalWidth>0));
  await page.screenshot({path:path.join(out,'attachments.png')});
  await select('other');await page.locator('textarea[data-paste="attach"]').fill('Other draft');
  await page.evaluate(()=>{window.__switchFrames=[];window.__switching=true;const frame=()=>{if(!window.__switching)return;const main=document.querySelector('main');window.__switchFrames.push({t:performance.now(),title:main?.querySelector('.title')?.textContent,sidebar:document.querySelectorAll('.sidebar .project').length,composer:!!document.querySelector('.composer'),thread:main?.dataset.thread,ready:document.documentElement.dataset.backplaneRemoteReady});requestAnimationFrame(frame);};requestAnimationFrame(frame);});
  await select(tid);await select('other');
  await page.locator('[data-on-click="select"][data-value="bub-thread"]').first().click();
  await page.waitForFunction(()=>document.querySelector('main')?.dataset.thread==='bub-thread'&&document.documentElement.dataset.backplaneRemoteReady==='true');
  await select(tid);
  observations.push(...await page.evaluate(()=>{window.__switching=false;return window.__switchFrames;}));
  assert(observations.every(x=>x.sidebar>0&&x.composer),'no collapsed sidebar or missing composer during the switches');
  assert.equal(await page.locator('textarea[data-paste="attach"]').inputValue(),'Draft stays with Backplane polish');
  assert.equal(await page.locator('.attachment-preview').count(),2);
  await page.locator('.composer [data-on-click="detach"]').first().click();await page.waitForFunction(()=>document.querySelectorAll('.attachment-preview').length===1);
  const settings=page.locator('.topbar [data-on-click="flag"][data-value="settings"]');
  const box=await settings.boundingBox();assert(box&&box.y<140,'Settings is at the top');await settings.click();
  await page.screenshot({path:path.join(out,'settings-desktop.png')});
  await page.locator('[data-on-click="settings-page"][data-value="appearance"]').first().click();
  await page.locator('[data-on-click="setting"][data-value="ui.theme=light"]').click();
  await page.waitForFunction(()=>document.querySelector('.app')?.classList.contains('light'));
  await page.screenshot({path:path.join(out,'settings-desktop-light.png')});
  await page.setViewportSize({width:390,height:844});await page.screenshot({path:path.join(out,'settings-mobile.png')});
  assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'390px layout has no document overflow');
  await page.locator('[data-on-click="setting"][data-value="ui.theme=dark"]').click();
  await page.waitForFunction(()=>!document.querySelector('.app')?.classList.contains('light'));
  await page.screenshot({path:path.join(out,'settings-mobile-dark.png')});
  assert.equal(errors.length,0,errors.join('\n'));
  const result={state:'passed',seconds:(performance.now()-start)/1000,home,observations,errors,checks:['branded shell/no menu','plain and bold internal links','unsafe link literal','external links preserved','multiple pasted previews','detach','per-thread drafts and attachments','frame-by-frame switch geometry','top Settings','390px layout','light and dark layouts']};
  fs.writeFileSync(path.join(out,'RESULT.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
 }catch(e){console.error(e);process.exitCode=1;if(page){await page.screenshot({path:path.join(out,'failure.png')}).catch(()=>{});fs.writeFileSync(path.join(out,'failure.html'),await page.content().catch(()=>''));}fs.writeFileSync(path.join(out,'failure.json'),JSON.stringify({error:e.stack,errors,traffic},null,2));}
 finally{if(app)await app.close();try{assert(await service.discover(),'resident hub survives window close');}finally{service.stop();}}
})();
