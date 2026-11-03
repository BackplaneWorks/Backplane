// Calibration only: render saved synthetic D.Node trees/CSS in Chromium.
// No Backplane app, hub, event store, agent or service is started.
import {readFileSync,writeFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
const module=process.env.PLAYWRIGHT_MODULE||'/home/h/Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core/index.mjs';
const {chromium}=await import(pathToFileURL(module));
const tree=JSON.parse(readFileSync(process.argv[2],'utf8')),css=readFileSync(process.argv[3],'utf8'),out=process.argv[4];
const escape=s=>String(s).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
const html=n=>n.type==='text'?escape(n.text):`<${n.tag} ${(n.attrs||[]).filter(a=>a.name&&!a.name.startsWith('on')).map(a=>`${a.name}="${escape(a.value)}"`).join(' ')}>${n.kids.map(html).join('')}</${n.tag}>`;
const executablePath=process.env.E2E_CHROME||'/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome';
const start=performance.now(),browser=await chromium.launch({headless:true,executablePath});
const rows=[];
try{for(const width of [320,390,430,1280]){
  const page=await browser.newPage({viewport:{width,height:844}});
  await page.setContent(`<html><head><style>${css}</style></head><body>${html(tree)}</body></html>`);
  await page.evaluate(()=>document.fonts.ready);
  const r=await page.evaluate(()=>{
    const foot=document.querySelector('.composer-foot'),buttons=[...foot.querySelectorAll('button')].filter(e=>e.getBoundingClientRect().height>0),rect=foot.getBoundingClientRect();
    const box=e=>{const b=e.getBoundingClientRect();return{tag:e.tagName,class:typeof e.className==='string'?e.className:'svg',x:b.x,y:b.y,width:b.width,height:b.height,
      clipped:e.scrollWidth>e.clientWidth+1,overflow: b.left<rect.left-1||b.right>rect.right+1};};
    const height=Math.max(...buttons.map(e=>e.getBoundingClientRect().height));
    return {width:innerWidth,footHeight:rect.height,buttonHeight:height,oneButtonTall:rect.height<=height+1,
      textClipping:[...foot.querySelectorAll('*')].filter(e=>e.getClientRects().length&&e.clientWidth>0&&e.scrollWidth>e.clientWidth+1&&getComputedStyle(e).whiteSpace==='nowrap').map(e=>({...box(e),intentionalEllipsis:getComputedStyle(e).textOverflow==='ellipsis'})),
      overflow:[...foot.querySelectorAll('*')].filter(e=>{const b=e.getBoundingClientRect();return b.width&&b.height&&(b.left<rect.left-1||b.right>rect.right+1)}).map(box),
      boxes:[foot,...buttons].map(box)};
  });rows.push(r);await page.close();
}}finally{await browser.close();}
const result={approach:'browser-calibration',seconds:(performance.now()-start)/1000,rows,scope:'saved synthetic production trees/CSS only, no running app'};
if(out)writeFileSync(out,JSON.stringify(result,null,2));console.log(JSON.stringify(result));
