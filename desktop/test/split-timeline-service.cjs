// Desktop 3D split and long-history regression. Never opens the person's profile or home.
const fs = require('node:fs'), path = require('node:path'), os = require('node:os'), assert = require('node:assert/strict');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright-core');
const { Service } = require('../service.cjs');
const [binary, web, output] = process.argv.slice(2);
assert([binary, web, output].every(path.isAbsolute));
const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-new-messages-')), agents = home + '/agents';
fs.mkdirSync(agents);
fs.cpSync('/usr/share/kicad/demos/multichannel', home + '/demo', { recursive: true });
fs.writeFileSync(home + '/.backplane.json', JSON.stringify({ project: 'demo/multichannel_mixer.kicad_pro' }));
for (const name of ['claude', 'codex', 'grok', 'muse', 'agy', 'opencode', 'pi', 'omp']) fs.writeFileSync(agents + '/' + name, '#!/bin/sh\nexit 127\n', { mode: 0o755 });
fs.writeFileSync(agents+'/codex', `#!/usr/bin/env node
const fs=require('node:fs');const args=process.argv.slice(2);if(args.includes('--version')){console.log('codex-cli 0.0.0');process.exit(0);}
const prompt=args.at(-1)==='-'?fs.readFileSync(0,'utf8'):args.at(-1)||'';const composer=prompt.includes('Fixture composer verification');
fs.appendFileSync(${JSON.stringify(home+'/provider-turns.jsonl')},JSON.stringify({composer,pid:process.pid})+'\\n');
const say=o=>console.log(JSON.stringify(o));say({type:'thread.started',thread_id:'fixture-'+process.pid});
if(composer)say({type:'item.completed',item:{id:'answer-'+process.pid,type:'agent_message',text:'Fixture assistant response for composer verification.'}});
say({type:'turn.completed',usage:{input_tokens:1,output_tokens:1}});
`,{mode:0o755});
const thread = 'chat', bot = 'fixture-bot', events = [
  { $: 'ProjectCreated', id: 'p', title: 'Isolated unread regression', root: home, at: 1 },
  { $: 'ThreadCreated', id: thread, project: 'p', title: 'Fixture chat', env: 'local', branch: '', worktree: home, provider: 'codex', model: '', at: 1 },
  { $: 'ThreadCreated', id: 'control', project: 'p', title: 'Fixture control', env: 'local', branch: '', worktree: home, provider: 'codex', model: '', at: 1 },
  ...(process.env.UNREAD_BOT==='1' ? [{ $: 'BotSet', id: bot, name: 'Fixture bot', thread, look: 0, persona: '', at: 1 }] : []),
];
for (let i = 0; i < 1200; i++) events.push({ $: 'MessagePosted', thread, msg: 'm' + i, role: 'assistant', text: 'Visible history ' + i + '\n\n' + 'Reading context, long wrapped line with engineering details. '.repeat(5 + (i % 7) * 12), at: 2 + i });
const imported=process.env.UNREAD_ENTRIES ? fs.readFileSync(process.env.UNREAD_ENTRIES,'utf8').trim().split('\n').map(JSON.parse) : [];
events.push(...imported);
const messageOrder=new Map(events.filter(e=>e.$==='MessagePosted').map((e,i)=>[e.msg,i]));
const readable=(imported.length?imported:events).filter(e=>e.$==='MessagePosted'&&e.role==='assistant'&&e.text.trim());
const olderTarget=readable.at(-20), normalTarget=readable.at(-30);assert(olderTarget&&normalTarget);
const normalizedText=s=>s.replace(/\[([^\]]+)\]\([^)]*\)/g,'$1').replace(/[^\p{L}\p{N}]+/gu,' ').trim().toLowerCase();
for (const [key, value] of [['restart.continue', 'off'], ['mail.on', 'off'], ['telemetry.share', 'off'], ['onboard.done', '1'], ['onboard.uber', '1']]) events.push({ $: 'SettingSet', key, value, at: 1 });
fs.writeFileSync(home + '/events.jsonl', events.map(JSON.stringify).join('\n') + '\n', { mode: 0o600 });
Object.assign(process.env, { BACKPLANE_HOME: home, XDG_CACHE_HOME: home + '/cache', BACKPLANE_COPY: '1', BACKPLANE_AGENTS: agents, BACKPLANE_KISTACK_SYNC: '0', BACKPLANE_NO_UPDATE: '1', BACKPLANE_TELEMETRY: 'off', BACKPLANE_KICAD_CLI: '/home/h/.backplane-bend/kicad/10.99.0-bp.4/bin/kicad-cli', BACKPLANE_PEERS: '', BACKPLANE_LOCK_DIR: home + '/locks', SHELL: '/bin/sh' });
const service = new Service({ home, binary, web });
let browser, page, step = 'start';
const result = { home, binary, web, state: 'running', snapshots: [], errors: [], wire: [] };
async function snapshot(label, retain = true) {
  const value = await page.evaluate(() => {
    const el = document.getElementById('timeline'), box = el.getBoundingClientRect();
    const viewer = document.querySelector('#viewer'), splitter = document.querySelector('.viewer-split');
    const rect = el => { const r = el?.getBoundingClientRect(); return r && { x: r.x, right: r.right, width: r.width }; };
    return { rects: { timeline: rect(el), viewer: rect(viewer), splitter: rect(splitter) }, hits: [7,4,1].map(off => { const e = document.elementFromPoint(box.right-off, box.top+box.height/2);return { off, tag: e?.tagName, id: e?.id, class: e?.className }; }), gaps: window.__testTimeline?.lastPlan?.items.filter(i => i.key.startsWith('@gap:')), rowTotal: window.__testTimeline?._keys.length, solidKey: document.getElementById('solid')?.dataset.key, position: window.__testTimeline?.lastPlan && { anchor: __testTimeline.lastPlan.anchor, offset: __testTimeline.lastPlan.offset, top: __testTimeline.lastPlan.top, total: __testTimeline.lastPlan.total }, width: box.width, paging: window.__bpRender?.paging?.(), counts: { ...window.__counts }, ready: document.documentElement.dataset.backplaneRemoteReady, top: el.scrollTop, height: el.scrollHeight, view: el.clientHeight,
      pill: el.querySelector('.hold-pill')?.textContent || '', draft: document.getElementById('composer')?.value,
      rows: [...el.querySelectorAll('.timeline-virtual-row')].map(e => ({ key: e.dataset.virtualKey, y: e.getBoundingClientRect().top - box.top, h: e.getBoundingClientRect().height, displayable: e.checkVisibility({checkOpacity:true,checkVisibilityCSS:true}), text: e.innerText.slice(0, 180) })) };
  });
  if(retain) result.snapshots.push({ label, ...value }); return value;
}
(async () => { try {
  const connection = await service.start(); assert(service.child, 'Own isolated child');
  browser = await chromium.launch({ executablePath: process.env.BACKPLANE_CHROME || '/home/h/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome', headless: true, ignoreDefaultArgs: ['--hide-scrollbars'], args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
  const context = await browser.newContext({ viewport: process.env.UNREAD_MOBILE === '1' ? { width: 390, height: 844 } : { width: 2556, height: 1398 } });
  const paired = await context.request.post(connection.origin + '/auth/pair', { headers: { Authorization: 'Bearer ' + await service.token() } }); assert(paired.ok());
  page = await context.newPage(); page.setDefaultTimeout(12000); page.on('pageerror', e => result.errors.push(e.message));
  const Wire = await import('../../src/web/wire.js'), cbor = fs.readFileSync(path.join(__dirname, '../../src/core/cbor.bend'), 'utf8');
  const dictionary = name => JSON.parse(new RegExp('def Cbor\\.' + name + '\\(\\) -> List<&2, String>:\\s*(\\[[\\s\\S]*?\\])').exec(cbor)[1]);
  const keys = dictionary('keys'), words = dictionary('words');
  page.on('websocket', socket => {
    for (const event of ['framesent', 'framereceived']) socket.on(event, frame => {
      try {
        const data = frame.payload, o = typeof data === 'string' ? JSON.parse(data) : Wire.plain(Wire.cbor(data, keys, words));
        const value = typeof o.result === 'string' ? JSON.parse(o.result) : o.result;
        result.wire.push({ resultType: typeof o.result, resultKeys: value && Object.keys(value), window: value?.window && { ids: value.window.ids?.length, newer: value.window.newer, older: value.window.older }, event, type: o.t, method: o.m, id: o.id, away: o.p?.away, cursor: o.p?.cursor, selected: o.p?.session?.sel, target: o.p?.target,
          newer: o.result?.window?.newer, resultSelected: o.result?.session?.sel, resultAway: o.result?.session?.drafts?.['@away.chat'] });
      } catch {}
    });
  });
  await page.addInitScript(() => {
    localStorage.setItem('backplane-selected', 'chat');
    window.__counts = {}; window.__timings = {}; window.__bpRender = { n(k, n = 1) { __counts[k] = (__counts[k] || 0) + n; }, t(k, ms) { (__timings[k] ||= []).push(ms); }, sent() {}, got() {} }; window.__longTasks = []; new PerformanceObserver(list => __longTasks.push(...list.getEntries().map(e => ({ start: e.startTime, duration: e.duration })))).observe({ type: 'longtask', buffered: true });
    window.__pointers = []; document.addEventListener('pointerdown', e => __pointers.push({x:e.clientX,y:e.clientY,target:e.target.className,id:e.target.id}), true);
    window.testSockets = []; const Socket = window.WebSocket;
    window.WebSocket = class extends Socket { constructor(...args) { super(...args); testSockets.push(this); } };
  });
  await page.addInitScript({ path: path.join(__dirname, 'split-viewer-probe.js') });
  // Passive fixture handle: no change to planner/effects, only expose the list.
  await page.route('**/*.js', async route => {
    const response = await route.fetch(); let body = await response.text();
    if (body.includes('this.lastPlan=null')) body = body.replace('this.lastPlan=null', 'globalThis.__testTimeline=this,this.lastPlan=null');
    await route.fulfill({ response, body });
  });
  if (process.env.UNREAD_VIEWER_RACE === '1') await page.routeWebSocket(/\/ws\?view=1/, ws => { const remote=ws.connectToServer(); remote.onMessage(message => setTimeout(() => ws.send(message), 350)); });
  await page.goto(connection.origin);
  await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true' && document.getElementById('composer'));
  for (let i = 0; i < 5 && await page.locator('[data-on-click="ob-skip"]').count(); i++) await page.locator('[data-on-click="ob-skip"]').click();
  if (process.env.UNREAD_BOT === '1') {
    const button = page.locator('[data-on-click="bot"][data-value="' + bot + '"]');
    result.botButtons = await page.locator('[data-value="' + bot + '"]').evaluateAll(es => es.map(e => ({ action: e.getAttribute('data-on-click'), text: e.textContent })));
    await button.first().click();
    await page.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === 'true' && __bpRender.paging().ready && !__bpRender.paging().pending);
  }
  await page.locator('#composer').fill('Unsent 日本語 draft');
  await snapshot('initial');
  let rpc = 1;
  async function append(text) {
    const response = await context.request.post(connection.origin + '/mcp/control', { headers: { Authorization: 'Bearer ' + await service.token() },
      data: { jsonrpc: '2.0', id: rpc++, method: 'tools/call', params: { name: 'thread_send', arguments: { threadId: thread, text } } } });
    assert(response.ok()); assert(!((await response.text()).includes('"isError":true')));
  }
  async function settle() { await page.waitForTimeout(700); }
  async function scroll(dy, times = 1) {
    const box = await page.locator('#timeline').boundingBox();
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    for (let i = 0; i < times; i++) { await page.mouse.wheel(0, dy); await page.waitForTimeout(120); }
    await settle();
  }
  function anchor(s) { return s.rows.find(r => r.key === s.position?.anchor) || s.rows.find(r => !r.key.startsWith('control:') && r.y + r.h > 0 && r.y < s.view); }
  function preserved(before, after, label) {
    const a = anchor(before), b = after.rows.find(r => r.key === a.key);
    result.anchors ||= []; result.anchors.push({ label, key: a.key, before: a.y, after: b?.y });
    assert(b && Math.abs(a.y - b.y) <= 2, label + ' preserves reading anchor');
  }
  step = 'open-3d';
  if (process.env.UNREAD_VIEWER_RACE === '1') {
    const box=await page.locator('#timeline').boundingBox(); await page.mouse.move(box.x+box.width/2,box.y+box.height/2); await page.mouse.wheel(0,-10000);
    await page.waitForFunction(() => __bpRender.paging().paging);
  }
  await page.locator('.workspace-bar [data-on-click="view-kind"][data-value="3d"]:visible').first().click();
  await page.waitForFunction(() => document.querySelector('#solid')?.width > 0 && document.querySelector('#solid')?.dataset.key, {}, { timeout: 60000 });
  await settle(); await snapshot('3d-open');
  if (process.env.UNREAD_VIEWER_RACE === '1') {
    assert((await page.evaluate(() => __counts['view.apply.older'] || 0)) > 0, 'Viewer opening must present the earlier in-flight history bridge');
  }
  if (process.env.UNREAD_HIT_ONLY === '1') { await page.screenshot({path:output+'.split.png'}); result.state='passed'; return; }
  async function reach(target,label) {
    const prefix=normalizedText(target.text).split(' ').slice(0,5).join(' ');
    for(let i=0;i<200;i++) {
      const s=await snapshot(label,false),row=s.rows.find(r=>r.key===target.msg&&r.y>=0&&r.y+Math.min(r.h,s.view)<s.view+1);
      if(row) {
        assert(row.displayable,'expected message row is paintable');assert(normalizedText(row.text).includes(prefix),'visible message contains expected source content');
        const indices=s.rows.filter(r=>messageOrder.has(r.key)).map(r=>messageOrder.get(r.key));assert(indices.every((v,i)=>i===0||v>indices[i-1]),'source messages remain in chronological display order');
        result.expected ||= [];result.expected.push({label,id:target.msg,prefix,y:row.y,h:row.h});await snapshot(label);await page.screenshot({path:output+'.'+label+'.png'});return;
      }
      const box=await page.locator('#timeline').boundingBox();await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.wheel(0,-650);await page.waitForTimeout(150);
    }
    assert.fail('Expected source message never became visible: '+target.msg);
  }
  step = 'older-with-viewer'; await reach(olderTarget,'split-expected-older'); const older = await snapshot('split-older');
  assert(older.rows.some(r => r.y < older.view && r.y + r.h > 0 && r.text.trim()), '3D split scroll mounts visible messages');
  await page.screenshot({ path: output + '.split.png' });
  step = 'resize-split';
  const edge = page.locator('.viewer-split:visible'); const edgeBox = await edge.boundingBox();
  if (edgeBox) {
    await page.mouse.move(edgeBox.x + edgeBox.width / 2, edgeBox.y + edgeBox.height / 2); await page.mouse.down();
    await page.mouse.move(edgeBox.x - 160, edgeBox.y + edgeBox.height / 2, { steps: 12 }); await page.mouse.up(); await settle();
    const resized = await snapshot('split-resized'); preserved(older, resized, 'Split resize');
  }
  step = 'close-viewer'; const beforeClose = await snapshot('before-close');
  await page.locator('.viewer-close:visible').first().click();
  await page.waitForFunction(() => !document.getElementById('viewer') && !__bpRender.paging().pending); await settle(); const closed = await snapshot('closed'); preserved(beforeClose, closed, 'Closing viewer');
  step = 'reopen-viewer';
  await page.locator('.workspace-bar [data-on-click="view-kind"][data-value="3d"]:visible').first().click();
  await page.waitForFunction(() => document.getElementById('viewer') && !__bpRender.paging().pending); await settle();
  const opened = await snapshot('reopened'); preserved(closed, opened, 'Reopening viewer');
  step = 'scrollbar-newer';
  const box = await page.locator('#timeline').boundingBox();
  const native = await page.locator('#timeline').evaluate(el => ({ width: el.offsetWidth - el.clientWidth, top: el.scrollTop, total: el.scrollHeight, view: el.clientHeight }));
  result.scrollbar = native;
  const thumb = Math.max(18, native.view * native.view / native.total), thumbY = box.y + (native.view - thumb) * native.top / Math.max(1, native.total - native.view) + thumb / 2;
  await page.mouse.move(box.x + box.width - 1, thumbY); await page.mouse.down();
  await page.mouse.move(box.x + box.width - 1, box.y + box.height - 5, { steps: 20 }); await page.mouse.up(); await settle();
  const dragged = await snapshot('scrollbar-dragged');
  assert(native.width > 0, 'Native scrollbar must be visible in acceptance');
  assert(dragged.top > native.top + 100, 'Trusted native thumb drag actually moves timeline');
  assert.equal(dragged.rects.viewer.width, opened.rects.viewer.width, 'Scrollbar drag must not resize viewer');
  await scroll(2400, 18); await page.waitForFunction(() => !__bpRender.paging().paging && !__bpRender.paging().newer, {}, { timeout: 30000 }); const newest = await snapshot('split-newest');
  assert.equal(newest.paging.newer, false, 'Manual scroll through 3D split reaches newest history');
  assert(newest.rows.some(r => r.y < newest.view && r.y + r.h > 0 && r.text.trim()), 'Newest split viewport has visible rows');
  step = 'continued-messages'; await append('Continued message while 3D viewer open');
  await page.waitForFunction(() => document.querySelector('#timeline')?.textContent.includes('Continued message while 3D viewer open'));
  const appended = await snapshot('appended-following');
  assert(appended.rows.some(r => r.text.includes('Continued message while 3D viewer open') && r.y < appended.view && r.y + r.h > 0), 'Following split view reveals appended message');
  assert.equal(appended.draft, 'Unsent 日本語 draft');
  step = 'split-reconnect'; await scroll(-450); const beforeReconnect=await snapshot('split-before-reconnect');
  const sockets=await page.evaluate(() => testSockets.length);
  await page.evaluate(() => testSockets.at(-1).close()); await append('Unseen split reconnect message');
  await page.waitForFunction(n => testSockets.length>n && document.documentElement.dataset.backplaneRemoteReady==='true',sockets); await settle();
  const reconnected=await snapshot('split-reconnected-away');preserved(beforeReconnect,reconnected,'Split reconnect');
  assert(reconnected.pill.includes('1 new'), 'Split reconnect keeps genuinely unseen message count');
  await page.locator('#timeline .hold-pill').click({force:true});
  await page.waitForFunction(() => !document.querySelector('#timeline .hold-pill'));
  const reached=await snapshot('split-button-bottom');
  assert(reached.rows.some(r => r.text.includes('Unseen split reconnect message') && r.y<reached.view && r.y+r.h>0), 'Split button reveals unseen reconnect message');
  assert.equal(reached.draft,'Unsent 日本語 draft');
  step='full-width-display'; await page.locator('.viewer-close:visible').first().click();
  await page.waitForFunction(()=>!document.getElementById('viewer')&&!__bpRender.paging().pending);
  await append('Continued message in normal full-width conversation');
  await page.waitForFunction(()=>document.querySelector('#timeline')?.innerText.includes('Continued message in normal full-width conversation'));
  const full=await snapshot('normal-appended');assert(full.rows.some(r=>r.text.includes('Continued message in normal full-width conversation')&&r.displayable&&r.y<full.view&&r.y+r.h>0),'normal full-width message visibly painted');
  await reach(normalTarget,'normal-expected-older');
  await append('Unread message in normal full-width conversation');await page.waitForFunction(()=>document.querySelector('#timeline .hold-pill'));
  await page.locator('#timeline .hold-pill').click({force:true});await page.waitForFunction(()=>!document.querySelector('#timeline .hold-pill'));
  const end=await snapshot('normal-button-bottom');assert(end.rows.some(r=>r.text.includes('Unread message in normal full-width conversation')&&r.displayable&&r.y<end.view&&r.y+r.h>0),'normal full-width button reveals expected new message');assert.equal(end.draft,'Unsent 日本語 draft');
  step='real-composer-provider';
  await page.locator('#composer').fill('Fixture composer verification');await page.locator('#composer').press('Enter');
  await page.waitForFunction(()=>[...document.querySelectorAll('#timeline .msg')].some(e=>e.textContent.includes('Fixture assistant response for composer verification.')));
  await page.waitForFunction(()=>document.getElementById('composer')?.value==='');
  const reply=page.locator('#timeline .msg').filter({hasText:'Fixture assistant response for composer verification.'});assert.equal(await reply.count(),1,'stand-in assistant reply appears exactly once');
  const sent=page.locator('#timeline .msg').filter({hasText:/^Fixture composer verification/});assert.equal(await sent.count(),1,'composer user message appears exactly once');
  const composed=await snapshot('composer-assistant-reply');assert(composed.rows.some(r=>r.text.includes('Fixture assistant response for composer verification.')&&r.displayable&&r.y<composed.view&&r.y+r.h>0),'assistant reply actually visible after real provider turn');
  const turns=fs.readFileSync(home+'/provider-turns.jsonl','utf8').trim().split('\n').map(JSON.parse);assert.equal(turns.filter(t=>t.composer).length,1,'one composer send starts one stand-in provider turn');
  result.provider={standinOnly:true,composerTurns:1,userMessageOnce:true,assistantReplyOnce:true};
  await page.locator('#composer').fill('Unsent 日本語 draft');
  step='target-drafts';
  if (!await page.locator('[data-on-click="select"][data-value="control"]:visible').count()) await page.locator('[data-on-click="toggle-settled"][data-value="p"]:visible').first().click();
  await page.locator('[data-on-click="select"][data-value="control"]:visible').first().click();
  await page.waitForFunction(()=>document.querySelector('main')?.dataset.thread==='control'&&document.documentElement.dataset.backplaneRemoteReady==='true');await page.locator('#composer').fill('Control target unsent draft');
  const action=process.env.UNREAD_BOT==='1'?'bot':'select',value=process.env.UNREAD_BOT==='1'?bot:thread;
  await page.locator('[data-on-click="'+action+'"][data-value="'+value+'"]:visible').first().click();
  await page.waitForFunction(()=>document.getElementById('composer')?.value==='Unsent 日本語 draft'&&document.documentElement.dataset.backplaneRemoteReady==='true');
  assert.equal(await page.locator('#timeline .msg').filter({hasText:'Fixture assistant response for composer verification.'}).count(),1,'returning to target restores its actual reply once');
  result.targetDrafts={preserved:true,kind:process.env.UNREAD_BOT==='1'?'bot':'thread'};
  await snapshot('returned-target');
  assert.deepEqual(result.errors, []); result.state = 'passed';
} catch (e) { result.state = 'failed'; result.step = step; result.error = e.stack; if (page) await snapshot('failure').catch(() => {}); process.exitCode = 1;
} finally {
  if (page) { result.probe = await page.evaluate(() => ({ pointers: __pointers, counts: __viewerScrollProbe.counts, events: __viewerScrollProbe.events, longTasks: __longTasks, timings: __timings })).catch(() => null); await page.screenshot({ path: output + '.final.png' }).catch(() => {}); }
  if (browser) await browser.close(); await service.stop(); fs.writeFileSync(output, JSON.stringify(result, null, 2) + '\n'); console.log(JSON.stringify({ state: result.state, step: result.step, error: result.error, output }));
} })();
