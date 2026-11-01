// Bounded own-window capture check. This fixture uses the SOURCE UI and real
// Electron capturePage; RPC is a stand-in, NOT a live provider/hub check.
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),http=require('node:http'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'../..');
const {_electron}=require(path.join(root,'build/improvements-deps/node_modules/playwright-core'));
const out=path.join(root,'build/improvements-evidence');fs.mkdirSync(out,{recursive:true});
const tmp=fs.mkdtempSync(path.join(os.tmpdir(),'bp-improvements-capture-'));
let app,server,page,xvfb;const uploads=[],errors=[];
const html=`<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/src/web/style.css"><style>body{margin:0;background:#207040;color:white;font:16px monospace}#underlying{height:100vh;padding:24px;box-sizing:border-box;background:linear-gradient(90deg,#207040 50%,#206090 50%)}.topbar{position:static}</style></head><body><div id="root"><main id="underlying"><header class="topbar"><button data-improvements>Improvements</button></header><h1>UNDERLYING BACKPLANE APP</h1><textarea id="composer">existing conversation draft</textarea><p>Only this app window may be captured.</p></main></div><script type="module">
import {installImprovements} from '/src/web/improvements.js';import {captureApp} from '/src/web/improvements-capture.js';
window.fixture={keys:[],calls:[],selected:[],failCapture:false,failStart:false,captures:0};
const f=window.fixture;window.addEventListener('keydown',e=>f.keys.push({key:e.key,code:e.code,ctrl:e.ctrlKey,alt:e.altKey,meta:e.metaKey,composing:e.isComposing}),true);f.ui=installImprovements({identity:()=> 'fixture-authenticated-hub',ready:()=>true,
context:()=>({project:'p',app:'Backplane fixture',view:'Thread',thread:'source',projects:[{id:'p',title:'Current project'},{id:'q',title:'Other project'}]}),
rpc:{request:async(m,p)=>{f.calls.push({m,p});if(m==='thread.create')return{thread:'improvement-'+p.creationKey};if(m==='improvements.context')return{thread:p.sourceThread,accountBinding:''};if(f.failStart){f.failStart=false;throw Error('Connection changed. Reconnect, then press Send to retry the same request.');}return{ok:true};}},
select:async thread=>{f.selected.push(thread);},beforeCapture:()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))),
capture:async options=>{f.captures++;if(f.failCapture)throw Error('capture canceled');try{return await captureApp(options);}catch(e){f.captureError=e.message;throw e;}}});
</script></body></html>`;
server=http.createServer((req,res)=>{
 const url=new URL(req.url,'http://fixture');
 if(req.method==='POST'&&url.pathname==='/attach'){
  const chunks=[];req.on('data',c=>chunks.push(c));req.on('end',()=>{const body=Buffer.concat(chunks);uploads.push({thread:url.searchParams.get('thread'),key:url.searchParams.get('key'),bytes:body.length,png:body.subarray(0,8).toString('hex')});res.setHeader('Content-Type','application/json');res.end(JSON.stringify({thread:url.searchParams.get('thread'),attached:'/tmp/fixture/attachments/'+url.searchParams.get('thread')+'/'+url.searchParams.get('key')+'_backplane.png'}));});return;
 }
 if(url.pathname==='/'){res.setHeader('Content-Type','text/html');res.end(html);return;}
 const file=path.join(root,url.pathname);if(!file.startsWith(root+path.sep)||!fs.existsSync(file)){res.writeHead(404);res.end();return;}
 res.setHeader('Content-Type',file.endsWith('.css')?'text/css':'text/javascript');res.end(fs.readFileSync(file));
});
(async()=>{try{
 const {spawn}=require('node:child_process');
 xvfb=spawn(process.env.BACKPLANE_XVFB||path.join(root,'build/improvements-xvfb/usr/bin/Xvfb'),['-displayfd','3','-screen','0','1280x1000x24','-nolisten','tcp','-ac'],{stdio:['ignore','ignore','pipe','pipe']});
 const display=await new Promise((resolve,reject)=>{xvfb.once('error',reject);xvfb.once('exit',code=>reject(Error('Private Xvfb exited '+code)));xvfb.stdio[3].once('data',chunk=>resolve(':'+String(chunk).trim()));});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));const origin='http://127.0.0.1:'+server.address().port;
 fs.writeFileSync(path.join(tmp,'preload.cjs'),`const {contextBridge,ipcRenderer}=require('electron');contextBridge.exposeInMainWorld('backplaneDesktop',{captureApp:()=>ipcRenderer.invoke('capture'),onImprovements:cb=>ipcRenderer.on('desktop:improvements',cb)});`);
 fs.writeFileSync(path.join(tmp,'main.cjs'),`const {app,BrowserWindow,ipcMain}=require('electron');const {captureAppWindow}=require(${JSON.stringify(path.join(root,'desktop/improvements-capture.cjs'))});const {installShortcuts}=require(${JSON.stringify(path.join(root,'desktop/window.cjs'))});app.whenReady().then(()=>{app.setPath('userData',${JSON.stringify(path.join(tmp,'data'))});const w=new BrowserWindow({width:1100,height:850,webPreferences:{preload:__dirname+'/preload.cjs',contextIsolation:true,sandbox:true}});ipcMain.handle('capture',e=>{if(e.sender!==w.webContents||e.senderFrame!==w.webContents.mainFrame)throw Error('Invalid sender');return captureAppWindow(w);});installShortcuts(w,require('electron'));w.loadURL(${JSON.stringify(origin)});});app.on('window-all-closed',()=>app.quit());`);
 // A private runtime WITHOUT the installed app/service. Never launch the
 // installed packaged app: it ignores the fixture entry point.
 const vendor='/home/h/.local/share/backplane/desktop/runtime',runtime=path.join(tmp,'runtime');fs.mkdirSync(runtime);
 for(const name of fs.readdirSync(vendor)){if(name==='resources')continue;const source=path.join(vendor,name),dest=path.join(runtime,name);if(fs.statSync(source).isDirectory())fs.cpSync(source,dest,{recursive:true});else fs.copyFileSync(source,dest,fs.constants.COPYFILE_FICLONE);}
 fs.mkdirSync(path.join(runtime,'resources'));fs.copyFileSync(path.join(vendor,'resources/default_app.asar'),path.join(runtime,'resources/default_app.asar'));
 app=await _electron.launch({executablePath:process.env.BACKPLANE_ELECTRON||path.join(runtime,'backplane-desktop'),args:['--ozone-platform=x11',path.join(tmp,'main.cjs')],env:{...process.env,DISPLAY:display,WAYLAND_DISPLAY:''},timeout:20000});
 app.process().stderr.on('data',chunk=>fs.appendFileSync(path.join(out,'electron.log'),chunk));page=await app.firstWindow();page.on('pageerror',e=>errors.push(e.message));await page.waitForFunction(()=>window.fixture?.ui);page.setDefaultTimeout(8000);
 await app.evaluate(({BrowserWindow})=>{const w=BrowserWindow.getAllWindows()[0];w.show();w.focus();w.webContents.focus();});await page.bringToFront();await page.locator('#composer').focus();await page.locator('#composer').evaluate(e=>e.setSelectionRange(3,8));
 await app.evaluate(({BrowserWindow})=>{const wc=BrowserWindow.getAllWindows()[0].webContents;wc.sendInputEvent({type:'keyDown',keyCode:'I',modifiers:['control','alt']});wc.sendInputEvent({type:'keyUp',keyCode:'I',modifiers:['control','alt']});});await page.locator('.improvements:not([hidden]) img').waitFor();
 assert.equal(await page.evaluate(()=>window.fixture.calls.length),0,'opening/capture do not dispatch');
 const firstImage=await page.locator('.improvements>img').getAttribute('src');
 const sample=await page.locator('.improvements>img').evaluate(async img=>{await img.decode();const c=document.createElement('canvas');c.width=img.naturalWidth;c.height=img.naturalHeight;c.getContext('2d').drawImage(img,0,0);return{width:c.width,height:c.height,pixel:[...c.getContext('2d').getImageData(c.width/2-80,c.height/2,1,1).data]};});
 assert.deepEqual(sample.pixel,[32,112,64,255],'center under popup contains unobscured underlying app');
 const raw=await page.locator('.improvements>img').evaluate(async img=>Array.from(new Uint8Array(await(await fetch(img.src)).arrayBuffer())));fs.writeFileSync(path.join(out,'underlying-native.png'),Buffer.from(raw));
 const text='  Make this button clearer.\n日本語 & exact spacing  ';await page.locator('#improvements-text').fill(text);
 const before=await page.locator('.improvements').boundingBox();await page.mouse.move(before.x+30,before.y+20);await page.mouse.down();await page.mouse.move(10000,10000);await page.mouse.up();
 const moved=await page.locator('.improvements').boundingBox(),vp=page.viewportSize()||await page.evaluate(()=>({width:innerWidth,height:innerHeight}));assert(moved.x>=0&&moved.y>=0&&moved.x+moved.width<=vp.width&&moved.y+moved.height<=vp.height);
 await page.locator('#improvements-text').focus();await page.locator('#improvements-text').evaluate(e=>e.setSelectionRange(4,9));
 await page.getByRole('button',{name:'Capture again',exact:true}).click();await page.waitForFunction(()=>window.fixture.captures===2&&document.querySelector('.improvements:not([hidden])'));
 assert.equal(await page.locator('#improvements-text').inputValue(),text);assert.notEqual(await page.locator('.improvements>img').getAttribute('src'),firstImage);
 // Keyboard activation keeps caret/focus in the textarea across failure.
 await page.locator('#improvements-text').focus();await page.locator('#improvements-text').evaluate(e=>e.setSelectionRange(4,9));
 await page.evaluate(()=>{window.fixture.failCapture=true;document.querySelector('.improvements footer button').click();});
 await page.waitForFunction(()=>document.querySelector('.improvements [role="status"]').textContent.includes('not completed'));
 assert.deepEqual(await page.locator('#improvements-text').evaluate(e=>[e.value,e.selectionStart,e.selectionEnd,document.activeElement===e]),[text,4,9,true]);
 await page.screenshot({path:path.join(out,'desktop-popup.png')});
 await app.evaluate(({BrowserWindow})=>{const wc=BrowserWindow.getAllWindows()[0].webContents;wc.sendInputEvent({type:'keyDown',keyCode:'ESC'});wc.sendInputEvent({type:'keyUp',keyCode:'ESC'});});assert.equal(await page.locator('#composer').inputValue(),'existing conversation draft');
 assert.deepEqual(await page.locator('#composer').evaluate(e=>[document.activeElement===e,e.selectionStart,e.selectionEnd]),[true,3,8]);
 await app.evaluate(({BrowserWindow})=>{const wc=BrowserWindow.getAllWindows()[0].webContents;wc.sendInputEvent({type:'keyDown',keyCode:'I',modifiers:['control','alt']});wc.sendInputEvent({type:'keyUp',keyCode:'I',modifiers:['control','alt']});});await page.locator('.improvements:not([hidden])').waitFor();assert.equal(await page.locator('#improvements-text').inputValue(),text);
 await page.evaluate(()=>window.fixture.failStart=true);await page.getByRole('button',{name:'Send',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.improvements [role="status"]').textContent.includes('Connection changed'));
 assert.equal(await page.locator('#improvements-text').inputValue(),text);assert(await page.locator('.improvements>img').isVisible());
 await page.evaluate(()=>{const b=document.querySelector('.improvements .primary');b.click();b.click();});await page.waitForFunction(()=>window.fixture.selected.length===1);
 const calls=await page.evaluate(()=>window.fixture.calls),creates=calls.filter(c=>c.m==='thread.create'),starts=calls.filter(c=>c.m==='turn.start');assert.equal(creates.length,2);assert.equal(starts.length,2);assert.deepEqual(creates[0].p,creates[1].p);assert.deepEqual(starts[0].p,starts[1].p);assert(starts[0].p.text.startsWith(text+'\n\n'));assert.equal(uploads.length,1);assert.equal(uploads[0].png,'89504e470d0a1a0a');
 // A native Browser pane is an own-window child, not an unrelated window.
 await app.evaluate(async({BrowserWindow,WebContentsView})=>{const w=BrowserWindow.getAllWindows()[0],view=new WebContentsView();view.setBounds({x:700,y:250,width:200,height:150});w.contentView.addChildView(view);await view.webContents.loadURL('data:text/html,<body style="margin:0;background:rgb(210,80,30)">OWN APP BROWSER PANE</body>');globalThis.fixtureChild=view;});
 const childPixel=await page.evaluate(async()=>{const blob=await(await fetch(await window.backplaneDesktop.captureApp())).blob(),image=await createImageBitmap(blob),canvas=document.createElement('canvas');canvas.width=image.width;canvas.height=image.height;canvas.getContext('2d').drawImage(image,0,0);return[...canvas.getContext('2d').getImageData(750,320,1,1).data];});
 assert.deepEqual(childPixel,[210,80,30,255],'own native Browser pane appears in app capture');
 await app.evaluate(({BrowserWindow})=>{const view=globalThis.fixtureChild;BrowserWindow.getAllWindows()[0].contentView.removeChildView(view);view.webContents.close();});
 // New draft at phone width: constrained sheet and controls remain reachable.
 await page.evaluate(()=>window.fixture.failCapture=false);await app.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows()[0].setSize(390,844));await page.getByRole('button',{name:'Improvements',exact:true}).click();await page.locator('.improvements:not([hidden])').waitFor();
 const mobile=await page.locator('.improvements').boundingBox();const mobileVP=await page.evaluate(()=>({width:innerWidth,height:innerHeight}));assert(mobile.x>=0&&mobile.width<=mobileVP.width&&mobile.y+mobile.height<=mobileVP.height);
 await page.locator('#improvements-text').fill('Phone-sized request');await page.screenshot({path:path.join(out,'phone-sheet.png')});
 assert.deepEqual(errors,[]);fs.writeFileSync(path.join(out,'capture-integration.json'),JSON.stringify({status:'passed',capture:'actual Linux Electron own-window capturePage',transport:'fixture RPC; real HTTP PNG upload',calls,uploads,sample,mobile,mobileVP,errors},null,2));console.log('PASS: actual Electron own-window capture, no overlay, drag, recapture failure/caret/draft, Send retry/dedup and constrained phone layout');
 }catch(error){console.error('ORIGINAL',error);if(page&&!page.isClosed()){console.error(await page.evaluate(()=>({fixture:window.fixture&&{captures:window.fixture.captures,keys:window.fixture.keys,error:window.fixture.captureError},popup:document.querySelector('.improvements')?.outerHTML})));await page.screenshot({path:path.join(out,'failure.png')}).catch(()=>{});}throw error;}finally{if(app)await app.close();if(xvfb)xvfb.kill();if(server)await new Promise(r=>server.close(r));fs.rmSync(tmp,{recursive:true,force:true});}})().catch(e=>{console.error(e);process.exitCode=1;});
