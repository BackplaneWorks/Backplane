// Real Canvas2D pixels and timings, using generated plot-wire chunks only.
// PLAYWRIGHT_MODULE=/path/to/playwright-core/index.mjs CHROME=/path/to/chrome \
//   node test/tools/plot2d_cache.mjs BASELINE_PLOT2D_JS [CANDIDATE_PLOT2D_JS]
// Baseline: git show 30884966e:src/web/plot2d.js > /tmp/plot2d-main.js
import { createServer } from "node:http";
import { readFileSync } from "node:fs";
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || "playwright-core");

const baseline = process.argv[2];
if (!baseline) throw new Error("Provide the uncached baseline source for independent pixel comparisons");
const sources = { "/baseline.js": readFileSync(baseline, "utf8"), "/candidate.js": readFileSync(process.argv[3] || "src/web/plot2d.js", "utf8") };
const server = createServer((req, res) => {
  res.setHeader("Content-Type", "text/javascript");
  if (sources[req.url]) res.end(sources[req.url] + "\nexport { View, chunk };\n");
  else if (req.url === "/solid.js") res.end(readFileSync("src/web/solid.js", "utf8"));
  else if (req.url === "/app.bend") res.end("export default {}; // 3D UI methods are not used in this Canvas2D harness");
  else { res.setHeader("Content-Type", "text/html"); res.end("<!doctype html><body></body>"); }
});
await new Promise(r => server.listen(0, "127.0.0.1", r));
const browser = await chromium.launch({ executablePath: process.env.CHROME, headless: true, args: ["--no-sandbox", "--disable-gpu"] });
let failed = false;
try {
  const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  const result = await page.evaluate(async () => {
    const [base, next] = await Promise.all([import("/baseline.js"), import("/candidate.js")]);
    window.ResizeObserver = class { observe() {} disconnect() {} };
    const canvas = () => { const c = document.createElement("canvas"); c.style.cssText = "width:480px;height:320px"; document.body.append(c); return c; };
    const ref = new base.View(canvas(), "#0d0f12"), v = new next.View(canvas(), "#0d0f12");
    let frames = 0;
    v.later = () => frames++;
    ref.later = () => {};
    if (v.idle) v.idle = () => {};
    const num = (n, out) => { do { const b = n % 128; n = Math.floor(n / 128); out.push(b | (n ? 128 : 0)); } while (n); };
    const signed = (n, out) => num(n < 0 ? -2 * n - 1 : n * 2, out);
    // Asymmetric copper polygons, line-stroke lettering and drill dots.
    function fixture(count, color = 0xd18a37) {
      const out = [], text = []; let px = 0, py = 0, tx = 0, ty = 0;
      for (let i = 0; i < count; i++) {
        const x = (i * 719) % 46000, y = (i * 997) % 30000;
        num(i, out); num(3, out);
        for (const [a, b] of [[x, y], [x + 330, y + 90], [x + 75, y + 260]]) { signed(a - px, out); signed(b - py, out); px = a; py = b; }
        if (i % 10 === 0) { num(i, text); num(4, text); for (const [a,b] of [[x,y],[x,y-350],[x+150,y-350],[x+150,y-180]]) { signed(a-tx,text);signed(b-ty,text);tx=a;ty=b; } }
      }
      return { box: [0,0,48000,32000], chunks: [next.chunk({ m:0, c:color, l:0, d:new Uint8Array(out) }), next.chunk({ m:1, w:20, c:0xf5eeee, l:1, d:new Uint8Array(text) })] };
    }
    const check = (name, ok, details) => checks.push({ name, ok, details });
    const checks = [];
    const set = p => { ref.plot = v.plot = p; for (const c of p.chunks) c.born = -1000; ref.fade = v.fade = 0; ref.fit(); v.fit(); v.draw(); if (v.bake) v.bake(v.plot, v.look || `${v.off} ${v.dim} ${v.lift} ${v.canvas.width} ${v.canvas.height} ${window.devicePixelRatio}`, v.canvas.width, v.canvas.height, window.devicePixelRatio); };
    const pixels = c => c.getContext("2d").getImageData(0,0,c.width,c.height).data;
    const compare = name => {
      Object.assign(ref, { scale:v.scale, ox:v.ox, oy:v.oy, off:v.off, dim:v.dim, lift:v.lift, fade:v.fade }); ref.plot = v.plot; ref.draw();
      const a=pixels(ref.canvas), b=pixels(v.canvas); let different=0, max=0;
      for (let i=0;i<a.length;i++) if (a[i] !== b[i]) { different++; max=Math.max(max,Math.abs(a[i]-b[i])); }
      check(name, different === 0, { different, max });
    };
    const settle = async () => { await new Promise(r=>setTimeout(r,150)); v.draw(); };
    set(fixture(700)); compare("initial rested pixels");
    v.ox += 0.375; v.oy -= 0.25; v.draw(); await settle(); compare("fractional pan settles to exact pixels");
    v.scale *= 1.37; v.ox -= 37; v.draw(); await settle(); compare("zoom settles to exact pixels");
    v.ox += 800; v.draw(); await settle(); compare("pan outside cache margin settles");
    v.fit(); v.draw(); compare("fit is exact immediately");
    v.off = 1; v.draw(); compare("layer toggle invalidates immediately");
    v.off = 0; v.dim = 2; v.lift = 1; v.draw(); compare("focus and text layer order invalidate");
    v.plot = fixture(600,0x57bded); v.draw(); compare("incoming geometry invalidates immediately");
    v.fade = 1000; for (const c of v.plot.chunks) c.born=performance.now()-500;
    // Freeze fade time for exact comparison through real draw/paint paths.
    const now = performance.now.bind(performance), frozen=now(); performance.now = () => frozen;
    v.draw(); compare("fade setting and newly born geometry invalidate"); performance.now = now;
    v.fade = 0; v.draw(); if (v.bake) v.bake(v.plot, v.look || `${v.off} ${v.dim} ${v.lift} ${v.canvas.width} ${v.canvas.height} ${window.devicePixelRatio}`, v.canvas.width, v.canvas.height, window.devicePixelRatio);
    Object.defineProperty(window,"devicePixelRatio",{ configurable:true, value:2 }); v.draw(); compare("DPR invalidates bitmap");
    ref.canvas.style.width = v.canvas.style.width = "500px"; v.draw(); compare("resize invalidates bitmap");
    Object.defineProperty(window,"devicePixelRatio",{ configurable:true, value:1 });
    const pin=document.createElement("span"); pin.dataset.x="10";pin.dataset.y="5";v.canvas.parentElement.append(pin);v.pins();
    check("pins follow camera in millimetres", Math.abs(parseFloat(pin.style.left)-(v.ox+10000*v.scale))<0.001 && Math.abs(parseFloat(pin.style.top)-(v.oy+5000*v.scale))<0.001);
    let location; next.onPlace(p => location = p);
    const rect = v.canvas.getBoundingClientRect();
    v.canvas.dispatchEvent(new MouseEvent("contextmenu",{clientX:rect.left+120,clientY:rect.top+80,cancelable:true}));
    const [hitX,hitY] = location.split("\t").map(Number);
    check("comment placement uses current camera", Math.abs(hitX-(120-v.ox)/v.scale/1000)<0.0001 && Math.abs(hitY-(80-v.oy)/v.scale/1000)<0.0001);
    await settle(); const before=frames; await new Promise(r=>setTimeout(r,300)); check("no continuous idle repaint",frames===before,{before,frames});
    // Identical synthetic dense geometry and camera sequence on this process.
    const dense=fixture(30000); for (const c of dense.chunks)c.born=-1000;
    const bench = (o, cache) => {
      o.plot=dense;o.fade=0;o.off=0;o.dim=0;o.lift=255;o.fit();
      const flush=()=>o.canvas.getContext("2d").getImageData(0,0,1,1);
      let t=performance.now();o.draw();flush();const first=performance.now()-t;
      if(cache){o.bake(o.plot, o.look || `${o.off} ${o.dim} ${o.lift} ${o.canvas.width} ${o.canvas.height} ${window.devicePixelRatio}`,o.canvas.width,o.canvas.height,window.devicePixelRatio);}
      const pan=[],zoom=[];
      for(let i=0;i<45;i++){o.ox+=1.125;o.oy+=0.375;t=performance.now();o.draw();flush();pan.push(performance.now()-t);}
      for(let i=0;i<45;i++){o.scale*=1.002;t=performance.now();o.draw();flush();zoom.push(performance.now()-t);}
      o.off=1;t=performance.now();o.draw();flush();const toggle=performance.now()-t;
      const p95=a=>a.sort((a,b)=>a-b)[Math.floor(a.length*.95)];return {first,toggle,panP95:p95(pan),zoomP95:p95(zoom)};
    };
    const baseline=bench(ref,false),candidate=bench(v,!!v.paint);
    return {checks,benchmark:{polygons:30000,baseline,candidate}, userAgent:navigator.userAgent};
  });
  console.log(JSON.stringify(result,null,2)); failed=result.checks.some(c=>!c.ok);
} finally { await browser.close(); server.close(); }
if (failed) process.exitCode=1;
