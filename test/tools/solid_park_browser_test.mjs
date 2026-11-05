// The 3D workbench kept off the page and taken back (viewer-tools.js kept /
// mount, solid.js mount / forget): the same workbench, model, thread and
// board come back with the same preview view and toolbar, catching up with a
// model that changed meanwhile; anything else gets a new one and releases
// the kept one. And the picture a view leaves (solid-pictures.js): shown
// only over the same thread and model at the same size, with its pose, until
// the model draws. Headless Chromium (software GL), no hub.
// node test/tools/solid_park_browser_test.mjs [--browser /usr/bin/chromium] [--playwright DIR] [--web src/web]
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import assert from 'node:assert/strict';
const args = process.argv.slice(2), opt = (n, d = '') => { const i = args.indexOf('--' + n); return i < 0 ? d : args[i + 1]; };
const web = path.resolve(opt('web', 'src/web'));
const server = http.createServer((req, res) => {
  const name = new URL(req.url, 'http://fixture').pathname.slice(1);
  res.setHeader('Content-Type', name.endsWith('.js') || name.endsWith('.bend') ? 'text/javascript' : 'text/html');
  if (name === 'solid.js') res.end(fs.readFileSync(path.join(web, name), 'utf8') + '\nexport {view as testView, parked as testParked, assets as testAssets, pictures as testPictures};');
  else if (name === 'app.bend') res.end('export default {solid_waiting:()=>"Preparing model",solid_stats:(l,size,k,waited,built)=>size+" · "+k+" · built in "+built};');
  else if (/^[\w-]+\.js$/.test(name) && fs.existsSync(path.join(web, name))) res.end(fs.readFileSync(path.join(web, name)));
  else res.end('<style>html,body{margin:0;height:100%}.viewer-stage{position:relative;width:400px;height:300px}canvas{width:100%;height:100%;display:block}</style><div id="page"></div>');
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const pw = await import(opt('playwright', path.resolve('desktop/node_modules/playwright-core/index.mjs')));
const browser = await pw.chromium.launch({ headless: true, executablePath: opt('browser', '/usr/bin/chromium'), args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
try {
  const page = await browser.newPage(), errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('http://127.0.0.1:' + server.address().port);
  const r = await page.evaluate(async () => {
    await new Promise((r) => setTimeout(r, 0));
    const S = await import('/solid.js'), T = await import('/viewer-tools.js'), out = {};
    const bench = (key, scope, boardKey = '') => {
      const root = document.createElement('div'); root.id = 'solid-workbench'; root.className = 'solid-workbench';
      Object.assign(root.dataset, { key, scope, boardKey });
      root.innerHTML = `<div class="solid-toolbar"></div><div class="solid-layout"><div class="viewer-stage"><canvas id="solid" data-key="${key}" data-scope="${scope}"></canvas><div class="viewer-note"></div><div class="viewer-stat"></div></div><aside class="solid-inspector"></aside></div>`;
      return root;
    };
    const show = (root) => { const p = document.getElementById('page'); p.replaceChildren(...(root ? [root] : [])); S.mount(document.getElementById('solid')); T.mount(document.getElementById('solid-workbench'), S, {}); };
    const A = bench('2|r|a.kicad_pcb', 't1');
    show(A);
    const v1 = S.testView, tools = [...A.querySelector('.solid-toolbar').children];
    out.built = tools.length > 0 && !!v1?.gl;
    show(null);
    out.parkedView = S.testParked === v1 && !v1.disposed && S.testView === null;
    out.keptSame = T.kept('2|r|a.kicad_pcb', 't1', '') === A;
    out.keptOtherKey = T.kept('2|r|b.kicad_pcb', 't1', '') === null;
    // (the same model for another thread: mount takes it up for that thread)
    out.keptOtherThread = T.kept('2|r|a.kicad_pcb', 't2', '') === A;
    out.keptOtherBoard = T.kept('2|r|a.kicad_pcb', 't1', '0|r|a.kicad_pcb') === null;
    // a model that came (or changed) while it was away is taken on its return
    const model = { pos: new Float32Array(9), nrm: new Float32Array(9), col: new Float32Array(9), count: 3, box: [0, 0, 0, 1, 1, 1] };
    const record = { key: '2|r|a.kicad_pcb', status: 'ready', model, stats: { bytes: 1, build: 0 }, progress: { completed: 1, total: 1 } };
    S.testAssets.entries.set(record.key, record);
    show(A);
    out.revivedView = S.testView === v1 && S.testParked === null && v1.record === record && v1.count === 3;
    out.revivedTools = tools.every((n, i) => A.querySelector('.solid-toolbar').children[i] === n);
    out.notKeptWhileShown = T.kept('2|r|a.kicad_pcb', 't1', '') === null;
    // another workbench lets the kept one go, graphics context and all
    show(null);
    const B = bench('2|r|b.kicad_pcb', 't1');
    show(B);
    out.otherIsNew = S.testView !== v1 && v1.disposed && v1.gl === null && T.kept('2|r|a.kicad_pcb', 't1', '') === null;
    // the same model in another thread is another workbench
    show(null);
    const v2 = S.testView === null ? S.testParked : null;
    show(bench('2|r|b.kicad_pcb', 't2'));
    out.otherThreadIsNew = !!v2 && v2.disposed && S.testView !== v2;
    // a reset (a new connection) lets the kept one go
    show(null);
    const v3 = S.testParked;
    T.reset();
    out.resetReleases = !!v3 && v3.disposed && S.testParked === null && T.kept('2|r|b.kicad_pcb', 't2', '') === null;
    // another thread showing the same model in the same element (a thread
    // switch with 3D open in both): the workbench and its toolbar are kept,
    // the preview view re-keyed to that thread; a thread that renders
    // differently gets a new workbench
    show(null); T.reset();
    const M = bench('2|r|m.kicad_pcb', 'ta'); show(M);
    const mv = S.testView, mtools = [...M.querySelector('.solid-toolbar').children];
    const rescope = (scope) => { M.dataset.scope = scope; M.querySelector('canvas').dataset.scope = scope; S.mount(M.querySelector('canvas')); T.mount(M, S, {}); };
    rescope('tb');
    out.rescopeKeeps = S.testView === mv && mv.stateKey === 'tb\0' + '2|r|m.kicad_pcb' && mtools.every((n, i) => M.querySelector('.solid-toolbar').children[i] === n);
    const mode = M.querySelector('select.solid-render-mode'); mode.value = 'fast'; mode.dispatchEvent(new Event('change'));
    rescope('ta');
    out.rescopeOtherModeRebuilds = M.querySelector('.solid-toolbar').children[0] !== mtools[0];
    // the mode chosen in tb stayed tb's (not written into ta's settings): back in tb, Fast again
    const atools = [...M.querySelector('.solid-toolbar').children];
    rescope('tb');
    out.rescopeModeIsThreads = M.querySelector('.solid-toolbar').children[0] !== atools[0] && M.querySelector('select.solid-render-mode').value === 'fast';
    // the picture: taken from a drawn model, shown over a new canvas of the
    // same thread and model at the same size (its pose taken up), dropped
    // when the model draws; never for another thread or another size
    show(null); T.reset();
    const P = bench('2|r|a.kicad_pcb', 't1'); show(P);
    const v4 = S.testView; await S.testPictures.ready;
    v4.receive(record); await new Promise(requestAnimationFrame);
    v4.cam.spin(30, 10); v4.takePicture();
    const pic = S.testPictures.get('t1\0' + '2|r|a.kicad_pcb');
    out.pictureTaken = !!pic && pic.url.startsWith('data:image/') && pic.w === 400 && pic.h === 300;
    show(null); T.reset(); S.testAssets.entries.clear();
    const Q = bench('2|r|a.kicad_pcb', 't1'); document.getElementById('page').replaceChildren(Q);
    out.pictureShown = !!S.picture(Q.querySelector('canvas')) && !!Q.querySelector('img.solid-picture');
    S.mount(Q.querySelector('canvas'));
    const v5 = S.testView;
    out.pictureKeepsPose = JSON.stringify(v5.restoreWanted.cam) === JSON.stringify(pic.state.cam);
    out.pictureNoNote = (v5.draw(), Q.querySelector('.viewer-note').textContent === '');
    // the hub's view making the stats line again (empty) under the picture: filled again at once
    const fresh = document.createElement('div'); fresh.className = 'viewer-stat'; Q.querySelector('.viewer-stat').replaceWith(fresh);
    S.mount(Q.querySelector('canvas')); v5.draw();
    out.pictureStatRefilled = fresh.textContent === pic.stat && !!pic.stat;
    record.stats.build = 900; // (this load's own time differs from the picture's)
    S.testAssets.entries.set(record.key, record); v5.receive(record); v5.draw();
    out.pictureDropped = !Q.querySelector('img.solid-picture') && v5.count === 3;
    // the same model keeps the picture's stats line (no late text change)
    out.pictureStatKept = Q.querySelector('.viewer-stat').textContent === pic.stat && !!pic.stat;
    show(null); T.reset();
    const O = bench('2|r|a.kicad_pcb', 't2'); document.getElementById('page').replaceChildren(O);
    out.noPictureOtherThread = S.picture(O.querySelector('canvas')) === null && !O.querySelector('img.solid-picture');
    const W = bench('2|r|a.kicad_pcb', 't1'); W.querySelector('.viewer-stage').style.width = '420px'; document.getElementById('page').replaceChildren(W);
    out.noPictureOtherSize = S.picture(W.querySelector('canvas')) === null;
    // a pose the page remembers for that view, other than the picture's: no picture (the model would move)
    const st = S.testAssets.getState('t1\0' + '2|r|a.kicad_pcb'); st.cam.dist *= 2; S.testAssets.saveState('t1\0' + '2|r|a.kicad_pcb', st);
    const V = bench('2|r|a.kicad_pcb', 't1'); document.getElementById('page').replaceChildren(V);
    out.noPictureOtherPose = S.picture(V.querySelector('canvas')) === null;
    return out;
  });
  assert.deepEqual(errors, []);
  for (const [k, v] of Object.entries(r)) assert.equal(v, true, k + ' ' + JSON.stringify(r));
  console.log('ok solid_park_browser_test ' + JSON.stringify(r));
} finally { await browser.close(); await new Promise((resolve) => server.close(resolve)); }
