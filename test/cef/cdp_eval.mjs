// bun test/cef/cdp_eval.mjs CDP_PORT EXPRESSION: evaluate EXPRESSION in every page target of the embedded Chromium and print `url<TAB>result`, one line per target.
const [port, expr] = process.argv.slice(2);
const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
for (const t of list.filter(x => x.type === 'page')) {
  const out = await new Promise(res => {
    const ws = new WebSocket(t.webSocketDebuggerUrl); const timer = setTimeout(() => { try { ws.close() } catch {} res('(timeout)') }, 4000);
    ws.onopen = () => ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression: expr, returnByValue: true, awaitPromise: true } }));
    ws.onmessage = e => { const m = JSON.parse(e.data); if (m.id === 1) { clearTimeout(timer); ws.close(); res(JSON.stringify(m.result?.result?.value ?? m.error ?? m.result)) } };
    ws.onerror = () => { clearTimeout(timer); res('(error)') };
  });
  console.log(`${t.url}\t${out}`);
}
