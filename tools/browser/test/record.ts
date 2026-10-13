// The in-page recorder, end to end: bun test/record.ts
// Uses dist/backplane-browser when BACKPLANE_BROWSER is set, else src/main.ts.
// Serves a fixture on 127.0.0.1, turns record on for owner "bot-t", drives
// it with the helper's own ops and checks the record events: semantic
// targets, typed values, secrets masked (never on stdout), navigation, and
// silence after record off and for another owner.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = mkdtempSync(join(tmpdir(), "bp-record-"));
const PW = "hunter2-SECRET-pw";
const CARD = "4111111111111111";

const server = Bun.serve({
  port: 0,
  hostname: "127.0.0.1",
  fetch(req) {
    const u = new URL(req.url);
    const html = (b: string) => new Response(`<!doctype html><title>${u.pathname}</title><body>${b}</body>`, { headers: { "content-type": "text/html" } });
    if (u.pathname === "/two") return html(`<h1>Second</h1><a href="/">Home</a>`);
    if (u.pathname === "/done") return html(`<h1>Done</h1><a id=l2 href="/two">Second page</a>`);
    return html(`
      <h1>Fixture</h1>
      <a href="/two">Second page</a>
      <form action="/done" method="post">
        <label for=q>Search</label><input id=q name=q>
        <label for=pw>Password</label><input id=pw type=password name=pw>
        <label for=cn>Card number</label><input id=cn name=cn autocomplete=cc-number>
        <label for=color>Color</label>
        <select id=color name=color><option value=r>Red</option><option value=g>Green</option></select>
        <button type=button id=btn>Do thing</button>
        <button type=submit id=go>Go</button>
      </form>`);
  },
});
const base = `http://127.0.0.1:${server.port}`;

const flags = ["--frames", join(root, "f.rgb"), "--profile", join(root, "profile"), "--jpeg-dir", join(root, "shots")];
const cmd = process.env.BACKPLANE_BROWSER
  ? [process.env.BACKPLANE_BROWSER, ...flags]
  : ["bun", new URL("../src/main.ts", import.meta.url).pathname, ...flags];
const proc = Bun.spawn(cmd, { stdin: "pipe", stdout: "pipe", stderr: "inherit" });
const waiting = new Map<number, (v: any) => void>();
const events: any[] = [];
let raw = "";
let nextId = 1;
(async () => {
  const dec = new TextDecoder();
  let buf = "";
  for await (const chunk of proc.stdout) {
    const text = dec.decode(chunk);
    raw += text;
    buf += text;
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

let failures = 0;
function check(what: string, ok: boolean, detail: unknown = "") {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}${ok ? "" : " " + JSON.stringify(detail).slice(0, 600)}`);
  if (!ok) failures++;
}
async function ok(p: Promise<any>, what: string) {
  const r = await p;
  check(what, r.ok === true, r);
  return r.result;
}
const steps = (page = "bot-t") => events.filter((e) => e.event === "record" && e.page === page).map((e) => e.step);
async function settle() { await Bun.sleep(400); }
const T = "bot-t";

try {
  await ok(call("open", { page: T, url: base + "/" }), "bot-t open");
  await ok(call("open", { page: "bot-u", url: base + "/" }), "bot-u open");
  // before recording: nothing
  await ok(call("click", { page: T, selector: "#btn" }), "click before record");
  await settle();
  check("no steps before record", steps().length === 0, steps());

  const r = await ok(call("record", { page: T, on: true }), "record on");
  check("record result", r?.on === true && r?.page === T, r);

  await ok(call("click", { page: T, selector: "#btn" }), "click Do thing");
  await ok(call("type", { page: T, selector: "#q", text: "bend lang" }), "type search");
  await ok(call("type", { page: T, selector: "#pw", text: PW }), "type password");
  await ok(call("type", { page: T, selector: "#cn", text: CARD }), "type card");
  await ok(call("select", { page: T, selector: "#color", value: "g" }), "select colour");
  await ok(call("press", { page: T, key: "Tab" }), "press Tab");
  // another owner acts while bot-t records
  await ok(call("click", { page: "bot-u", selector: "#btn" }), "bot-u click");
  await ok(call("type", { page: "bot-u", selector: "#q", text: "other" }), "bot-u type");
  await ok(call("press", { page: "bot-u", key: "Tab" }), "bot-u press");
  await ok(call("click", { page: T, selector: "#go" }), "click Go");
  await ok(call("wait_for", { page: T, url: "/done" }), "reached /done");
  await settle();

  let s = steps();
  const click = s.find((x) => x.do === "click" && x.target.name === "Do thing");
  check("click step: role button, name", click?.target.role === "button" && click.target.tag === "button" && click.target.near === "Fixture", click);
  check("click step keys", click && ["do", "target", "url", "at", "frame"].every((k) => k in click) && click.frame === "" && click.url === base + "/", click);
  check("target keys", click && ["role", "name", "label", "placeholder", "text", "testid", "css", "nth", "near", "tag", "type"].every((k) => k in click.target) && typeof click.target.nth === "number", click?.target);
  check("css prefers #id", click?.target.css === "#btn", click?.target.css);

  const typed = s.find((x) => x.do === "type" && x.target.css === "#q");
  check("type step: value present", typed?.value === "bend lang" && typed.target.role === "textbox" && typed.target.label === "Search", typed);
  check("one type step for the search field", s.filter((x) => x.do === "type" && x.target.css === "#q").length === 1, s);

  const pw = s.find((x) => x.do === "type" && x.target.css === "#pw");
  check("password: secret, label, no value", pw?.secret === true && pw.label === "Password" && !("value" in pw), pw);
  const cn = s.find((x) => x.do === "type" && x.target.css === "#cn");
  check("card: secret, no value", cn?.secret === true && cn.label === "Card number" && !("value" in cn), cn);

  const sel = s.find((x) => x.do === "select");
  check("select step", sel?.value === "g" && sel.text === "Green" && sel.target.role === "combobox", sel);
  const tab = s.find((x) => x.do === "press" && x.key === "Tab");
  check("press Tab step", !!tab && tab.target.css === "#cn", tab);
  check("submit step", s.some((x) => x.do === "submit" && x.target.tag === "form"), s.map((x) => x.do));
  check("navigate to /done", s.some((x) => x.do === "navigate" && x.url === base + "/done"), s.map((x) => x.url));
  const navs = s.filter((x) => x.do === "navigate");
  check("navigate deduped", new Set(navs.map((n) => n.url)).size === navs.length, navs);
  check("every step has at", s.every((x) => typeof x.at === "number"), s);

  // a link click, then a navigate
  await ok(call("click", { page: T, selector: "#l2" }), "click link");
  await ok(call("wait_for", { page: T, url: "/two" }), "reached /two");
  await settle();
  s = steps();
  const link = s.find((x) => x.do === "click" && x.target.role === "link");
  check("link click step", link?.target.name === "Second page" && link.target.css === "#l2", link);
  check("navigate to /two", s.some((x) => x.do === "navigate" && x.url === base + "/two"), s.map((x) => x.url));

  check("another owner emitted nothing", steps("bot-u").length === 0 && events.every((e) => e.event !== "record" || e.page === T), steps("bot-u"));

  // off: no more steps, even from clicks
  const off = await ok(call("record", { page: T, on: false }), "record off");
  check("record off result", off?.on === false, off);
  const n = steps().length;
  await ok(call("click", { page: T, selector: "a" }), "click after off");
  await ok(call("wait_for", { page: T, url: base + "/" }), "back home");
  await ok(call("click", { page: T, text: "x", selector: "body" }), "click body after off");
  await settle();
  check("no events after record off", steps().length === n, steps().slice(n));

  check("secrets never on stdout", !raw.includes(PW) && !raw.includes(CARD), "leaked");
} catch (e) {
  console.log("FAIL exception", e);
  failures++;
} finally {
  proc.kill();
  server.stop(true);
  rmSync(root, { recursive: true, force: true });
}
console.log(failures ? `${failures} FAILED` : "all passed");
process.exit(failures ? 1 : 0);
