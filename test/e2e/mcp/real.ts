// The MCP end-to-end with REAL agents (docs/MCP.md "Tests"), on a temp hub that test/e2e/mcp/real.sh starts: a stdio server added the way
// Settings > MCP adds it (its key typed into the env goes to the vault), a third-party server pasted as a command, a Claude turn and a Codex turn
// that call them, the key reaching the server and nowhere else, a turned-off tool hidden, and an agent's own mcp_add confirmed by a tap.
// usage: bun real.ts PORT TOKEN_FILE WORKDIR SECRET   (prints one JSON summary; exit 1 on a failed check)
const [port, tokenFile, work, secret] = Bun.argv.slice(2);
const token = (await Bun.file(tokenFile).text()).trim();
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?data=1&enc=json&token=${token}`);
let rid = 0; const wait = new Map<number, (v: any) => void>();
setTimeout(() => { console.error("[mcp-e2e] timed out"); process.exit(2) }, 25 * 60 * 1000);
ws.onerror = (e) => { console.error("[mcp-e2e] socket:", String((e as any).message ?? e)); process.exit(2) };
ws.onmessage = (e) => { const m = JSON.parse(String(e.data)); if (m.t === "reply" && wait.has(m.id)) { wait.get(m.id)!(m); wait.delete(m.id); } };
await new Promise((r) => (ws.onopen = r));
const call = (m: string, p: any = {}): Promise<any> => new Promise((res) => { const id = ++rid; wait.set(id, res); ws.send(JSON.stringify({ id, m, p })); });
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const log = (...a: any[]) => console.error("[mcp-e2e]", ...a);
const checks: Record<string, boolean | string> = {};
const ok = (k: string, v: boolean, why = "") => { checks[k] = v ? true : `FAILED ${why}`; log(k, v ? "ok" : `FAILED ${why}`); };
const probeOut = `${work}/probe.jsonl`;

// 1. add the probe as a person would (Command), its key typed in the env; turn its "forbidden" tool off
let r = await call("mcp.page.act", { action: "mcp-add", value: "", ui: { view: "add", mode: "command" }, fields: { mname: "probe", mcmd: `python3 ${import.meta.dir}/probe_server.py`, menv: `PROBE_TOKEN=${secret}\nPROBE_OUT=${probeOut}` } });
ok("add probe", r.ok === true, JSON.stringify(r.error ?? ""));
r = await call("mcp.page.act", { action: "mcp-tool", value: "probe|forbidden", ui: {}, fields: {} });
ok("turn a tool off", r.ok === true, JSON.stringify(r.error ?? ""));
// 2. a third-party server pasted as the command its docs give
r = await call("mcp.page.act", { action: "mcp-add", value: "", ui: { view: "add", mode: "paste" }, fields: { mpaste: "npx -y @modelcontextprotocol/server-everything" } });
ok("paste everything", r.ok === true, JSON.stringify(r.error ?? ""));
// 3. both checked live: Ready, with their tools
const cards = async () => ((await call("mcp.page", { ui: {}, fields: {} })).result?.rows ?? []).filter((x: any) => x.kind === "card");
let ready = false;
for (let i = 0; i < 90 && !ready; i++) { const cs = await cards(); ready = ["probe", "everything"].every((t) => cs.some((c: any) => c.title === t && c.pill?.text === "Ready")); if (!ready) await sleep(2000); }
const cs = await cards(); log("cards", cs.map((c: any) => `${c.title}: ${c.pill?.text} · ${c.meta}`));
ok("both Ready", ready, JSON.stringify(cs.map((c: any) => [c.title, c.pill?.text])));
ok("probe shows 1 of 2 tools", cs.some((c: any) => c.title === "probe" && String(c.meta).includes("1 of 2 tools")), JSON.stringify(cs.find((c: any) => c.title === "probe")?.meta));

// 4. a real turn on each provider
const proj = (await call("project.add", { path: `${work}/proj`, title: "mcp-e2e" })).result.project.id;
async function turn(provider: string, word: string) {
  const text = `Use the MCP tool \`stamp\` of the server named \`probe\` with word "${word}". Then use the \`echo\` tool of the server named \`everything\` with message "${word}-echo". Reply with exactly what each tool returned, nothing else.`;
  const t = (await call("thread.create", { project: proj, title: `${provider} mcp`, text, provider, runtime: "full-access" })).result?.thread?.id;
  log(provider, "thread", t);
  let last = "";
  for (let i = 0; i < 150; i++) {
    await sleep(3000);
    const e = (await call("thread.entries", { thread: t, limit: 40 })).result;
    const done = e?.thread?.state === "idle" && e.entries.some((x: any) => x.kind === "assistant" && !x.live);
    last = e?.entries?.filter((x: any) => x.kind === "assistant").map((x: any) => x.text).join(" | ") ?? "";
    if (done || e?.thread?.state === "failed") { log(provider, "state", e.thread.state); break }
  }
  log(provider, "said:", last.slice(0, 400));
  const lines = (await Bun.file(probeOut).exists()) ? (await Bun.file(probeOut).text()).trim().split("\n").map((l) => JSON.parse(l)) : [];
  const hit = lines.find((l: any) => l.word === word);
  ok(`${provider} called the probe`, !!hit, JSON.stringify(lines));
  ok(`${provider}: the vault's key reached the server`, hit?.token === secret, hit ? "token differs" : "no call");
  ok(`${provider} called everything's echo`, last.includes(`${word}-echo`), last.slice(0, 200));
  return t;
}
// PROVIDERS="codex" when Claude's login cannot take a turn now (its weekly limit): the agent's own mcp_add is then asked of the first one run
const provs = (process.env.PROVIDERS ?? "claude codex").split(/\s+/).filter(Boolean);
let ct = ""; for (const p of provs) { const t = await turn(p, `${p}-ok`); if (!ct) ct = t; }
const lines = (await Bun.file(probeOut).text()).trim().split("\n").map((l) => JSON.parse(l));
ok("the turned-off tool was never called", !lines.some((l: any) => l.tool === "forbidden"));

// 5. an agent adds a server itself: the person confirms with one tap
await call("message.send", { thread: ct, text: "Use your mcp_add tool to add the Settings > MCP gallery server `time` (gallery: \"time\"). Then end your turn." });
let ask = "";
for (let i = 0; i < 60 && !ask; i++) { await sleep(3000); const e = (await call("thread.entries", { thread: ct, limit: 40 })).result; const a = e?.entries?.find((x: any) => x.kind === "ask" && x.ask?.state === "open"); if (a) { ask = a.ask.id; log("ask:", a.ask.question.slice(0, 200)) } }
ok("mcp_add asked the person", !!ask);
if (ask) {
  const before = (await cards()).some((c: any) => c.title === "Time");
  ok("nothing added before the tap", !before);
  await call("ask.answer", { ask, answer: "accept" });
  let added = false; for (let i = 0; i < 20 && !added; i++) { await sleep(1000); added = (await cards()).some((c: any) => c.title === "Time"); }
  ok("added after the tap", added);
}
console.log(JSON.stringify({ checks }, null, 1));
ws.close();
process.exit(Object.values(checks).every((v) => v === true) ? 0 : 1);
