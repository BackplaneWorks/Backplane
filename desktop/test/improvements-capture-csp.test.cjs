const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),http=require('node:http');
const {pathToFileURL}=require('node:url');
const root=path.resolve(__dirname,'../..');
const capturePath=path.join(root,'src/web/improvements-capture.js');
const png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aH1sAAAAASUVORK5CYII=';

test('native capture decodes binary PNG locally without fetching a data URL',async()=>{
 const {captureApp}=await import(pathToFileURL(capturePath).href);
 const bytes=Uint8Array.from({length:300000},(_,i)=>i%256);
 const fetchBefore=globalThis.fetch;
 globalThis.fetch=()=>assert.fail('native screenshot decoding must not use fetch');
 try {
  let calls=0;
  const file=await captureApp({bridge:{captureApp:async()=>{calls++;return 'data:image/png;base64,'+Buffer.from(bytes).toString('base64');}},media:null,doc:null,win:null});
  assert.equal(calls,1);assert.equal(file.name,'backplane.png');assert.equal(file.type,'image/png');
  assert.deepEqual(new Uint8Array(await file.arrayBuffer()),bytes);
  await assert.rejects(captureApp({bridge:{captureApp:async()=> 'data:image/jpeg;base64,AAAA'},media:null,doc:null,win:null}),/could not capture/);
 } finally {globalThis.fetch=fetchBefore;}
});

test('native PNG becomes a preview File under the actual desktop connect-src self policy',{timeout:20000},async()=>{
 const {chromium}=require(process.env.BACKPLANE_PLAYWRIGHT_PATH||path.join(root,'desktop/node_modules/playwright-core'));
 const csp=fs.readFileSync(path.join(root,'desktop/main.cjs'),'utf8').match(/'Content-Security-Policy': \["([^"]+)"\]/)?.[1];
 assert(csp?.includes("connect-src 'self'"),'use the production desktop CSP without relaxing it');
 const server=http.createServer((req,res)=>{
  res.setHeader('Content-Security-Policy',csp);
  if(req.url==='/capture.js'){res.setHeader('Content-Type','text/javascript');res.end(fs.readFileSync(capturePath));}
  else {res.setHeader('Content-Type','text/html');res.end('<!doctype html><title>Capture CSP regression</title><img alt="Screenshot preview">');}
 });
 let browser;
 try {
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  browser=await chromium.launch({headless:true,executablePath:process.env.BACKPLANE_CHROME,args:['--no-sandbox','--disable-dev-shm-usage']});
  const page=await browser.newPage();
  await page.goto('http://127.0.0.1:'+server.address().port);
  const result=await page.evaluate(async png=>{
   const uri='data:image/png;base64,'+png,violations=[];
   document.addEventListener('securitypolicyviolation',e=>violations.push(e.effectiveDirective));
   let blocked=false;
   try {await fetch(uri);} catch {blocked=true;}
   await new Promise(r=>setTimeout(r,0));
   const negativeControl=violations.splice(0);
   let fetchCalls=0;const savedFetch=window.fetch;
   window.fetch=(...args)=>{fetchCalls++;return savedFetch(...args);};
   const {captureApp}=await import('/capture.js');
   const file=await captureApp({bridge:{captureApp:async()=>uri}});
   const image=document.querySelector('img'),url=URL.createObjectURL(file);
   try {
    image.src=url;await image.decode();await new Promise(r=>setTimeout(r,0));
    return {blocked,negativeControl,violations,fetchCalls,name:file.name,type:file.type,bytes:[...new Uint8Array(await file.arrayBuffer())],width:image.naturalWidth,height:image.naturalHeight};
   } finally {URL.revokeObjectURL(url);}
  },png);
  assert(result.blocked,'negative control: production CSP must reject data URL fetch');
  assert(result.negativeControl.includes('connect-src'));
  assert.equal(result.fetchCalls,0);assert.deepEqual(result.violations,[]);
  assert.equal(result.name,'backplane.png');assert.equal(result.type,'image/png');
  assert.deepEqual(result.bytes,[...Buffer.from(png,'base64')]);
  assert.equal(result.width,1);assert.equal(result.height,1);
 } finally {if(browser)await browser.close();await new Promise(r=>server.close(r));}
});
