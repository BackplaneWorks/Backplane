// Render the actual Bend page fixture with production CSS and cue. No live hub.
// bun test/tools/star_nudge_browser.mjs [--serve]
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync, mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFileSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';
const out = resolve('build/star-nudge');
mkdirSync(out,{recursive:true});
const baselineCss=execFileSync('git',['show','273b268cd1c56661ead5cc8ba0ae1b5a33ed375f:src/web/style.css']);
const lib = (await import(pathToFileURL(resolve(out, 'fixture.mjs')).href)).default;
const snapshots = Object.fromEntries(['page', 'before', 'dismiss', 'opened'].map(k => [k, JSON.parse(lib[k](0n))]));
const html = `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Star suggestion comparison</title><link rel="stylesheet" href="/style.css"></head><body><div id="root"></div><script type="module">
import {starAttention} from '/star-attention.js';
const data = ${JSON.stringify(snapshots).replaceAll('<', '\\u003c')};
const q = new URLSearchParams(location.search);
function node(n) {
  if(n.type==='text') return document.createTextNode(n.text);
  const el=n.tag==='svg'||n.tag==='path' ? document.createElementNS('http://www.w3.org/2000/svg',n.tag) : document.createElement(n.tag);
  for(const a of n.attrs||[]) if(a.name) el.setAttribute(a.name,a.value); else el.dataset.action=a.action;
  for(const child of n.kids||[]) el.append(node(child));
  return el;
}
const root=document.getElementById('root');root.append(node(data.page));
if(q.has('before')) root.querySelector('.star-nudge').replaceWith(node(data.before));
if(q.get('theme')==='light'){document.documentElement.classList.add('light');root.querySelector('.app').classList.add('light');}
window.syncStar=starAttention();window.syncStar(root);
window.starSnapshot=data;
window.setNudge=on=>{root.querySelector('.star-nudge')?.remove();if(on)root.querySelector('.app').append(node(data.page.kids.find(n=>n.key==='star-nudge')));window.syncStar(root);};
</script></body></html>`;
const server = Bun.serve({hostname:'127.0.0.1',port:process.env.STAR_PREVIEW_PORT ? +process.env.STAR_PREVIEW_PORT : 0,fetch(req){
  const url = new URL(req.url);
  if(url.pathname==='/style.css')return new Response(url.searchParams.has('before') ? baselineCss : readFileSync('src/web/style.css'),{headers:{'content-type':'text/css'}});
  if(url.pathname==='/star-attention.js')return new Response(readFileSync('src/web/star-attention.js'),{headers:{'content-type':'application/javascript'}});
  return new Response(html.replace('href="/style.css"', 'href="/style.css'+(url.searchParams.has('before')?'?before':'')+'"'),{headers:{'content-type':'text/html; charset=utf-8'}});
}});
console.log('Star fixture port',server.port);
if(process.argv.includes('--serve')) await new Promise(()=>{});
else {
  const {chromium}=await import(pathToFileURL(process.env.PLAYWRIGHT_CORE_PATH || '/home/h/.bun/install/cache/playwright-core@1.64.1@@@1/index.mjs').href);
  const browser=await chromium.launch({executablePath:process.env.CHROME_BIN||'/usr/bin/google-chrome-stable',headless:true,args:['--no-sandbox','--disable-dev-shm-usage']});
  const results=[];
  try {
    for(const theme of ['dark','light']) for(const width of [320,360,390,430,1280]) {
      const context=await browser.newContext({viewport:{width,height:width===1280?800:844},reducedMotion:'reduce'});
      const page=await context.newPage();
      for(const mode of ['before','after','solid']) {
        await page.goto('http://127.0.0.1:'+server.port+'/?theme='+theme+(mode==='before'?'&before':''));
        await page.waitForSelector('.star-nudge');
        if(mode==='solid') await page.addStyleTag({content:'.star-nudge{background:var(--accent);color:var(--on-accent)}.star-nudge-icon,.star-nudge-dismiss{color:var(--on-accent)}.star-nudge-open{background:var(--on-accent);color:var(--accent);border-color:var(--on-accent)}'});
        await page.evaluate(()=>document.fonts.ready);
        const geometry=await page.evaluate(()=>{
          const rect=s=>{const r=document.querySelector(s).getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom,right:r.right};};
          const strip=document.querySelector('.star-nudge');
          return {strip:rect('.star-nudge'),composer:rect('.composer'),action:rect('.star-nudge-open'),dismiss:rect('.star-nudge-dismiss'),position:getComputedStyle(strip).position,overflow:document.documentElement.scrollWidth>innerWidth,role:strip.getAttribute('role')};
        });
        assert.equal(geometry.position,'static');assert.equal(geometry.role,'status');
        assert.ok(geometry.composer.bottom<=geometry.strip.y+1,'strip must not cover composer');
        assert.ok(geometry.action.right<=width && geometry.dismiss.right<=width,'actions fit viewport');
        assert.equal(geometry.overflow,false,'page fits viewport');
        if(mode==='after') {
          assert.ok(geometry.action.height>=44 && geometry.dismiss.height>=44,'touch targets');
          await page.locator('textarea').focus();
          const focus=await page.evaluate(()=>{const before=document.activeElement;window.syncStar(document.getElementById('root'));return before===document.activeElement;});
          assert.ok(focus,'attention cue preserves focus');
          assert.equal(await page.locator('.star-nudge-icon').evaluate(el=>el.getAnimations().length),0,'reduced motion');
        }
        if(mode==='after') assert.equal(geometry.composer.height,results.at(-1).composer.height,'strip does not change composer height');
        results.push({theme,width,mode,...geometry});
        if([390,1280].includes(width)) await page.screenshot({path:out+'/'+mode+'-'+theme+'-'+width+'.png'});
      }
      await context.close();
    }
    const page=await browser.newPage({viewport:{width:390,height:844},reducedMotion:'no-preference'});
    await page.goto('http://127.0.0.1:'+server.port+'/');await page.waitForSelector('.star-nudge-icon');
    await page.locator('textarea').focus();
    const cue=await page.evaluate(()=>{
      const el=document.querySelector('.star-nudge-icon'), a=el.getAnimations()[0];
      return {iterations:a?.effect.getTiming().iterations,focus:document.activeElement.tagName};
    });
    assert.equal(cue.iterations,1);assert.equal(cue.focus,'TEXTAREA');
    await page.evaluate(()=>Promise.all(document.getAnimations().map(a=>a.finished)));
    await page.evaluate(()=>{window.setNudge(false);window.setNudge(true);});
    assert.equal(await page.locator('.star-nudge-icon').evaluate(el=>el.getAnimations().length),0,'remount never repeats cue');
    for(const key of ['dismiss','opened']) assert.ok(!JSON.stringify(snapshots[key]).includes('"key":"star-nudge"'),'real Bend '+key+' removes strip');
    // Contact sheets keep all three screenshot options beside each other.
    for(const theme of ['dark','light']) for(const width of [390,1280]) {
      const tile=width===390?390:640, height=width===390?844:400;
      const sheet=await browser.newPage({viewport:{width:tile*3,height:height+42}});
      await sheet.setContent('<html><meta charset="utf-8"><style>body{margin:0;display:flex;background:#13161b;color:#d9dde4;font:14px system-ui}section{width:'+tile+'px;flex:none}p{margin:0;height:42px;display:flex;align-items:center;padding:0 12px}img{display:block;width:100%}</style>'+['before','after','solid'].map((mode,i)=>'<section><p>'+['Before','A · Tinted strip (implemented)','B · Solid accent (preview)'][i]+'</p><img src="data:image/png;base64,'+readFileSync(out+'/'+mode+'-'+theme+'-'+width+'.png').toString('base64')+'"></section>').join('')+'</html>');
      await sheet.screenshot({path:out+'/compare-'+theme+'-'+width+'.png'});
      await sheet.close();
    }
    writeFileSync(out+'/browser-results.json',JSON.stringify({results,cue},null,2));
    console.log('PASS: 30 layout/theme/option cases; focus, 44px targets, reduced motion, one-time cue, open/dismiss');
  } finally {await browser.close();server.stop();}
}
