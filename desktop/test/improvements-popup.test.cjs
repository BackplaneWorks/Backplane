// Source popup and HTTP attachment integration under production desktop CSP.
// Native capture/RPC are fixtures; live-app/provider acceptance is separate.
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),http=require('node:http');
const {pathToFileURL}=require('node:url');
const root=path.resolve(__dirname,'../..');
const load=name=>import(pathToFileURL(path.join(root,'src/web',name)).href);
const png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aH1sAAAAASUVORK5CYII=';

test('Backplane project resolution never borrows the origin and ambiguity needs selection',async()=>{
 const {backplaneProject,improvementContext}=await load('improvements-context.js');
 const origin={id:'origin',title:'Backplane',root:'/work/hardware'};
 const app={id:'app',title:'Application',root:'/work/backplane-bend/'};
 assert.equal(backplaneProject([origin,app]),app);
 assert.equal(backplaneProject([origin,{id:'old',root:'/work/Backplane'}]),null);
 assert.equal(backplaneProject([app,{...app,id:'other',root:'D:\\copy\\backplane-bend'}]),null);
 const context=improvementContext({thread:'source-thread',project:'origin',bot:'bub',name:'Bub',view:'Chat',path:'board.kicad_pcb'},
  {thread:'source-thread',project:'origin',projects:[origin,app]});
 assert.equal(context.implementationProject.id,'app');assert.equal(context.sourceProject.id,'origin');
 assert.equal(context.sourceThread.url,'backplane://thread/source-thread');assert.equal(context.sourceBot.url,'backplane://bot/bub');assert.equal(context.sourceBot.name,'Bub');
 assert.equal(context.sourceProject.url,'backplane://project/origin');
});

test('direct original bot chat retains its bot identity without a bot route',async()=>{
 const {improvementOrigin}=await load('improvements-context.js');
 const doc={querySelector(selector){
  if(selector==='main.thread-view')return{dataset:{thread:'t123-45',project:''},querySelector:()=>null};
  if(selector.startsWith('.topbar'))return{textContent:'Bub'};
  return null;
 },querySelectorAll:()=>[{dataset:{value:'b123-45'}}]};
 const origin=improvementOrigin({doc,session:{sel:'t123-45'}});
 assert.equal(origin.bot,'b123-45');assert.equal(origin.name,'Bub');assert.equal(origin.project,'');
 doc.querySelectorAll=()=>[{dataset:{value:'b999-99'}}];
 assert.equal(improvementOrigin({doc,session:{sel:'t123-45'}}).bot,'');
});

test('popup toggle, focus, actionable Send, IME-safe Ctrl+Enter, origin routing and retry',{timeout:20000},async()=>{
 const {chromium}=require(process.env.BACKPLANE_PLAYWRIGHT_PATH||path.join(root,'desktop/node_modules/playwright-core'));
 const csp=fs.readFileSync(path.join(root,'desktop/main.cjs'),'utf8').match(/'Content-Security-Policy': \["([^"]+)"\]/)[1];
 const uploads=[];
 const html='<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/src/web/style.css"><div id="root"><main class="thread-view" data-thread="source-thread" data-project="origin"><header class="thread-head"><span class="bot-title">Bub</span></header><button data-improvements>Improvements</button><textarea id="composer">original composer draft</textarea></main></div><script type="module" src="/fixture.js"></script>';
 const fixture=`import {installImprovements} from '/src/web/improvements.js';import {improvementOrigin,improvementContext} from '/src/web/improvements-context.js';
 const f=window.fixture={calls:[],selected:[],failCapture:true,failUpload:false,failStart:false,captures:0};
 window.backplaneDesktop={captureApp:async()=>{f.captures++;if(f.capturePending)await new Promise(r=>f.releaseCapture=r);if(f.failCapture)throw Error('fixture cancel');return 'data:image/png;base64,${png}';},onImprovements:cb=>f.nativeShortcut=cb};
 f.ui=installImprovements({identity:()=> 'fixture-hub',ready:()=>true,
 context:async()=>improvementContext(improvementOrigin({doc:document,session:{sel:'source-thread',route:{bot:'bub',tab:'chat'}}}),{thread:'source-thread',project:'origin',projects:[{id:'origin',title:'Hardware',root:'/work/hardware'},{id:'app',title:'Backplane',root:'/work/backplane-bend'}]}),
 rpc:{request:async(m,p)=>{f.calls.push({m,p});if(m==='thread.create')return{thread:'improvement-'+p.creationKey};if(m==='improvements.context')return{thread:p.sourceThread,accountBinding:'fixture-account'};if(f.failStart){f.failStart=false;throw Error('Lost reply. Retry Send.');}return{ok:true};}},
 select:async id=>f.selected.push(id)});`;
 const server=http.createServer((req,res)=>{
  res.setHeader('Content-Security-Policy',csp);
  if(req.method==='POST'&&req.url.startsWith('/attach?')){
   const chunks=[];req.on('data',x=>chunks.push(x));req.on('end',()=>{uploads.push({url:req.url,bytes:Buffer.concat(chunks)});res.setHeader('Content-Type','application/json');res.end(JSON.stringify({thread:new URL(req.url,'http://fixture').searchParams.get('thread'),attached:'/fixture/backplane.png'}));});return;
  }
  if(req.url==='/'){res.setHeader('Content-Type','text/html');res.end(html);return;}
  if(req.url==='/fixture.js'){res.setHeader('Content-Type','text/javascript');res.end(fixture);return;}
  const file=path.join(root,req.url);if(!file.startsWith(root+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}
  res.setHeader('Content-Type',file.endsWith('.css')?'text/css':'text/javascript');res.end(fs.readFileSync(file));
 });
 let browser;
 try {
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  browser=await chromium.launch({headless:true,executablePath:process.env.BACKPLANE_CHROME,args:['--no-sandbox','--disable-dev-shm-usage']});
  const page=await browser.newPage({viewport:{width:1100,height:850}});page.setDefaultTimeout(5000);
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto('http://127.0.0.1:'+server.address().port);await page.waitForFunction(()=>window.fixture?.ui);
  await page.locator('#composer').focus();await page.locator('#composer').evaluate(n=>n.setSelectionRange(2,5));
  await page.keyboard.press('Control+Alt+i');await page.locator('.improvements:not([hidden])').waitFor();
  assert.equal(await page.locator('#improvements-text').evaluate(n=>document.activeElement===n),true);
  assert.equal(await page.locator('#improvements-project').inputValue(),'app');
  await page.locator('#improvements-text').fill('  Harmless fixture: report only, no edits. 日本語  ');
  await page.getByRole('button',{name:'Send',exact:true}).click();
  assert.match(await page.locator('.improvements [role=status]').textContent(),/Capture again or choose/);
  assert.equal(await page.evaluate(()=>fixture.calls.length),0);
  await page.keyboard.press('Control+Alt+i');assert.equal(await page.locator('.improvements').evaluate(n=>n.hidden),true);
  assert.deepEqual(await page.locator('#composer').evaluate(n=>[document.activeElement===n,n.selectionStart,n.selectionEnd]),[true,2,5]);
  await page.evaluate(()=>{fixture.failCapture=false;fixture.nativeShortcut();});
  await page.waitForFunction(()=>!document.querySelector('.improvements').hidden&&document.querySelector('.improvements img').naturalWidth>0);
  const text=await page.locator('#improvements-text').inputValue(),image=await page.locator('.improvements img').getAttribute('src');
  assert.equal(await page.locator('#improvements-text').evaluate(n=>document.activeElement===n),true);
  // Native bridge shortcuts toggle too, while ordinary editing keeps its keys.
  await page.evaluate(()=>fixture.nativeShortcut());assert.equal(await page.locator('.improvements').evaluate(n=>n.hidden),true);
  await page.evaluate(()=>{document.querySelector('main').dataset.thread='different-thread';fixture.nativeShortcut();});
  await page.locator('.improvements:not([hidden])').waitFor();assert.equal(await page.locator('#improvements-text').inputValue(),text);
  assert(await page.locator('.improvements img').isVisible());
  await page.locator('#improvements-text').evaluate(n=>n.dispatchEvent(new KeyboardEvent('keydown',{bubbles:true,key:'Enter',ctrlKey:true,isComposing:true})));
  await page.locator('#improvements-text').evaluate(n=>n.dispatchEvent(new KeyboardEvent('keydown',{bubbles:true,key:'Enter',ctrlKey:true,keyCode:229})));
  await page.locator('#improvements-text').evaluate(n=>n.dispatchEvent(new KeyboardEvent('keydown',{bubbles:true,key:'Enter',ctrlKey:true,repeat:true})));
  assert.equal(await page.evaluate(()=>fixture.calls.length),0);
  // Closing during a capture keeps the old draft and ignores its late result.
  await page.evaluate(()=>fixture.capturePending=true);await page.getByRole('button',{name:'Capture again',exact:true}).click();
  await page.waitForFunction(()=>typeof fixture.releaseCapture==='function');
  await page.evaluate(()=>{fixture.nativeShortcut();fixture.capturePending=false;fixture.releaseCapture();});
  await page.waitForFunction(()=>document.querySelector('.improvements').hidden);
  await page.evaluate(()=>fixture.nativeShortcut());await page.locator('.improvements:not([hidden])').waitFor();
  assert.equal(await page.locator('#improvements-text').inputValue(),text);assert(await page.locator('.improvements img').isVisible());
  // Recapture failure preserves the previous image and draft; retry is explicit.
  await page.evaluate(()=>{fixture.failCapture=true;});await page.getByRole('button',{name:'Capture again',exact:true}).click();
  await page.waitForFunction(()=>!document.querySelector('.improvements').hidden);
  assert.equal(await page.locator('#improvements-text').inputValue(),text);assert(await page.locator('.improvements img').isVisible());
  await page.evaluate(()=>fixture.failStart=true);await page.keyboard.press('Control+Enter');
  await page.waitForFunction(()=>document.querySelector('.improvements [role=status]').textContent.includes('Lost reply'));
  assert.equal(await page.locator('#improvements-text').inputValue(),text);assert(await page.locator('.improvements img').isVisible());
  await page.evaluate(()=>{const send=document.querySelector('.improvements .primary');send.click();send.click();});
  await page.waitForFunction(()=>fixture.selected.length===1);
  const calls=await page.evaluate(()=>fixture.calls),creates=calls.filter(x=>x.m==='thread.create'),starts=calls.filter(x=>x.m==='turn.start');
  assert.equal(creates.length,2);assert(creates.every(x=>x.p.project==='app'));assert.deepEqual(creates[0].p,creates[1].p);
  assert.equal(starts.length,2);assert.deepEqual(starts[0].p,starts[1].p);assert(starts[0].p.thread.startsWith('improvement-'));
  assert(starts[0].p.text.startsWith(text+'\n\n'));assert.match(starts[0].p.text,/Backplane application code/);
  assert.match(starts[0].p.text,/"sourceThread":\{"id":"source-thread"/);assert.match(starts[0].p.text,/"sourceBot":\{"id":"bub","name":"Bub"/);
  assert.match(starts[0].p.text,/"sourceProject":\{"id":"origin"/);assert.match(starts[0].p.text,/\[attachment: \/fixture\/backplane.png\]$/);
  assert.equal(uploads.length,1);assert.deepEqual(uploads[0].bytes,Buffer.from(png,'base64'));
  assert.equal(await page.locator('#composer').inputValue(),'original composer draft');
  // Capture a source fixture evidence image at a constrained phone width.
  await page.setViewportSize({width:390,height:844});await page.evaluate(()=>{fixture.failCapture=false;fixture.nativeShortcut();});
  await page.waitForFunction(()=>!document.querySelector('.improvements').hidden&&document.querySelector('.improvements img').naturalWidth>0);
  const box=await page.locator('.improvements').boundingBox();assert(box.x>=0&&box.x+box.width<=390&&box.y+box.height<=844);
  const out=process.env.BACKPLANE_IMPROVEMENTS_EVIDENCE;if(out){fs.mkdirSync(out,{recursive:true});await page.screenshot({path:path.join(out,'popup-followup-phone.png')});fs.writeFileSync(path.join(out,'popup-followup.json'),JSON.stringify({fixture:true,csp,calls,uploads:uploads.length,box,errors},null,2));}
  assert.deepEqual(errors,[]);
 } finally {if(browser)await browser.close();await new Promise(r=>server.close(r));}
});
