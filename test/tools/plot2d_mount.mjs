// Real mount/got, animation frames, fade, resize/DPR observers and disposal.
// PLAYWRIGHT_MODULE=/absolute/playwright-core/index.mjs CHROME=/absolute/chrome \
//   node test/tools/plot2d_mount.mjs BASELINE_PLOT2D_JS [CANDIDATE_PLOT2D_JS]
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const sources = {
  '/baseline.js': readFileSync(process.argv[2], 'utf8') + '\nexport {View};',
  '/candidate.js': readFileSync(process.argv[3] || 'src/web/plot2d.js', 'utf8') + '\nexport {View}; export const current = () => view;',
  '/solid.js': readFileSync('src/web/solid.js', 'utf8'),
  '/app.bend': 'export default {};'
};
const server = createServer((req, res) => {
  res.setHeader('Content-Type', sources[req.url] ? 'text/javascript' : 'text/html');
  res.end(sources[req.url] || '<!doctype html><style>body{margin:0}canvas{width:480px;height:320px}</style><div><canvas id="plot"></canvas></div>');
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const browser = await chromium.launch({ executablePath: process.env.CHROME, headless: true, args: ['--no-sandbox', '--disable-gpu'] });
try {
  const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await page.evaluate(async () => {
    window.B = await import('/baseline.js'); window.C = await import('/candidate.js');
    window.canvas = document.querySelector('#plot');
    const draw = C.View.prototype.draw, bake = C.View.prototype.bake;
    C.View.prototype.draw = function () { this.draws = (this.draws || 0) + 1; return draw.call(this); };
    C.View.prototype.bake = function (...args) { this.bakes = (this.bakes || 0) + 1; return bake.apply(this, args); };
    window.fixture = (key, color=0xe0ab40) => {
      const d=[], put=n=>{do{const b=n%128;n=Math.floor(n/128);d.push(b|(n?128:0));}while(n);}, signed=n=>put(n<0?-2*n-1:2*n);
      let px=0,py=0;
      for (const p of [[[1000,1000],[4500,1700],[1800,8000]],[[6000,2000],[9500,2000],[9500,9000]]]) {
        put(0);put(p.length);
        for(const [x,y] of p){signed(x-px);signed(y-py);px=x;py=y;}
      }
      return {key,fam:'fixture',box:[0,0,10000,10000],cs:[{m:0,c:color,l:0,d:Uint8Array.from(d)}]};
    };
    canvas.dataset.key='first';canvas.dataset.fam='fixture';canvas.dataset.fade='0';
    C.got(fixture('first'));C.mount(canvas);
    window.v=C.current();v.asked=0; // omit arrival telemetry from vector pixel comparisons
    window.snapshot=()=>Array.from(canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data);
    window.exact=()=>{
      const c=document.createElement('canvas');c.style.cssText=canvas.style.cssText;c.style.width=canvas.clientWidth+'px';c.style.height=canvas.clientHeight+'px';document.body.append(c);
      const ref=new B.View(c,'#0d0f12');ref.later=()=>{};
      Object.assign(ref,{plot:v.plot,scale:v.scale,ox:v.ox,oy:v.oy,off:v.off,dim:v.dim,lift:v.lift,fade:v.fade});
      const now=performance.now.bind(performance),frozen=now();performance.now=()=>frozen;ref.draw();performance.now=now;
      const a=c.getContext('2d').getImageData(0,0,c.width,c.height).data,b=canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data;
      const equal=a.length===b.length&&a.every((x,i)=>x===b[i]);c.remove();return equal;
    };
  });
  const checks=[];
  const check=(name,ok)=>{checks.push({name,ok});};
  await page.waitForTimeout(400);
  check('mount paints exact initial pixels and warms cache', await page.evaluate(()=>exact()&&v.cache&&v.bakes>0));
  const counts=await page.evaluate(()=>[v.draws,v.bakes]);
  await page.waitForTimeout(400);
  check('rested production timers stop painting and baking', await page.evaluate(([d,b])=>v.draws===d&&v.bakes===b,counts));
  // Warmed, resting canvas: changing only data-fade must request a frame.
  const beforeFade=await page.evaluate(()=>{for(const c of v.plot.chunks)c.born=performance.now();const before=v.draws;canvas.dataset.fade='120';C.mount(canvas);return before;});
  await page.waitForTimeout(45);
  check('changing only fade schedules a new frame', await page.evaluate(before=>v.draws>before,beforeFade));
  await page.waitForTimeout(400);
  check('completed fade restores exact pixels',await page.evaluate(()=>!v.fading&&exact()));
  await page.evaluate(()=>{canvas.dataset.fade='0';canvas.dataset.off='1';C.mount(canvas);});
  await page.waitForTimeout(50);
  check('mount layer mask redraws exact pixels',await page.evaluate(()=>exact()));
  await page.evaluate(()=>{canvas.dataset.off='0';C.mount(canvas);C.got(fixture('first',0x40b8e0));});
  await page.waitForTimeout(50);
  check('incoming geometry redraws through real got path',await page.evaluate(()=>v.plot.chunks[0].color.startsWith('rgba(64,184,224')&&exact()));
  checks[checks.length-1].details=await page.evaluate(()=>({color:v.plot.chunks[0].color,moving:v.moving,fading:v.fading,exact:exact()}));
  const camera=await page.evaluate(()=>{v.ox+=13.375;v.oy-=7.25;v.touched=true;v.later();return [v.scale,v.ox,v.oy];});
  await page.waitForTimeout(400);
  await page.evaluate(()=>{C.got(fixture('version2',0xc050f0));canvas.dataset.key='version2';C.mount(canvas);});
  await page.waitForTimeout(50);
  check('history version keeps camera and refreshes exact geometry',await page.evaluate(cam=>v.key==='version2'&&[v.scale,v.ox,v.oy].every((x,i)=>x===cam[i])&&exact(),camera));
  await page.evaluate(()=>{canvas.style.width='520px';});
  await page.waitForTimeout(100);
  check('real resize observer requests exact new-sized frame',await page.evaluate(()=>canvas.width===520&&exact()));
  const cdp=await page.context().newCDPSession(page);
  await cdp.send('Emulation.setDeviceMetricsOverride',{width:1000,height:800,deviceScaleFactor:2,mobile:false});
  // CDP changes devicePixelRatio without delivering the OS media event.
  // Deliver that notification through the listener's actual EventTarget.
  await page.evaluate(()=>v.dprQuery.dispatchEvent(new Event('change')));
  await page.waitForTimeout(100);
  check('DPR change notification requests exact new-resolution frame',await page.evaluate(()=>devicePixelRatio===2&&canvas.width===1040&&exact()));
  checks[checks.length-1].details=await page.evaluate(()=>({dpr:devicePixelRatio,width:canvas.width,media:v.dprQuery.media,matches:v.dprQuery.matches,exact:exact()}));
  const disposed=await page.evaluate(()=>{C.mount(null);return [v.draws,v.bakes];});
  await page.evaluate(()=>{canvas.style.width='540px';});
  await page.waitForTimeout(400);
  check('hidden viewer cancels timers and resize observer',await page.evaluate(([d,b])=>C.current()===null&&v.disposed&&v.draws===d&&v.bakes===b,disposed));
  console.log(JSON.stringify({browser:await browser.version(),checks},null,2));
  assert.ok(checks.every(c=>c.ok),'production viewer lifecycle regressions');
} finally {await browser.close();server.close();}
