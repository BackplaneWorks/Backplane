// data/1 over the socket, as a phone speaks it (docs/PHONE-DATA.md): prints hello's features, then each call's result.
//   bun test/swarm/probe.ts HOST PORT TOKEN 'method {json}' ...
const [host, port, token, ...calls] = process.argv.slice(2);
const ws = new WebSocket(`ws://${host}:${port}/ws?data=1&enc=json&token=${token}`);
let id = 0; const wait = new Map<number, (v: any) => void>();
const call = (m: string, p: any) => new Promise<any>(r => { id++; wait.set(id, r); ws.send(JSON.stringify({ id, m, p })); });
ws.onmessage = async (ev) => {
  const o = JSON.parse(String(ev.data));
  if (o.t === "hello") {
    console.log("features", JSON.stringify(o.features));
    for (const c of calls) { const i = c.indexOf(" "); const m = i < 0 ? c : c.slice(0, i); const p = i < 0 ? {} : JSON.parse(c.slice(i + 1)); console.log(m, JSON.stringify(await call(m, p))); }
    ws.close(); process.exit(0);
  } else if (o.t === "reply") { wait.get(o.id)?.(o.ok ? o.result : { error: o.error }); }
};
ws.onerror = (e) => { console.error("socket error", e); process.exit(1); };
setTimeout(() => { console.error("timeout"); process.exit(1); }, 20000);
