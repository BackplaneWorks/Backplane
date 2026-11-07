// bun test/cef/cdp_targets.mjs CDP_PORT [resume]: every target the engine lists (type, url, attached), from the browser's own session; `resume` also sends Runtime.runIfWaitingForDebugger to each non-page target.
const [port, mode] = process.argv.slice(2);
const v = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
const ws = new WebSocket(v.webSocketDebuggerUrl); let n = 0; const wait = new Map();
const send = (method, params = {}, sessionId) => new Promise(r => { const id = ++n; wait.set(id, r); ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) })); setTimeout(() => r({ timeout: true }), 3000) });
ws.onmessage = e => { const m = JSON.parse(e.data); wait.get(m.id)?.(m); };
await new Promise(r => ws.onopen = r);
const t = (await send('Target.getTargets')).result?.targetInfos || [];
for (const x of t) console.log(`${x.type}\t${x.attached}\t${x.url}\t${x.title}\t${x.targetId.slice(0, 8)}`);
if (mode === 'resume') for (const x of t.filter(x => x.type !== 'page' || !x.url)) {
  const a = await send('Target.attachToTarget', { targetId: x.targetId, flatten: true }); const sid = a.result?.sessionId;
  const r = sid ? await send('Runtime.runIfWaitingForDebugger', {}, sid) : a; console.log('resume', x.targetId.slice(0, 8), JSON.stringify(r).slice(0, 120));
}
ws.close();
