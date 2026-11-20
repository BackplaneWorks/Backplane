import assert from 'node:assert/strict';
import { checkAccess, checkApprovalRestricted, checkRegisteredRoutes, checkPageHeaders, checkProtectedAssets, checkSessionCookie } from './security.mjs';

// rpc is the synthetic native client's data/1 transport, connected to the
// same temporary hub. Approval goes through the action shown in Devices.
async function deviceButton(rpc, name, action) {
  const { rows } = await rpc('devices.page', {});
  const row = rows.find(row => row.label === name && row.buttons?.some(button => button.action === action));
  assert.ok(row, 'missing ' + action + '; available actions: ' + rows.flatMap(r => (r.buttons || []).map(b => b.action)).join(', '));
  return { row, button: row.buttons.find(button => button.action === action) };
}

export async function browserTasks({ context, origin, rpc, thread, canary, advanceDays, moveFixtureGerbers, assets, replyText, screenshot, nativeShot, readRegistry, checkLogs, checkBoundary, registeredRoutes }) {
  let leaked = false;
  const inspect = bytes => { if (Buffer.from(bytes).includes(canary)) leaked = true; };
  const urls = new Set();
  context.on('request', req => urls.add(req.url()));
  if (process.env.BP_WEB_VIEWERS === '1') await context.addInitScript(() => {
    window.fixtureDraws = 0;
    const draw = WebGL2RenderingContext.prototype.drawArrays;
    WebGL2RenderingContext.prototype.drawArrays = function(mode, first, count) {
      if (mode === this.TRIANGLES && count >= 3) window.fixtureDraws++;
      return draw.call(this, mode, first, count);
    };
  });
  const page = await context.newPage();
  page.on('websocket', ws => {
    urls.add(ws.url());
    ws.on('framereceived', ({payload}) => inspect(payload));
  });
  const name = 'Headless browser';
  const testId = id => page.getByTestId(id);
  await page.setViewportSize({width:1440, height:900});
  await page.goto(origin);
  await testId('pair-card').waitFor();
  await checkAccess(page, true);
  await checkRegisteredRoutes(page, registeredRoutes);
  await checkPageHeaders(page);
  await screenshot(page, 'before-pair');
  await testId('pair-name').fill(name);
  await testId('pair-request-btn').click();
  await testId('pair-code').waitFor();
  const code = (await testId('pair-code').innerText()).replaceAll(' ', '');
  assert.match(code, /^\d{6}$/);
  await screenshot(page, 'pair-code');
  const approval = await deviceButton(rpc, name, 'device-web-approve');
  assert.ok(approval.row.note.replaceAll(' ', '').includes(code), 'browser and native comparison codes differ');
  assert.ok(approval.row.note.includes('person@example.com'), 'native approval omitted verified login');
  if (nativeShot) await nativeShot('native-devices');
  const connected = page.waitForEvent('websocket', socket => new URL(socket.url()).pathname === '/ws');
  const approvedAt = performance.now();
  await rpc('devices.page.act', approval.button);
  await testId('sidebar-list').waitFor();
  assert.ok(performance.now() - approvedAt <= 1000, 'approval to app exceeded one second');
  const socket = await connected;
  await testId('composer-input').waitFor();
  const interactiveMs = await page.evaluate(() => performance.now());
  await checkPageHeaders(page);
  const session = await checkSessionCookie(context, origin, 'https://other.example.test', readRegistry);
  await checkBoundary(session);
  await checkApprovalRestricted(page);
  const storage = await page.evaluate(() => JSON.stringify([document.cookie, {...localStorage}, {...sessionStorage}]));
  assert.ok(!storage.includes(session), 'session leaked into script-readable storage');
  await checkProtectedAssets(page, origin, assets, true);

  await testId('thread-row-' + thread).click();
  await testId('timeline').waitFor();
  await testId('timeline').getByText('Mask check: ' + '•'.repeat(canary.length), {exact:true}).waitFor({timeout:5000});
  const earliest = testId('timeline').getByText('History entry 0', {exact:true});
  await testId('load-older-btn').waitFor();
  assert.equal(await earliest.count(), 0);
  await testId('load-older-btn').click();
  await earliest.waitFor();
  await testId('composer-input').fill('/');
  await testId('slash-menu').waitFor();
  const timeline = testId('timeline');
  await testId('composer-input').fill('mode:stream Browser');
  let firstFrameAt;
  socket.on('framereceived', ({payload}) => {
    if (typeof payload !== 'string') return;
    const frame = JSON.parse(payload);
    if (frame.k === 'text' && frame.thread === thread && frame.append?.includes('Browser0')) firstFrameAt ??= performance.now();
  });
  const partial = timeline.getByText('Browser0', {exact:true}).waitFor({timeout:10000}).then(() => performance.now());
  const sendAt = performance.now();
  await testId('composer-send').click();
  await timeline.getByText('mode:stream Browser', {exact:true}).waitFor();
  const echoMs = performance.now() - sendAt;
  const displayedAt = await partial;
  assert.ok(Number.isFinite(firstFrameAt), 'missing first text frame');
  console.log(JSON.stringify({interactiveMs:Math.round(interactiveMs), sendToEchoMs:Math.round(echoMs),
    firstFrameToVisibleMs:Math.round(displayedAt-firstFrameAt)}));
  await timeline.getByText(replyText, { exact: true }).waitFor();
  await screenshot(page, 'paired-chat');

  if (nativeShot) await nativeShot('native-chat');
  for (const [width, height] of [[820, 1180], [390, 844]]) {
    await page.setViewportSize({width, height});
    await screenshot(page, 'paired-chat-' + width);
  }
  await page.setViewportSize({width:1440, height:900});
  if (process.env.BP_WEB_VIEWERS === '1') {
    const { execFileSync } = await import('node:child_process');
    for (const view of ['board', 'schematic', '3d', 'mech', 'gerbers']) {
      await page.evaluate(() => {window.fixtureDraws = 0;});
      const start = performance.now();
      await testId('rail-' + view).click({timeout:5000});
      const canvas = page.locator('canvas:visible').first();
      await canvas.waitFor({timeout:8000});
      if (view === '3d' || view === 'mech') await page.waitForFunction(() => window.fixtureDraws > 0, null, {timeout:20000});
      let colors = 0;
      do {
        const png = await canvas.screenshot({timeout:4000});
        colors = Number(execFileSync('convert', ['png:-','-resize','240x160','-format','%k','info:'], {input:png}));
        if (colors > 8) break;
        await new Promise(resolve => setTimeout(resolve,100));
      } while (performance.now() - start < 15000);
      assert.ok(colors > 8, view + ' canvas remained blank');
      console.log(JSON.stringify({view, firstVisibleMs:Math.round(performance.now()-start), colors}));
      if (view === '3d') {
        await page.waitForTimeout(2000);
        const draws = await page.evaluate(() => window.fixtureDraws);
        await page.waitForTimeout(1000);
        assert.equal(await page.evaluate(() => window.fixtureDraws), draws, 'idle viewer drew again');
      }
      for (const width of [1440,820,390]) {
        await page.setViewportSize({width,height:width===390?844:900});
        await page.evaluate(() => new Promise(resolve =>
          requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
        await screenshot(page, view + '-' + width);
      }
      await page.setViewportSize({width:1440,height:900});
      await testId('viewer-close-btn').click();
    }
  }
  const revocation = await deviceButton(rpc, name, 'device-revoke');
  const closed = socket.waitForEvent('close', { timeout: 1000 });
  await Promise.all([closed, rpc('devices.page.act', revocation.button)]);
  await testId('pair-card').waitFor();
  await checkProtectedAssets(page, origin, assets, false);
  await screenshot(page, 'revoked');
  for (const absolute of [false, true]) {
    const label = absolute ? 'Absolute expiry' : 'Idle expiry';
    await testId('pair-name').fill(label);
    await testId('pair-request-btn').click();
    await testId('pair-code').waitFor();
    const {button} = await deviceButton(rpc, label, 'device-web-approve');
    await rpc('devices.page.act', button);
    await testId('sidebar-list').waitFor();
    const opened = page.waitForEvent('websocket');
    await page.evaluate(() => new Promise((resolve, reject) => {
      const socket = new WebSocket('/ws?data=1&enc=json');
      window.fixtureBinaryFrames = 0;
      let reply;
      window.fixtureCall = (m, p) => new Promise(r => {reply=r; socket.send(JSON.stringify({id:1,m,p}));});
      socket.onerror = () => reject(Error('expiry fixture connection failed'));
      socket.onmessage = e => {
        if (typeof e.data !== "string") { window.fixtureBinaryFrames++; return; }
        const v = JSON.parse(e.data);
        if (v.t === 'hello') resolve();
        if (v.t === 'reply') reply(v);
      };
    }));
    if (!absolute) {
      const call = (method, params) => page.evaluate(([m,p]) => window.fixtureCall(m,p), [method,params]);
      assert.equal((await call('viewer.files', {thread})).result.hasGerbers, true);
      await moveFixtureGerbers();
      assert.equal((await call('viewer.files', {thread})).result.hasGerbers, false);
      assert.equal((await call('viewer.rail', {thread})).result.has.gerbers, false);
      const watch = await call('viewer.watch', {thread, kind:'gerbers'});
      assert.equal(watch.ok, false);
      assert.match(JSON.stringify(watch.error), /no Gerbers in this project/i);
      // Keep the fresh socket open across a response and a delivery window.
      await call('viewer.files', {thread});
      await new Promise(resolve => setTimeout(resolve, 500));
      assert.equal(await page.evaluate(() => window.fixtureBinaryFrames), 0);
    }
    if (absolute) {
      for (const days of [25,25,25,14]) {
        await advanceDays(days);
        assert.ok((await page.evaluate(() => window.fixtureCall('viewer.follow', {thread:'t1',action:'pause'}))).ok);
      }
    }
    const expired = (await opened).waitForEvent('close', {timeout:5000});
    await advanceDays(absolute ? 1 : 30);
    // Trigger a validity check through a normal read on the still-open test socket.
    await page.evaluate(() => { void window.fixtureCall('thread.entries', {thread:'t1'}); });
    await expired;
    await page.reload();
    await testId('pair-card').waitFor({timeout:5000});
    await checkProtectedAssets(page, origin, assets, false);
  }
  assert.equal(leaked, false, 'raw fixture value visible in browser traffic');
  checkLogs(session);
  for (const address of urls) {
    assert.ok(!address.includes(session) && !new URL(address).searchParams.has('token'), 'credential in a browser URL');
  }
}
