const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),http=require('node:http');
const root=path.resolve(__dirname,'../..');
test('public report writer, exact editable preview, evidence and consent on desktop and phone',{timeout:30000},async()=>{
 const {chromium}=require(process.env.BACKPLANE_PLAYWRIGHT_PATH||path.join(root,'desktop/node_modules/playwright-core'));
 const policy=process.env.BACKPLANE_ISSUE_POLICY||path.join(root,'build/issue-report-policy.mjs');
 assert(fs.existsSync(policy),'compile src/core/issue-report.bend to build/issue-report-policy.mjs before this test');
 const fixture=`import Bend from '/policy.mjs';import {installImprovements} from '/src/web/improvements.js';
 const api=n=>Bend[Object.keys(Bend).find(k=>k.endsWith('Issue.'+n))];
 const plain=j=>{if(j.$.split('.').at(-1)==='Str')return j.text;const out={};for(let f=j.fields;typeof f?.key==='string';f=f.tail)out[f.key]=plain(f.value);return out;};
 const f=window.fixture={calls:[],opens:[],captures:0,reads:0,done:false,quota:false,data:new Map()};
 const body='# Title\\n\\nConnection closes after reconnect.\\n\\n## Evidence\\nUser typed a detailed description.\\n\\nOS: Linux\\nApp version: 1.2.3\\n';
 f.body=body;
 f.ui=installImprovements({identity:()=> 'authenticated-fixture',ready:()=>true,context:async()=>({appVersion:'1.2.3-dev',sourceThread:{id:'PRIVATE-thread'},path:'/home/PRIVATE',projects:[{id:'reports',title:'Report workspace',root:'/fixture/reports'}]}),
 reportBody:api('report'),reportTarget:api('target'),reportPrompt:api('prompt'),reportTemplate:api('template'),previewBody:b=>plain(api('preview.body')(b)),appVersion:async()=> '1.2.3-dev.private',
 store:{read:async k=>structuredClone(f.data.get(k)),write:async(k,v)=>{if(f.quota)throw Error('Local draft storage is full.');f.data.set(k,structuredClone(v));}},
 openIssue:async url=>f.opens.push(url),select:async()=>{},capture:async()=>{f.captures++;return new Blob(['test-image'],{type:'image/png'});},
 diagnostics:{enabled:()=>true,snapshot:async()=>{f.reads++;return{connections:[{event:'closed',at:Date.now(),closeCode:1006,prompt:'PRIVATE'}],hub:{events:[{event:'latency',name:'hub.rpc',count:1,us:4000,prompt:'PRIVATE',path:'/home/PRIVATE'}]}};}},
 rpc:{request:async(m,p)=>{f.calls.push({m,p});if(m==='thread.create')return{thread:'improvement-'+p.creationKey};if(m==='improvements.context')return{thread:p.sourceThread,accountBinding:'fixture-generation'};if(m==='improvements.report')return{thread:p.thread,done:f.done,body};return{ok:true};}}});`;
 const server=http.createServer((req,res)=>{
  if(req.url==='/'){res.setHeader('Content-Type','text/html');res.end('<meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/src/web/style.css"><div id="root"><button data-improvements>Improvements</button></div><script type="module" src="/fixture.js"></script>');return;}
  const file=req.url==='/policy.mjs'?policy:path.join(root,req.url);
  if(req.url==='/fixture.js'){res.setHeader('Content-Type','text/javascript');res.end(fixture);return;}
  if(!file.startsWith(root+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}
  res.setHeader('Content-Type',file.endsWith('.css')?'text/css':'text/javascript');res.end(fs.readFileSync(file));
 });let browser,page,errors=[];
 try{
  await new Promise(r=>server.listen(0,'127.0.0.1',r));browser=await chromium.launch({headless:true,executablePath:process.env.BACKPLANE_CHROME,args:['--no-sandbox','--disable-dev-shm-usage']});
  page=await browser.newPage({viewport:{width:1050,height:900}});page.on('pageerror',e=>errors.push(e.message));page.setDefaultTimeout(6000);
  // No GitHub request is allowed; the handoff and agent are local fixtures.
  await page.route('https://github.com/**',route=>{throw Error('External issue navigation forbidden in tests');});
  await page.goto('http://127.0.0.1:'+server.address().port);await page.getByRole('button',{name:'Improvements',exact:true}).click();
  await page.locator('.improvements:not([hidden])').waitFor();assert.equal(await page.locator('#improvements-target').inputValue(),'issue');
  assert.equal(await page.evaluate(()=>fixture.captures),0);assert.equal(await page.locator('#improvements-logs').isChecked(),false);assert.equal(await page.locator('#improvements-screenshot').isChecked(),false);
  const description='  After the hub restarts, the connection dot stays red. 日本語  ';
  await page.locator('#improvements-text').fill(description);await page.getByRole('button',{name:'Review issue',exact:true}).click();assert.match(await page.locator('.improvements [role=status]').textContent(),/thorough behavior description/);
  await page.locator('#improvements-evidence').fill('I restarted the test hub, waited for reconnect, then opened a thread. The conversation loaded, but the connection dot stayed red.');
  await page.getByRole('button',{name:'Write report with agent',exact:true}).click();await page.getByRole('button',{name:'Load agent report',exact:true}).waitFor();
  const calls=await page.evaluate(()=>fixture.calls),turn=calls.find(x=>x.m==='turn.start');assert.equal(calls[0].p.project,'reports');assert.equal(turn.p.accountBinding,'fixture-generation');
  assert.match(turn.p.text,/ASD-STE100/);assert.match(turn.p.text,/# Unslop/);assert.match(turn.p.text,/image or a thorough description/);assert(turn.p.text.endsWith(description));assert(!turn.p.text.includes('PRIVATE-thread'));assert(!turn.p.text.includes('/home/PRIVATE'));assert.equal(await page.evaluate(()=>fixture.reads),0);
  await page.getByRole('button',{name:'Load agent report',exact:true}).click();assert.match(await page.locator('.improvements [role=status]').textContent(),/still working/);
  await page.evaluate(()=>fixture.done=true);await page.getByRole('button',{name:'Load agent report',exact:true}).click();await page.locator('#improvements-review').waitFor();
  assert.equal(await page.locator('#improvements-review').inputValue(),await page.evaluate(()=>fixture.body));
  const edited=(await page.locator('#improvements-review').inputValue())+'\nEdited by reporter. 🐛';await page.locator('#improvements-review').fill(edited);
  await page.getByRole('button',{name:'Review issue',exact:true}).click();assert.equal(await page.locator('#improvements-open-issue').isDisabled(),true);
  await page.locator('#improvements-evidence-confirm').check();await page.evaluate(()=>{const n=document.querySelector('#improvements-open-issue');n.click();n.click();});
  await page.waitForFunction(()=>fixture.opens.length===1);const url=await page.evaluate(()=>fixture.opens[0]);assert.equal(new URL(url).searchParams.get('body'),edited);assert.equal(await page.locator('#improvements-open-issue').isDisabled(),true);
  assert.equal((await page.evaluate(()=>fixture.calls)).filter(x=>x.m==='turn.start').length,1);
  // Reopening/reviewing unchanged text preserves the durable receipt.
  await page.evaluate(async()=>{fixture.ui.close();await fixture.ui.open();});
  await page.getByRole('button',{name:'Review issue',exact:true}).click();
  await page.locator('#improvements-evidence-confirm').check();assert.equal(await page.locator('#improvements-open-issue').isDisabled(),true);
  assert.equal(await page.evaluate(()=>fixture.opens.length),1);
  // Elected diagnostics rebuild the report from explicitly selected fields.
  await page.locator('#improvements-logs').check();await page.getByRole('button',{name:'Review issue',exact:true}).click();const withLogs=await page.locator('#improvements-review').inputValue();assert.match(withLogs,/hub.rpc/);assert(!withLogs.includes('PRIVATE'));assert.equal(await page.evaluate(()=>fixture.reads),1);
  await page.locator('#improvements-logs').uncheck();await page.getByRole('button',{name:'Review issue',exact:true}).click();assert(!((await page.locator('#improvements-review').inputValue()).includes('hub.rpc')));
  // Clearing persists revoked inclusion and generated report state across reopen.
  await page.locator('#improvements-logs').check();
  await page.locator('.improvements>details').evaluate(n=>n.open=true);
  await page.getByRole('button',{name:'Clear local diagnostics',exact:true}).click();
  await page.evaluate(async()=>{fixture.ui.close();await fixture.ui.open();});
  assert.equal(await page.locator('#improvements-logs').isChecked(),false);
  await page.getByRole('button',{name:'Review issue',exact:true}).click();
  // Full local storage must prevent any further handoff.
  await page.evaluate(()=>fixture.quota=true);await page.locator('#improvements-evidence-confirm').check();await page.locator('#improvements-open-issue').click();assert.match(await page.locator('.improvements [role=status]').textContent(),/full/);assert.equal(await page.evaluate(()=>fixture.opens.length),1);await page.evaluate(()=>fixture.quota=false);
  await page.setViewportSize({width:390,height:844});await page.locator('#improvements-text').fill(description+' mobile');await page.getByRole('button',{name:'Review issue',exact:true}).click();
  const box=await page.locator('.improvements').boundingBox();assert(box.x>=0&&box.x+box.width<=390&&box.y>=0&&box.y+box.height<=844);
  if(process.env.BACKPLANE_REPORT_EVIDENCE){fs.mkdirSync(process.env.BACKPLANE_REPORT_EVIDENCE,{recursive:true});await page.screenshot({path:path.join(process.env.BACKPLANE_REPORT_EVIDENCE,'public-issue-phone.png')});}
  assert.deepEqual(errors,[]);
 }catch(error){if(page) console.error('PUBLIC FIXTURE FAILURE',JSON.stringify({errors,state:await page.evaluate(()=>({status:document.querySelector('.improvements [role=status]')?.textContent,buttons:[...document.querySelectorAll('.improvements button')].map(n=>({text:n.textContent,hidden:n.hidden,disabled:n.disabled,html:n.outerHTML})),target:document.querySelector('#improvements-target')?.value})).catch(()=>null)}));throw error;}finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}
});
