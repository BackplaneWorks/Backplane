// Isolated rendering only. Never attaches to the person's app or hub.
// A red sidebar row says what it wants where its age goes (U.Side.want):
// an approval or a question in red, a failed turn in grey beside a hollow
// square. A red row with no word of its own keeps its plain note.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright-core');
const css=fs.readFileSync(path.resolve(__dirname,'../../src/web/style.css'),'utf8'),out=path.resolve(__dirname,'../../build/sidebar-want');
const row=(id,dot,note)=>`<div class="thread" role="button" data-fixture="${id}"><span role="img" class="dot ${dot}" aria-label="${dot}"></span><span class="title">A long thread title that must stay reachable</span>${note}</div>`;
const want=w=>`<span class="ago want" title="${w} · 1m">${w}</span>`;
(async()=>{const browser=await chromium.launch({headless:true,executablePath:process.env.E2E_CHROME||'/usr/bin/google-chrome'});try{
 fs.mkdirSync(out,{recursive:true});const page=await browser.newPage();await page.route('**/*',route=>route.abort());let checks=0;
 for(const width of [1280,375])for(const [locale,words] of [['en',['approval','question','failed']],['ja',['承認','質問','失敗']]])for(const scale of [100,200]){
  await page.setViewportSize({width,height:600});
  await page.setContent(`<html lang="${locale}" data-theme="dark" style="--text-scale:${scale}"><head><style>${css}</style></head><body><aside class="sidebar" style="width:${Math.min(264,width)}px">${row('approval','blocked',want(words[0]))}${row('question','blocked',want(words[1]))}${row('failed','failed',want(words[2]))}${row('child','blocked','<span class="ago agents" title="1 agent">1 agent</span>')}${row('age','finished','<span class="ago">5m</span>')}</aside></body></html>`);
  const r=Object.fromEntries(await page.evaluate(()=>[...document.querySelectorAll('.thread')].map(el=>{const dot=el.querySelector('.dot'),d=getComputedStyle(dot),note=el.querySelector('.ago'),box=el.getBoundingClientRect(),side=el.closest('.sidebar').getBoundingClientRect(),root=getComputedStyle(document.documentElement);
   const probe=v=>{const s=document.createElement('span');s.style.color=v;document.body.append(s);const c=getComputedStyle(s).color;s.remove();return c};
   return [el.dataset.fixture,{note:getComputedStyle(note).color,need:probe('var(--st-need)'),dim:probe('var(--dim)'),accent:probe('var(--accent)'),bg:d.backgroundColor,shadow:d.boxShadow,radius:d.borderRadius,text:note.textContent,overflows:el.scrollWidth>el.clientWidth||box.right>side.right+1}]})));
  for(const id of ['approval','question']){assert.equal(r[id].note,r[id].need,id+' is red');assert.notEqual(r[id].bg,'rgba(0, 0, 0, 0)',id+' square is filled')}
  assert.equal(r.failed.note,r.failed.dim,'failed is grey');
  assert.equal(r.failed.bg,'rgba(0, 0, 0, 0)','failed square is hollow');assert(r.failed.shadow.includes('inset'),r.failed.shadow);assert.equal(r.failed.radius,'0px');
  assert.equal(r.child.note,r.child.accent,'a red row without a want keeps its badge colour');
  for(const [id,x] of Object.entries(r))assert(!x.overflows,JSON.stringify({width,locale,scale,id,x}));
  if(locale==='en'&&scale===100)await page.screenshot({path:path.join(out,`want-${width}.png`)});checks++;
 }
 console.log(`ok ${checks} desktop/mobile/locale/text-scale layouts of what a red row wants`);
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1});
