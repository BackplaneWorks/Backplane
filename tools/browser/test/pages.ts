// Pages per owner, end to end: bun test/pages.ts
// Uses dist/backplane-browser when BACKPLANE_BROWSER is set, else src/main.ts.
// Serves a small site on 127.0.0.1, drives two owners' pages side by side,
// checks JPEG frames and events per owner, the new ops, and that a cookie
// set in one run is still there after a restart (one persistent profile).

import { existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = mkdtempSync(join(tmpdir(), "bp-pages-"));
const profile = join(root, "profile");
const shots = join(root, "shots");
const rgb = join(root, "frames.rgb");

const server = Bun.serve({
  port: 0,
  hostname: "127.0.0.1",
  fetch(req) {
    const u = new URL(req.url);
    const html = (b: string, headers: Record<string, string> = {}) =>
      new Response(`<!doctype html><title>${u.pathname}</title><body>${b}</body>`, { headers: { "content-type": "text/html", ...headers } });
    if (u.pathname === "/login") return html("logged in", { "set-cookie": "who=bot; Max-Age=3600; Path=/" });
    if (u.pathname === "/me") return html(`<main><h1>Me</h1><p>cookie: ${req.headers.get("cookie") ?? "none"}</p><p>${"words ".repeat(60)}</p></main><nav>menu</nav>`);
    if (u.pathname === "/file.txt") return new Response("hello file\n", { headers: { "content-disposition": "attachment; filename=\"file.txt\"" } });
    if (u.pathname === "/form") return html(`
      <select id=s><option value=a>Apple</option><option value=b>Banana</option></select>
      <input id=f type=file><div id=out></div>
      <a id=dl href="/file.txt">get</a>
      <a id=pop href="/popup" target=_blank>pop</a>
      <script>f.onchange = () => out.textContent = "picked " + f.files[0].name</script>`);
    if (u.pathname === "/popup") return html("popup page");
    if (u.pathname === "/anim") return html(`<div id=d>0</div><script>let i=0;setInterval(()=>d.textContent=++i,50)</script>`);
    return html("home " + u.pathname);
  },
});
const base = `http://127.0.0.1:${server.port}`;

function start() {
  const flags = ["--frames", rgb, "--profile", profile, "--jpeg-dir", shots];
  const cmd = process.env.BACKPLANE_BROWSER
    ? [process.env.BACKPLANE_BROWSER, ...flags]
    : ["bun", new URL("../src/main.ts", import.meta.url).pathname, ...flags];
  const proc = Bun.spawn(cmd, { stdin: "pipe", stdout: "pipe", stderr: "inherit" });
  const waiting = new Map<number, (v: any) => void>();
  const events: any[] = [];
  let nextId = 1;
  (async () => {
    const dec = new TextDecoder();
    let buf = "";
    for await (const chunk of proc.stdout) {
      buf += dec.decode(chunk);
      let nl;
      while ((nl = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, nl);
        buf = buf.slice(nl + 1);
        const msg = JSON.parse(line);
        if (msg.event) events.push(msg);
        else waiting.get(msg.id)?.(msg);
      }
    }
  })();
  function call(op: string, args: Record<string, unknown> = {}): Promise<any> {
    const id = nextId++;
    proc.stdin.write(JSON.stringify({ id, op, ...args }) + "\n");
    proc.stdin.flush();
    return new Promise((res) => waiting.set(id, res));
  }
  return { proc, call, events };
}

let failures = 0;
function check(what: string, ok: boolean, detail: unknown = "") {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}${ok ? "" : " " + JSON.stringify(detail).slice(0, 400)}`);
  if (!ok) failures++;
}
async function ok(p: Promise<any>, what: string) {
  const r = await p;
  check(what, r.ok === true, r);
  return r.result;
}

// run 1
{
  const { proc, call, events } = start();
  await ok(call("open", { url: `${base}/shared`, width: 640, height: 480 }), "shared open");
  await ok(call("frames", { on: true, maxFps: 10 }), "shared raw frames on");
  await ok(call("open", { page: "bot-1", url: `${base}/anim`, width: 800, height: 600 }), "bot-1 open");
  await ok(call("open", { page: "a.b/c", url: `${base}/login` }), "a.b/c open");
  await Bun.sleep(1500);

  const s0 = await ok(call("status"), "shared status");
  const s1 = await ok(call("status", { page: "bot-1" }), "bot-1 status");
  check("shared url unchanged", s0.url === `${base}/shared`, s0.url);
  check("bot-1 url", s1.url === `${base}/anim`, s1.url);
  check("per-owner viewport", s0.width === 640 && s1.width === 800, [s0.width, s1.width]);

  const byPage = (p: string) => events.filter((e) => e.event === "frame" && e.page === p);
  // no client shows it: not cast, a frame after open (and a still every 5 s)
  const n0 = byPage("bot-1").length;
  check("bot-1 unwatched: a frame, not a cast", n0 >= 1 && n0 <= 2, n0);
  await ok(call("cast", { page: "bot-1", ms: 2500 }), "bot-1 cast");
  await Bun.sleep(1500);
  check("bot-1 cast: jpeg events", byPage("bot-1").length - n0 >= 3, byPage("bot-1").length - n0);
  await Bun.sleep(4000);
  const n1 = byPage("bot-1").length;
  await Bun.sleep(2000);
  check("bot-1 lease over: not cast again", byPage("bot-1").length - n1 <= 1, byPage("bot-1").length - n1);
  check("shared jpeg event", byPage("").length >= 1, byPage(""));
  check("a.b/c jpeg event", byPage("a.b/c").length >= 1);
  check("raw frame events have no page", events.some((e) => e.event === "frame" && e.page === undefined && e.w === 640));
  for (const f of ["bot-1.jpg", "shared.jpg", "a_b_c.jpg"]) {
    const p = join(shots, f);
    const good = existsSync(p) && readFileSync(p).subarray(0, 2).equals(Buffer.from([0xff, 0xd8]));
    check(`${f} is a JPEG`, good);
  }
  check("raw frame file", readFileSync(rgb).subarray(0, 4).toString() === "BPF1");
  const seqs = byPage("bot-1").map((e) => e.n);
  check("bot-1 n increases", seqs.every((n, i) => i === 0 || n > seqs[i - 1]), seqs);

  // ops on one owner leave the other alone
  await ok(call("navigate", { page: "bot-1", url: `${base}/form` }), "bot-1 navigate");
  const s0b = await ok(call("status"), "shared status again");
  check("shared unaffected by bot-1 navigate", s0b.url === `${base}/shared`, s0b.url);
  await ok(call("type", { text: "x" }), "type on shared");

  await ok(call("select", { page: "bot-1", selector: "#s", label: "Banana" }), "select");
  const v = await ok(call("evaluate", { page: "bot-1", expression: "s.value" }), "select value");
  check("select chose b", v === "b", v);

  const up = join(root, "up.txt");
  writeFileSync(up, "upload me");
  await ok(call("upload", { page: "bot-1", selector: "#f", path: up }), "upload");
  await ok(call("wait_for", { page: "bot-1", text: "picked up.txt", timeoutMs: 3000 }), "upload seen by page");

  const dl = await ok(call("download", { page: "bot-1", selector: "#dl", dir: join(root, "dl") }), "download");
  check("download saved", dl && readFileSync(dl.path, "utf8") === "hello file\n", dl);

  await ok(call("click", { page: "bot-1", selector: "#pop" }), "open popup");
  await Bun.sleep(500);
  const tabs = await ok(call("tabs", { page: "bot-1" }), "tabs");
  check("popup joined bot-1 tabs and is active", tabs.tabs.length === 2 && tabs.active === 1 && tabs.tabs[1].url.endsWith("/popup"), tabs);
  await ok(call("tabs", { page: "bot-1", close: 1 }), "close popup tab");
  const tabs2 = await ok(call("tabs", { page: "bot-1" }), "tabs after close");
  check("back on the form", tabs2.tabs.length === 1 && tabs2.tabs[0].url.endsWith("/form"), tabs2);

  await ok(call("navigate", { page: "bot-1", url: `${base}/me` }), "bot-1 to /me");
  await ok(call("back", { page: "bot-1" }), "back");
  check("back went to form", (await call("status", { page: "bot-1" })).result.url.endsWith("/form"));
  await ok(call("forward", { page: "bot-1" }), "forward");
  await ok(call("reload", { page: "bot-1" }), "reload");
  const t = await ok(call("text", { page: "bot-1", max: 100 }), "text");
  check("text reads main, capped", t.text.startsWith("Me\n") && t.text.length === 100 && t.truncated && !t.text.includes("menu"), t);
  check("cookie shared across pages", t.text.includes("who=bot"), t.text);

  const shot = join(root, "full.png");
  const ss = await ok(call("screenshot", { page: "bot-1", path: shot }), "screenshot");
  check("screenshot png", ss && readFileSync(shot).subarray(1, 4).toString() === "PNG");
  const pdf = await ok(call("pdf", { page: "bot-1", path: join(root, "p.pdf") }), "pdf");
  check("pdf written", pdf && statSync(pdf.path).size > 100);

  const pages = await ok(call("pages"), "pages");
  check("pages lists three owners", pages.pages.length === 3, pages);

  // a page inside args never picks the owner
  const sneaky = await ok(call("status", { args: { page: "bot-1" } }), "status with page in args");
  check("args.page ignored", sneaky.page === "", sneaky.page);

  await ok(call("close", { page: "a.b/c" }), "close a.b/c page (helper stays)");
  check("a.b/c jpg removed", !existsSync(join(shots, "a_b_c.jpg")));
  const pages2 = await ok(call("pages"), "pages after close");
  check("two owners left", pages2.pages.length === 2, pages2);
  check("helper still alive", (await call("status")).ok === true);

  await call("close");
  await proc.exited;
}

// run 2: the profile kept the cookie
{
  const { proc, call } = start();
  const t = await ok(call("text", { page: "bot-2", url: `${base}/me` }), "run 2 text (blank)");
  await ok(call("navigate", { page: "bot-2", url: `${base}/me` }), "run 2 navigate");
  const t2 = await ok(call("text", { page: "bot-2" }), "run 2 text");
  check("cookie survived a restart", t2.text.includes("who=bot"), t2.text);
  await ok(call("cookies_clear"), "cookies_clear");
  await ok(call("reload", { page: "bot-2" }), "reload");
  const t3 = await ok(call("text", { page: "bot-2" }), "run 2 text after clear");
  check("cookies cleared", t3.text.includes("cookie: none"), t3.text);
  void t;
  await call("close");
  await proc.exited;
}

server.stop(true);
rmSync(root, { recursive: true, force: true });
console.log(failures ? `${failures} failed` : "all passed");
process.exit(failures ? 1 : 0);
