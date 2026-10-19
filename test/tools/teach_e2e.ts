// End to end: teaching a bot a browser task (docs/teach.md) on a headless
// hub with the real browser helper: the person's input through
// bots.browser, the recorder's draft (a password's value never in it, the
// log or the home), save with a parameter, the bot's task_* tools, a pause
// (its ask, its screenshot, the answer handing the browser back), and the
// bot's own preview_* calls refused while the person drives.
//
//   bun test/tools/teach_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane; WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire). Needs bun and Chrome (the helper
// downloads one when none is installed). The helper runs from
// tools/browser/src/main.ts through a shim on PATH. No real agent runs.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 600)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function until<T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
}
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

// the site the person teaches on: a search field, a password field, a
// button that posts both (a form post, so the password is never in a URL)
// and a link, all at fixed places so a click can be aimed in thousandths
const SECRET = "hunter2-e2e";
const site = Bun.serve({
  port: 0,
  async fetch(req) {
    const u = new URL(req.url);
    if (u.pathname === "/results") {
      let q = u.searchParams.get("q") ?? "";
      if (req.method === "POST") q = new URLSearchParams(await req.text()).get("q") ?? "";
      return new Response(`<!doctype html><title>Results</title><h1>Order ${q}</h1><p id=status>Shipped</p><a href="/">Back</a>`, { headers: { "content-type": "text/html" } });
    }
    return new Response(
      `<!doctype html><title>Orders</title>
<style>body{margin:0}input,button,a{position:absolute;left:100px;width:300px;height:30px;box-sizing:border-box}</style>
<h2 style="position:absolute;left:100px;top:20px;margin:0">Find an order</h2>
<form method="post" action="/results">
<label for="q" style="position:absolute;left:100px;top:80px">Order</label>
<input id="q" name="q" type="text" style="top:100px">
<label for="pw" style="position:absolute;left:100px;top:140px">Password</label>
<input id="pw" name="pw" type="password" style="top:160px">
<button type="submit" style="top:220px;width:120px">Search</button>
</form>
<a href="/results?q=link" style="top:300px">All orders</a>`,
      { headers: { "content-type": "text/html" } },
    );
  },
});
const siteUrl = `http://127.0.0.1:${site.port}/`;
// thousandths of a 1280x800 frame at the centre of a box at (100, top) 300x30
const aim = (top: number, w = 300) => ({ x: Math.round(((100 + w / 2) / 1280) * 1000), y: Math.round(((top + 15) / 800) * 1000) });

const root = mkdtempSync(join(tmpdir(), "bp-teach-e2e-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
writeFileSync(join(fake, "backplane-browser"), `#!/bin/sh\nexec bun ${resolve("tools/browser/src/main.ts")} "$@"\n`);
chmodSync(join(fake, "backplane-browser"), 0o755);
const home = join(root, "home");
const user = join(root, "user");
mkdirSync(user);
const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, HOME: user, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: process.env.E2E_LOG ? "inherit" : "ignore",
});

function walk(dir: string, skip: string[], out: string[] = []): string[] {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (skip.includes(e.name)) continue;
    if (e.isDirectory()) walk(p, skip, out);
    else out.push(p);
  }
  return out;
}

try {
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  const seen: any[] = [];
  const frames: string[] = [];
  const info: Record<string, string> = {};
  const replies = new Map<number, any>();
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const text = W.decode(new Uint8Array(e.data as ArrayBuffer));
    frames.push(text);
    const o = JSON.parse(text);
    for (const c of o.items ?? []) seen.push(c);
    if (o.info) Object.assign(info, o.info);
    if (o.t === "reply") replies.set(Number(o.id), o);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let n = 0;
  const rpc = async (m: string, p: object) => {
    const id = ++n;
    ws.send(W.encode(JSON.stringify({ id, m, p })));
    return await until(15000, () => replies.get(id));
  };
  const change = (tag: string, f: (c: any) => boolean = () => true) => until(10000, () => seen.find((c) => c.$ === tag && f(c)));
  const base = `http://127.0.0.1:${port}`;
  const mcp = (th: string, name: string, args: object) =>
    fetch(`${base}/mcp/${th}`, {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 9, method: "tools/call", params: { name, arguments: args } }),
    }).then((r) => r.json());
  const text = (r: any) => String(r?.result?.content?.[0]?.text ?? "");

  const created = await rpc("bots.create", { name: "miso" });
  const set = await change("BotSet", (c) => c.id === created?.bot);
  const bot: string = created?.bot ?? "";
  const thread: string = set?.thread ?? "";
  check("a bot to teach", !!bot && !!thread, created);

  // Ordinary thread viewers use the same control path, scoped to their page.
  await rpc("project.add", { path: root });
  const project = await change("ProjectCreated", c => c.root === root || c.path === root);
  // Different model versions name the path differently; use this test's only project.
  const projectId = project?.id ?? seen.find(c => c.$ === "ProjectCreated")?.id;
  await rpc("thread.create", { project: projectId, title: "Browser input" });
  const ordinary = await change("ThreadCreated", c => c.title === "Browser input");
  const owner = ordinary?.id ?? "";
  check("ordinary browser thread created", !!owner);
  await mcp(owner, "preview_open", { url: siteUrl });
  check("ordinary thread takes control", (await rpc("bots.browser", {bot:owner,op:"takeover"}))?.ok === true);
  check("ordinary agent cannot race user input", (await mcp(owner,"preview_evaluate",{expression:"1"}))?.result?.isError === true);
  const orderedInput = await Promise.all([
    rpc("bots.browser", {bot:owner,op:"click",...aim(100)}),
    rpc("bots.browser", {bot:owner,op:"type",text:"first"}),
    rpc("bots.browser", {bot:owner,op:"press",key:"Tab"}),
    rpc("bots.browser", {bot:owner,op:"type",text:"second"}),
  ]);
  check("ordinary input acknowledges execution", orderedInput.every(r=>r?.ok === true), orderedInput);
  const badInput = await rpc("bots.browser", {bot:owner,op:"press",key:"NotARealKey"});
  check("input failures reach the viewer", badInput?.ok === false, badInput);
  await rpc("bots.browser", {bot:owner,op:"handback"});
  const values = text(await mcp(owner,"preview_evaluate",{expression:"[document.getElementById('q').value,document.getElementById('pw').value]"}));
  check("click typing and Tab stay ordered", values.includes("first") && values.includes("second"), values);
  check("handback disables ordinary input", (await rpc("bots.browser",{bot:owner,op:"type",text:"late"}))?.ok === false);

  const tools = await fetch(`${base}/mcp/${thread}`, {
    method: "POST", headers: { "content-type": "application/json", accept: "application/json, text/event-stream" }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
  }).then((r) => r.json());
  const names = (tools?.result?.tools ?? []).map((t: any) => t.name);
  check("the bot sees the task tools", ["task_list", "task_get", "task_save", "task_delete", "task_run", "task_step", "task_pause", "task_end"].every((t) => names.includes(t)), names);

  // teach: the person drives, recording
  const r0 = await rpc("bots.browser", { bot, op: "click", x: 500, y: 500 });
  check("input is refused while the bot drives", r0?.ok === false, r0);
  check("the bot drives at first", (info[`drive.${bot}`] ?? "") === "");
  const start = await rpc("bots.teach", { bot, op: "start" });
  check("teach starts", start?.ok === true, start);
  check("teach: info says recording", await until(5000, () => (info[`teach.${bot}`] ?? "").startsWith("rec ")) !== undefined, info);
  check("teach: the person drives", info[`drive.${bot}`] === "teach", info);

  // the bot's own browser calls are refused meanwhile
  const refused = await mcp(thread, "preview_navigate", { url: siteUrl });
  check("preview is refused while teaching", refused?.result?.isError === true && text(refused).includes("The person has the browser"), refused);

  // the person's input: navigate, click the search field and type, the password, submit
  const draft = () => { try { return JSON.parse(info[`teach.draft.${bot}`] || "[]") as any[]; } catch { return []; } };
  await rpc("bots.browser", { bot, op: "navigate", url: siteUrl });
  check("a navigate is recorded", await until(10000, () => draft().find((s) => s.do === "navigate")) !== undefined, draft());
  const q = aim(100), pw = aim(160), go = aim(220, 120);
  await rpc("bots.browser", { bot, op: "click", ...q });
  await rpc("bots.browser", { bot, op: "type", text: "W2026" });
  await rpc("bots.browser", { bot, op: "click", ...pw });
  await rpc("bots.browser", { bot, op: "type", text: SECRET });
  await rpc("bots.browser", { bot, op: "click", ...go });
  const typed = await until(15000, () => draft().find((s) => s.do === "type" && s.value === "W2026"));
  check("the typed value is recorded", !!typed, draft());
  const secret = await until(15000, () => draft().find((s) => s.do === "type" && s.value?.secret));
  check("the password is a secret step with its label", !!secret && typeof secret.value.secret === "string" && secret.value.secret.length > 0, draft());
  check("a click is recorded with its target", draft().some((s) => s.do === "click" && (s.target.name === "Search" || s.target.text === "Search")), draft());
  check("steps are numbered from 1", draft().every((s, i) => s.n === i + 1), draft());
  check("the password is nowhere in the draft", !JSON.stringify(draft()).includes(SECRET) && !(info[`teach.draft.${bot}`] ?? "").includes(SECRET));
  await rpc("bots.teach", { bot, op: "note", text: "the status is on the results page" });
  check("a note joins the draft", await until(5000, () => draft().find((s) => s.do === "note" && s.note.includes("results page"))) !== undefined, draft());

  // stop: review; the draft file is a best effort mirror
  await rpc("bots.teach", { bot, op: "stop" });
  check("stop: review", await until(5000, () => info[`teach.${bot}`] === "review") !== undefined, info);
  const df = join(home, "bots", bot, "tasks", ".draft.jsonl");
  check("the draft is mirrored to a file", await until(5000, () => existsSync(df) && readFileSync(df, "utf8").trim().split("\n").length >= draft().length) !== undefined);

  // save with the typed order number as a parameter
  const orderStep = draft().find((s) => s.do === "type" && s.value === "W2026");
  const noName = await rpc("bots.teach", { bot, op: "save", name: "", params: [] });
  check("save needs a name", noName?.ok === false, noName);
  const saved = await rpc("bots.teach", { bot, op: "save", name: "Order status", description: "Look an order up", params: [{ step: orderStep?.n, name: "order", label: "Order number" }] });
  check("save is accepted", saved?.ok === true, saved);
  const ts = await change("TaskSet", (c) => c.bot === bot && c.slug === "order-status");
  check("a TaskSet is logged", !!ts, seen.filter((c) => c.$ === "TaskSet"));
  const task = JSON.parse(ts?.json ?? "{}");
  check("the task has the parameter", task.params?.[0]?.name === "order" && task.params?.[0]?.example === "W2026", task.params);
  check("the marked value is {{order}}", task.steps?.some((s: any) => s.do === "type" && s.value === "{{order}}"), task.steps);
  check("the task starts at the site", task.start === siteUrl && task.origin === "show", task);
  check("a copy is in the bot's folder", await until(5000, () => existsSync(join(home, "bots", bot, "tasks", "order-status.json"))) !== undefined);
  check("the teach state is cleared", await until(5000, () => info[`teach.${bot}`] === "" && info[`drive.${bot}`] === "") !== undefined, info);
  check("the draft is cleared", !existsSync(df));
  const woke = await change("MessagePosted", (c) => c.thread === thread && String(c.text).startsWith("[taught: Order status]"));
  check("the bot is woken to learn it", !!woke && String(woke.text).includes("task_get"), woke);
  check("the bot may drive again", !JSON.stringify(await mcp(thread, "preview_navigate", { url: siteUrl })).includes("The person has the browser"));

  // the bot's tools
  const list = JSON.parse(text(await mcp(thread, "task_list", {})) || "[]");
  check("task_list", list[0]?.slug === "order-status" && list[0]?.params?.[0] === "order", list);
  const got = JSON.parse(text(await mcp(thread, "task_get", { slug: "order-status" })) || "{}");
  check("task_get", got.slug === "order-status" && Array.isArray(got.steps), got);
  const upd = await mcp(thread, "task_save", { slug: "order-status", description: "Look an order up by number" });
  check("task_save merges", text(upd).includes("order-status") && upd?.result?.isError !== true, upd);
  const got2 = JSON.parse(text(await mcp(thread, "task_get", { slug: "order-status" })) || "{}");
  check("task_save keeps the steps", got2.description === "Look an order up by number" && got2.steps?.length === got.steps?.length, got2);

  const miss = await mcp(thread, "task_run", { slug: "order-status", params: {} });
  check("task_run names a missing parameter", miss?.result?.isError === true && text(miss).includes("missing parameter: order"), miss);
  const run = await mcp(thread, "task_run", { slug: "order-status", params: { order: "W9" } });
  const plan = JSON.parse(text(run) || "{}");
  const runId: string = plan.run ?? "";
  check("task_run answers a run and the plan", /^order-status-\d+$/.test(runId) && plan.plan?.steps?.some((s: any) => s.value === "W9" && s.locators?.length > 0), plan);
  const rdir = join(home, "bots", bot, "tasks", "order-status", "runs", runId);
  check("the run has a folder", existsSync(rdir));
  const act = await change("ActivityLogged", (c) => c.act === `task:${runId}`);
  check("the run is logged as an Act of tone task", !!act && act.tone === "task" && JSON.parse(act.summary).status === "running", act);

  // the page the bot works on: open the site, log a step with a picture
  await mcp(thread, "preview_navigate", { url: siteUrl });
  const step = await mcp(thread, "task_step", { run: runId, n: 1, ok: true, note: "opened", shot: true });
  check("task_step", step?.result?.isError !== true, step);
  check("a step's picture is kept", await until(8000, () => existsSync(join(rdir, "1.jpg")) && statSync(join(rdir, "1.jpg")).size > 100) !== undefined);
  check("the log has the step", readFileSync(join(rdir, "log.jsonl"), "utf8").includes("\"note\":\"opened\""));
  const get = (path: string) => fetch(`${base}${path}`);
  const shot1 = await get(`/bots/${bot}/tasks/shot?run=${runId}&f=1.jpg`);
  const bytes = new Uint8Array(await shot1.arrayBuffer());
  check("the picture is served as a JPEG", shot1.status === 200 && bytes[0] === 0xff && bytes[1] === 0xd8, [shot1.status, bytes.length]);
  check("a path out of the folder is refused", (await get(`/bots/${bot}/tasks/shot?run=..%2F..&f=1.jpg`)).status === 404 && (await get(`/bots/${bot}/tasks/shot?run=${runId}&f=..%2F..%2Fx.jpg`)).status === 404);
  check("another bot's id is refused", (await get(`/bots/x%2F..%2F${bot}/tasks/shot?run=${runId}&f=1.jpg`)).status === 404);

  // a pause: an ask with a screenshot; the person takes over, then answers
  const paused = mcp(thread, "task_pause", { run: runId, kind: "login", question: "Log in to continue?", n: 2 });
  const ask = await change("AskOpened", (c) => c.kind === "browser" && c.tool === "task_pause");
  check("a pause opens a browser ask", !!ask, ask);
  const detail = JSON.parse(ask?.detail ?? "{}");
  check("the ask names the run and its question", detail.run === runId && detail.n === 2 && detail.question === "Log in to continue?" && detail.risky === false && detail.bot === bot, detail);
  check("the ask offers continue, skip, stop", ask?.options === "continue\nskip\nstop", ask?.options);
  const shot2 = await get(detail.shot);
  const b2 = new Uint8Array(await shot2.arrayBuffer());
  check("the pause's screenshot is served", shot2.status === 200 && b2[0] === 0xff && b2[1] === 0xd8, [detail.shot, shot2.status]);
  await rpc("bots.browser", { bot, op: "takeover" });
  check("take over: the person drives", await until(5000, () => info[`drive.${bot}`] === "takeover") !== undefined, info);
  const refused2 = await mcp(thread, "preview_navigate", { url: siteUrl });
  check("preview is refused while the person has taken over", refused2?.result?.isError === true && text(refused2).includes("The person has the browser"), refused2);
  const nav = await rpc("bots.browser", { bot, op: "navigate", url: `${siteUrl}results?q=by-hand` });
  check("the person's input goes through while they drive", nav?.ok === true, nav);
  await rpc("ask.answer", { id: ask?.id, answer: "continue" });
  const answered = await Promise.race([paused, sleep(10000).then(() => null)]);
  check("the answer reaches the bot", !!answered && text(answered).includes("continue"), answered);
  check("answering hands the browser back", await until(5000, () => info[`drive.${bot}`] === "") !== undefined, info);
  check("the bot drives again", !JSON.stringify(await mcp(thread, "preview_snapshot", {})).includes("The person has the browser"));

  // a risky pause confirms
  const risky = mcp(thread, "task_pause", { run: runId, kind: "risky", question: "Place the order?", n: 3, risky: true });
  const ask2 = await change("AskOpened", (c) => c.kind === "browser" && c.id !== ask?.id);
  check("a risky pause offers confirm, skip, stop", ask2?.options === "confirm\nskip\nstop" && JSON.parse(ask2?.detail ?? "{}").risky === true, ask2);
  await rpc("ask.answer", { id: ask2?.id, answer: "stop" });
  const stopped = await Promise.race([risky, sleep(10000).then(() => null)]);
  check("stop reaches the bot", !!stopped && text(stopped).includes("task_end"), stopped);

  // the end
  const end = await mcp(thread, "task_end", { run: runId, status: "stopped", summary: "stopped at the order" });
  check("task_end", end?.result?.isError !== true, end);
  const endAct = await change("ActivityLogged", (c) => c.act === `task:${runId}.end`);
  const fin = JSON.parse(endAct?.summary ?? "{}");
  check("the end is logged with its status and pictures", fin.status === "stopped" && fin.summary === "stopped at the order" && fin.shots?.some((u: string) => u.includes("f=1.jpg")), fin);
  check("a bad run id is refused", (await mcp(thread, "task_step", { run: "../x", n: 1, ok: true }))?.result?.isError === true);

  // the oldest runs are pruned beyond twenty
  const runs = join(home, "bots", bot, "tasks", "order-status", "runs");
  for (let i = 0; i < 22; i++) mkdirSync(join(runs, `order-status-${1000 + i}`), { recursive: true });
  await mcp(thread, "task_end", { run: runId, status: "ok", summary: "again" });
  check("runs beyond twenty are pruned", await until(8000, () => readdirSync(runs).length <= 20) !== undefined, readdirSync(runs).length);

  // delete
  const del = await mcp(thread, "task_delete", { slug: "order-status" });
  check("task_delete", del?.result?.isError !== true && !!(await change("TaskDeleted", (c) => c.slug === "order-status")));
  check("the copy is gone", await until(5000, () => !existsSync(join(home, "bots", bot, "tasks", "order-status.json"))) !== undefined);

  // discard leaves nothing
  await rpc("bots.teach", { bot, op: "start" });
  await rpc("bots.teach", { bot, op: "discard" });
  check("discard clears the state", await until(5000, () => info[`teach.${bot}`] === "" && info[`drive.${bot}`] === "") !== undefined, info);

  // the password is nowhere: not in the log, a frame, or any file of the home
  const needle = Buffer.from(SECRET);
  const hits = walk(home, ["browser-profile"]).filter((p) => readFileSync(p).includes(needle));
  check("the password is in no file of the home", hits.length === 0, hits);
  check("the password reached no client", !frames.some((f) => f.includes(SECRET)));
  ws.close();
} finally {
  proc.kill();
  await proc.exited;
  site.stop(true);
  rmSync(root, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
