// Isolated rendering only. Never attaches to the person's app or hub.
// Commands' thread rows (workspace.bend's WS.row.face): the sidebar's dot,
// the title, what it wants (red for an approval or a question, grey for a
// failure, the row's own colour when selected) and the project at the right.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright-core');
const css=fs.readFileSync(path.resolve(__dirname,'../../src/web/style.css'),'utf8'),out=path.resolve(__dirname,'../../build/palette-needs');
const row=(id,on,dot,want,where)=>`<button type="button" role="option" class="workspace-command${on?' on':''} thread" data-fixture="${id}"><span class="dot ${dot}" role="img"></span><span class="name">A long thread title that must stay reachable in the list</span><span class="want">${want}</span><span class="where">${where}</span></button>`;
(async()=>{const browser=await chromium.launch({headless:true,executablePath:process.env.E2E_CHROME||'/usr/bin/google-chrome'});try{
 fs.mkdirSync(out,{recursive:true});const page=await browser.newPage();await page.route('**/*',route=>route.abort());let checks=0;
 for(const width of [1280,375])for(const scale of [100,200]){
  await page.setViewportSize({width,height:600});
  await page.setContent(`<html lang="en" data-theme="dark" style="--text-scale:${scale}"><head><style>${css}</style></head><body><div class="workspace-palette-backdrop"><section class="workspace-palette"><div id="workspace-commands">${row('approval',true,'blocked','approval','SENSOR-NODE')}${row('question',false,'blocked','question','AMP')}${row('failed',false,'failed','failed','SENSOR-NODE')}${row('plain',false,'finished','','PSU')}<button type="button" class="workspace-command" data-fixture="cmd">New subthread of current thread</button></div></section></div></body></html>`);
  const r=Object.fromEntries(await page.evaluate(()=>[...document.querySelectorAll('[data-fixture]')].map(el=>{const probe=v=>{const s=document.createElement('span');s.style.color=v;document.body.append(s);const c=getComputedStyle(s).color;s.remove();return c};
   const want=el.querySelector('.want'),where=el.querySelector('.where'),dot=el.querySelector('.dot'),box=el.getBoundingClientRect();
   return [el.dataset.fixture,{want:want&&getComputedStyle(want).color,shown:want?getComputedStyle(want).display!=='none':false,need:probe('var(--st-need)'),dim:probe('var(--dim)'),row:getComputedStyle(el).color,
     dotw:dot?dot.getBoundingClientRect().width:0,order:want&&where?want.getBoundingClientRect().right<=where.getBoundingClientRect().left:true,inside:where?where.getBoundingClientRect().right<=box.right+1:true}]})));
  assert.equal(r.question.want,r.question.need,'question is red');assert.equal(r.failed.want,r.failed.dim,'failed is grey');
  assert.equal(r.approval.want,r.approval.row,'the selected row keeps its own colour');
  assert.equal(r.plain.shown,false,'no word, no gap');
  for(const id of ['approval','question','failed','plain']){assert(r[id].dotw>0&&r[id].dotw<16,id+' has its dot');assert(r[id].order,id+' word before its project');assert(r[id].inside,JSON.stringify({width,scale,id,x:r[id]}))}
  if(scale===100)await page.screenshot({path:path.join(out,`palette-${width}.png`)});checks++;
 }
 console.log(`ok ${checks} layouts of Commands' thread rows`);
}finally{await browser.close()}})().catch(e=>{console.error(e);process.exitCode=1});
