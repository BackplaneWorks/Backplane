// Real browser input -> UI adapter -> helper -> remote Chromium, without a live profile.
// bun tools/browser/test/input.ts (install Playwright's chromium/firefox/webkit first)
import { chromium, firefox, webkit } from 'playwright-core';
import { mkdtempSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
const root = mkdtempSync(join(tmpdir(), 'bp-input-'));
const child = Bun.spawn(['bun', fileURLToPath(new URL('../src/main.ts', import.meta.url)), '--profile', join(root, 'profile')], {stdin:'pipe', stdout:'pipe', stderr:'inherit', env:{...process.env,BACKPLANE_CHROME:chromium.executablePath()}});
const pending = new Map<number, (r:any)=>void>(); let id = 0;
(async()=>{ let rest=''; for await(const bytes of child.stdout) { rest += new TextDecoder().decode(bytes); let i; while((i=rest.indexOf('\n'))>=0) { const r=JSON.parse(rest.slice(0,i)); rest=rest.slice(i+1); pending.get(r.id)?.(r); pending.delete(r.id); } } })();
function ask(op:string, args:any={}, page:string|null='thread-a'):Promise<any> { return new Promise((resolve,reject)=>{
  const n=++id; const timer=setTimeout(()=>reject(new Error('timeout '+op)),20000);
  pending.set(n,r=>{clearTimeout(timer); r.ok ? resolve(r.result) : reject(new Error(r.error));});
  child.stdin.write(JSON.stringify({id:n,op,...(page === null ? {} : {page}),...args})+'\n');
}); }
const remote = '<style>body{margin:0}input,textarea{position:absolute;left:10px;width:250px;height:30px}#a{top:10px}#b{top:80px}#c{top:150px}</style><input id=a><input id=b><textarea id=c></textarea>';
const html = `<!doctype html><div id=root><img id=frame width=640 height=400 tabindex=0 data-owner=thread-a data-pt=bb-click data-wheel=bb-scroll data-keyto=bb-key data-type=bb-type src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='640' height='400'/%3E"></div><script type=module>
import {installBrowserInput} from '/input.js'; window.owner='thread-a'; window.sent=[];window.inflight=[];
installBrowserInput(document,(action,value)=>{window.sent.push({action,value});window.inflight.push(window.forward(action,value));},()=>window.owner);window.ready=true;
</script>`;
const server=Bun.serve({port:0,fetch(req){return new Response(new URL(req.url).pathname==='/input.js'?readFileSync(new URL('../../../src/web/browser-input.js',import.meta.url),'utf8'):html,{headers:{'content-type':new URL(req.url).pathname==='/input.js'?'text/javascript':'text/html'}});}});
try {
  for(const [name, engine] of Object.entries({chromium,firefox,webkit})) {
    const browser=await engine.launch();
    try {
      const page=await browser.newPage({viewport:{width:800,height:600}});
      await page.exposeFunction('forward', (action:string,value:string)=>{
        const op=({'bb-click':'click','bb-type':'type','bb-key':'press','bb-scroll':'scroll'} as any)[action];
        const [x,y]=value.split(' ').map(Number);
        return ask('input',{action:op,...(op==='click'?{x,y}:op==='type'?{text:value}:op==='press'?{key:value}:{dy:Number(value)})});
      });
      await ask('open',{url:'data:text/html,'+encodeURIComponent(remote),width:640,height:400});
      await page.goto(`http://127.0.0.1:${server.port}`); await page.waitForFunction(()=>window.ready);
      const drain=()=>page.evaluate(()=>Promise.all(window.inflight));
      const read=(field:string)=>ask('evaluate',{expression:`document.getElementById('${field}').value`});
      const click=(y:number)=>page.locator('#frame').click({position:{x:50,y}});
      await click(25); await page.keyboard.type('hello'); await drain(); assert.equal(await read('a'),'hello');
      await page.keyboard.press('Shift+ArrowLeft'); await page.keyboard.type('!'); await drain(); assert.equal(await read('a'),'hell!');
      await page.keyboard.press(process.platform==='darwin'?'Meta+a':'Control+a'); await page.keyboard.insertText('café 日本語 😀'); await drain(); assert.equal(await read('a'),'café 日本語 😀');
      await page.locator('textarea').evaluate(el=>{ const e=new Event('paste',{bubbles:true,cancelable:true});Object.defineProperty(e,'clipboardData',{value:{getData:()=> ' pasted text'}});el.dispatchEvent(e); });
      await drain(); assert.equal(await read('a'),'café 日本語 😀 pasted text');
      await page.keyboard.press('Tab');await page.keyboard.type('second');await drain();assert.equal(await read('b'),'second');
      await page.keyboard.press('Backspace');await drain();assert.equal(await read('b'),'secon');
      // No wait between selecting another field and typing into it.
      await click(165);await page.keyboard.type('line1');await page.keyboard.press('Enter');await page.keyboard.type('line2');await drain();assert.equal(await read('c'),'line1\nline2');
      await page.locator('textarea').evaluate(el=>{
        el.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));
        el.value='\u200b漢';el.dispatchEvent(new InputEvent('input',{bubbles:true,isComposing:true,data:'漢'}));
        el.value='\u200b漢字';el.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true,data:'漢字'}));
        el.dispatchEvent(new InputEvent('input',{bubbles:true,data:'漢字'}));
      }); await drain();assert.equal(await read('c'),'line1\nline2漢字');
      await page.locator('textarea').evaluate(el=>{
        el.dispatchEvent(new InputEvent('beforeinput',{bubbles:true,cancelable:false,inputType:'deleteContentBackward'}));
        el.value='';el.dispatchEvent(new InputEvent('input',{bubbles:true,inputType:'deleteContentBackward'}));
      }); await drain();assert.equal(await read('c'),'line1\nline2漢');
      const before=await page.evaluate(()=>window.sent.length);
      await page.evaluate(()=>{window.owner='thread-b';document.getElementById('frame').setAttribute('data-owner','thread-b');});
      await page.keyboard.type('stale'); await drain(); assert.equal(await page.evaluate(()=>window.sent.length),before);
      console.log('ok '+name+': typing, selection, primary shortcut, Unicode, paste, Tab, deletion, multiline, composition, stale-owner rejection');
    } finally { await browser.close(); }
  }
  // Queue click (including viewport conversion), typing and Enter without waiting.
  await ask('open',{url:'data:text/html,'+encodeURIComponent(remote),width:640,height:400});
  const batch=[ask('input',{action:'click',x:80,y:60}),...Array.from('ordered').map(text=>ask('input',{action:'type',text})),ask('input',{action:'press',key:'Tab'}),ask('input',{action:'type',text:'next'})];
  await Promise.all(batch);
  assert.deepEqual(await ask('evaluate',{expression:'[a.value,b.value]'}),['ordered','next']);
  await assert.rejects(ask('input',{action:'evaluate',expression:'1'}),/unknown input/);
  await ask('input',{action:'type',text:' after error'});
  assert.equal(await ask('evaluate',{expression:'b.value'}),'next after error');
  console.log('ok ordered input and queue recovery after failure');
} finally {
  try {
    // No page means close the whole helper and await Chromium's shutdown.
    // Killing Bun on Windows can leave Chromium holding the profile open.
    await ask('close', {}, null);
    await child.exited;
  } finally {
    if (child.exitCode === null) { child.kill(); await child.exited; }
    server.stop(true);
    // Allow Windows a bounded interval to release recently closed handles.
    rmSync(root, {recursive:true, force:true, maxRetries:10, retryDelay:100});
  }
}
console.log('ok browser shutdown and temporary profile cleanup');
