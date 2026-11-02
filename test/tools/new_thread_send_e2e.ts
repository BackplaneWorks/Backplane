// Long pasted messages in empty threads must reach the ordinary send RPC
// even with a busy sidebar. The temporary outbox bubble used to overwhelm
// the view budget: the client rejected the action, reconnected and restored
// the draft before it could dispatch turn.start.
// bun test/tools/new_thread_send_e2e.ts BINARY WEB_DIR
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const [binary, web] = process.argv.slice(2);
assert(binary && web, 'pass BINARY WEB_DIR');
const port = Number(process.env.E2E_PORT || 3934);
assert(port >= 3799 && port <= 65535);
const pw = [resolve('tools/browser/node_modules/playwright-core'), join(homedir(), 'Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core')].find(existsSync);
assert(pw, 'playwright-core is required');
const { chromium } = await import(pw);
const root = mkdtempSync(join(tmpdir(), 'bp-new-thread-send-'));
const home = join(root, 'home'), agents = join(root, 'agents'), project = join(root, 'project');
for (const dir of [home, agents, project]) mkdirSync(dir);
const token = crypto.randomUUID().replaceAll('-', '');
writeFileSync(join(home, 'token'), token, {mode: 0o600});
const at = Math.floor(Date.now() / 1000);
const events: any[] = [
  {$:'ProjectCreated', id:'p', title:'Busy project', root:project, at},
  {$:'SettingSet', key:'restart.continue', value:'off', at},
  {$:'SettingSet', key:'onboard.done', value:'1', at},
  {$:'SettingSet', key:'onboard.uber', value:'1', at},
];
for (let i = 0; i < 45; i++) events.push({$:'ThreadCreated', id:'busy-'+i, project:'p', title:'Other active conversation '+i, env:'local', branch:'', worktree:'', provider:'codex', model:'', at});
for (const id of ['click', 'enter', 'mobile', 'flicker', 'flicker-mobile']) events.push({$:'ThreadCreated', id, project:'p', title:'New thread '+id, env:'local', branch:'', worktree:'', provider:'codex', model:'', at});
writeFileSync(join(home, 'events.jsonl'), events.map(e => JSON.stringify(e)).join('\n')+'\n');
const hub = Bun.spawn([resolve(binary), '--home', home, '--port', String(port), '--host', '127.0.0.1', '--no-tailscale', '--headless', '--foreground', '--web', resolve(web)], {
  env: {...process.env, BACKPLANE_AGENTS:agents, BACKPLANE_LOCK_DIR:join(root, 'locks'), BACKPLANE_KISTACK_SYNC:'0', BACKPLANE_NO_UPDATE:'1', BACKPLANE_TELEMETRY:'off', BACKPLANE_PEERS:'', BACKPLANE_GOOGLE_BAKE:'0', BACKPLANE_ORDER_SERVER:'http://127.0.0.1:9', HOME:join(root, 'user')},
  stdout:'ignore', stderr:Bun.file(join(root, 'hub.err')),
});
const pause = (ms:number) => new Promise(r => setTimeout(r, ms));
async function until(label:string, f:()=>boolean|Promise<boolean>) {
  const end = Date.now()+30000;
  while (Date.now()<end) { if(await f()) return;await pause(50); }
  throw Error('timed out: '+label);
}
let browser:any;
let activePage:any;
let activeErrors:string[] = [];
const connection:string[] = [];
try {
  await until('hub startup', async () => { try { return (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch { return false; } });
  browser = await chromium.launch({headless:true, ...(process.env.E2E_CHROME ? {executablePath:process.env.E2E_CHROME} : {})});
  for (const [id, viewport] of [['click', {width:1500,height:950}], ['enter', {width:1500,height:950}], ['mobile', {width:390,height:844}],
    ['flicker', {width:1500,height:950}], ['flicker-mobile', {width:390,height:844}]] as const) {
    if (process.argv.includes('--flicker-only') && !id.startsWith('flicker')) continue;
    const context = await browser.newContext({viewport, serviceWorkers:'block'});
    const page = await context.newPage(), errors:string[] = [];
    activePage=page;activeErrors=errors;
    await page.addInitScript(() => {
      (globalThis as any).__bpPerf={render:[], act:[], part:[], live:[], trace:[]};
      const probe = (globalThis as any).__sendProbe = { text:'', seen:false, frames:0, gaps:[], duplicates:[], delayed:0 };
      const send = WebSocket.prototype.send;
      WebSocket.prototype.send = function(data) {
        if (!probe.text) return send.call(this, data);
        // Hold each outbound hop so both the action and its later ordinary
        // RPC have time to paint. The old action/ACK handoff loses the row
        // for several frames here even on a fast local test hub.
        probe.delayed++;
        setTimeout(() => { if (this.readyState === WebSocket.OPEN) send.call(this, data); }, 400);
      };
      function frame() {
        if (probe.text) {
          const rows = [...document.querySelectorAll('#timeline .msg.user')].filter(row => row.textContent?.includes(probe.text));
          if (rows.length) probe.seen = true;
          if (probe.seen) {
            probe.frames++;
            if (!rows.length) probe.gaps.push(performance.now());
            if (rows.length > 1) probe.duplicates.push(performance.now());
          }
        }
        requestAnimationFrame(frame);
      }
      requestAnimationFrame(frame);
    });
    page.on('pageerror', (e:any) => errors.push(e.stack || String(e)));
    page.on('console', (m:any) => { if(m.text().includes('bp-connection')) connection.push(m.text());if (/bp-connection.*"event":"(view-error|closed)"/.test(m.text())) errors.push(m.text()); });
    let frame=0;
    if(process.env.E2E_KEEP) page.on('websocket', (ws:any) => {
      for(const event of ['framesent','framereceived']) ws.on(event, (f:any) => writeFileSync(join(root,`${id}-${++frame}-${event}.cbor`),f.payload));
    });
    await page.goto(`http://127.0.0.1:${port}/#token=${token}`);
    await page.waitForSelector('html[data-backplane-remote-ready="true"]');
    await page.evaluate((id:string) => {
      const b = document.createElement('button');b.dataset.onClick='select';b.dataset.value=id;document.body.append(b);b.click();b.remove();
    }, id);
    await page.waitForSelector(`html[data-backplane-remote-ready="true"] main[data-thread="${id}"] #composer`);
    assert.equal(await page.locator('#composer').inputValue(), '');
    // Multi-line prose plus code and non-ASCII text, without private user data.
    const flicker = id.startsWith('flicker');
    const text = flicker ? 'This sent message stays visible through delayed replies. 日本語🙂' : process.env.E2E_MESSAGE_FILE ? readFileSync(process.env.E2E_MESSAGE_FILE, 'utf8') : 'Evaluate this research article.\n\n' + Array.from({length:680}, (_,i)=>`Paragraph ${i}: neural network weights, layout and schematic capture. 日本語`).join('\n') + '\n\n```python\ndef example(x):\n  return x * 2\n```\n\nGive your thoughts.';
    if (!flicker) assert(Buffer.byteLength(text)>42000 && Buffer.byteLength(text)<65536);
    if (flicker) await page.evaluate((text:string) => { (globalThis as any).__sendProbe.text=text; }, text);
    await page.evaluate((text:string) => { const c=document.getElementById('composer') as HTMLTextAreaElement;c.value=text;c.dispatchEvent(new Event('input', {bubbles:true})); }, text);
    await page.locator('#composer').focus();
    if (id==='enter') await page.keyboard.press('Enter');
    else await page.locator('[data-on-click="send"]').click();
    await until('message persisted in '+id, () => readFileSync(join(home, 'events.jsonl'), 'utf8').split('\n').filter(Boolean).some(line => { const e=JSON.parse(line);return e.$==='MessagePosted' && e.thread===id && e.text===text; }));
    await page.waitForFunction(() => (document.getElementById('composer') as HTMLTextAreaElement)?.value==='');
    await pause(250);
    if (flicker) {
      await page.waitForFunction((text:string) => [...document.querySelectorAll('#timeline .msg.user:not(.sending)')].some(row => row.textContent?.includes(text)), text);
      const probe = await page.evaluate(() => (globalThis as any).__sendProbe);
      assert(probe.seen && probe.frames > 10 && probe.delayed >= 2, 'must observe both delayed send hops');
      assert.deepEqual(probe.gaps, [], 'sent message disappeared between animation frames');
      assert.deepEqual(probe.duplicates, [], 'preview overlapped the saved message');
      assert.equal(await page.locator('#timeline .msg.user.sending').count(), 0);
      console.log(`ok ${id}: ${probe.frames} frames, ${probe.delayed} delayed sends, no missing or duplicate message (${viewport.width}px)`);
    }
    assert.deepEqual(errors, []);
    const posted = readFileSync(join(home, 'events.jsonl'), 'utf8').split('\n').filter(Boolean).map(line=>JSON.parse(line)).filter(e=>e.$==='MessagePosted' && e.thread===id && e.role==='user');
    assert.equal(posted.length, 1, 'one send, no duplicates');assert.equal(posted[0].text,text);
    console.log(`ok ${id}: ${Buffer.byteLength(text)} bytes delivered exactly once; composer empty; connection retained (${viewport.width}px)`);
    await context.close();
  }
} catch(error) {
  if(activePage && process.env.E2E_KEEP) {
    await activePage.screenshot({path:join(root,'failure.png')}).catch(()=>{});
    const state=await activePage.evaluate(() => ({composerBytes:(document.getElementById('composer') as HTMLTextAreaElement)?.value.length, text:document.body.innerText.slice(-2000), trace:(globalThis as any).__bpPerf?.trace})).catch(()=>null);
    writeFileSync(join(root,'failure.json'),JSON.stringify({connection,state,errors:activeErrors},null,2));
  }
  throw error;
} finally {
  await browser?.close();hub.kill();await hub.exited;
  if (process.env.E2E_KEEP) console.log(root);else rmSync(root, {recursive:true,force:true});
}
