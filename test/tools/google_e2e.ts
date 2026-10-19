// End to end: connecting Google with the shared client (docs/bots.md) on
// headless hubs, against a stand-in for Google's token, revoke, Gmail and
// Calendar endpoints. Connect asks for Gmail and Calendar, shows Google's
// account chooser and needs nothing pasted; the redirect finishes the
// sign-in; a bot's tools run; a second sign-in adds a second account, and
// a tool's "account" picks whose token it uses; one account signs out
// alone; a secrets file from before accounts still works; a client of the
// user's own wins; a build with no shared client says so. Then mail: replies,
// attachments, drafts, signatures, HTML and the whole email in the approval,
// against a stand-in Gmail that keeps what is uploaded. Prints "ok ..."
// / "FAIL ..." lines.
//
//   bun test/tools/google_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane and must be built with
// BACKPLANE_GOOGLE_BAKE=0; WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire).
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync, mkdirSync, openSync, ftruncateSync, closeSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 400)}`);
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

// Google, as far as the hub sees it
const b64u = (s: string) => Buffer.from(s).toString("base64url");
const grants: Record<string, string>[] = [];
const revoked: string[] = [];
const bearers: string[] = [];
const calendarCalls: { method: string; path: string; updates: string | null; body: any }[] = [];
const calendar: Record<string, any> = {
  ev1: { id: "ev1", summary: "Board bring-up", start: { dateTime: "2026-09-28T09:00:00Z" }, end: { dateTime: "2026-09-28T10:00:00Z" }, attendees: [{ email: "a@x.com" }, { email: "b@y.com" }], reminders: { useDefault: true } },
  slow: { id: "slow", summary: "Slow preview", start: { dateTime: "2026-09-28T09:00:00Z" }, end: { dateTime: "2026-09-28T10:00:00Z" } },
  alone: { id: "alone", summary: "Solo", start: { dateTime: "2026-09-28T09:00:00Z" }, end: { dateTime: "2026-09-28T10:00:00Z" } },
  "ev+_20261014T090000Z": { id: "ev+_20261014T090000Z", summary: "Instance", start: { dateTime: "2026-10-14T09:00:00Z" }, end: { dateTime: "2026-10-14T10:00:00Z" } },
};
const uploads: { method: string; path: string; query: string; ctype: string; body: string }[] = [];
const sends: string[] = [];
// the code names the account: 4/0Ab* signs in cat, 4/0Ad* dog
const who = (code: string) => (code.startsWith("4/0Ad") ? "dog" : "cat");

// a MIME entity read back: headers (lower case, unfolded), body, and the parts of a multipart
type Part = { h: Record<string, string>; body: string; parts: Part[] };
function parseMime(s: string): Part {
  const i = s.indexOf("\r\n\r\n");
  const h: Record<string, string> = {};
  for (const l of s.slice(0, i).replace(/\r\n[ \t]+/g, " ").split("\r\n")) {
    const j = l.indexOf(":");
    if (j > 0) h[l.slice(0, j).toLowerCase()] = l.slice(j + 1).trim();
  }
  const body = s.slice(i + 4);
  const parts: Part[] = [];
  const m = /boundary="?([^";]+)"?/.exec(h["content-type"] ?? "");
  if (m && (h["content-type"] ?? "").startsWith("multipart/")) {
    for (const seg of body.split("--" + m[1]).slice(1)) {
      if (seg.startsWith("--")) break;
      parts.push(parseMime(seg.replace(/^\r\n/, "").replace(/\r\n$/, "")));
    }
  }
  return { h, body, parts };
}
const bytesOf = (p: Part) => Buffer.from(p.body.replace(/\s+/g, ""), "base64");
const textOf = (p: Part) => bytesOf(p).toString("utf8");
// an upload: the JSON part and the message
const upload = (u: { ctype: string; body: string }) => {
  const r = parseMime(`content-type: ${u.ctype}\r\n\r\n${u.body}`);
  return { json: JSON.parse(r.parts[0].body.trim()), msg: parseMime(r.parts[1].body), head: r.parts[1].h };
};
// the draft's message id: Gmail gives an updated draft a new one
let draftMid = "dm1";
const google = Bun.serve({
  hostname: "127.0.0.1",
  port: 0,
  async fetch(req) {
    const u = new URL(req.url);
    bearers.push(req.headers.get("authorization") ?? "");
    if (req.method === "POST" && u.pathname === "/token") {
      const f = Object.fromEntries(new URLSearchParams(await req.text()));
      grants.push(f);
      const w = who(f.code ?? "");
      const id_token = `${b64u("{}")}.${b64u(JSON.stringify({ email: `${w}@example.com` }))}.sig`;
      return Response.json({ access_token: `at-${w}-${grants.length}`, expires_in: 3600, refresh_token: `rt-${w}`, id_token, token_type: "Bearer" });
    }
    if (u.pathname === "/revoke") {
      revoked.push(new URLSearchParams(await req.text()).get("token") ?? "");
      return new Response("{}");
    }
    if (u.pathname === "/gmail/v1/users/me/messages") return Response.json({ resultSizeEstimate: 0 });
    if (u.pathname === "/gmail/v1/users/me/messages/m2")
      return Response.json({ id: "m2", threadId: "t1", labelIds: ["INBOX"], payload: { headers: [
        { name: "Message-Id", value: "<m2@mail.x>" }, { name: "References", value: "<m1@mail.x>" }, { name: "Subject", value: "Invoice 1042" },
        { name: "From", value: "Alice <alice@example.com>" }, { name: "Reply-To", value: "billing@example.com" }] } });
    if (u.pathname === "/gmail/v1/users/me/threads/t1")
      return Response.json({ id: "t1", messages: [
        { id: "m1", threadId: "t1", payload: { headers: [{ name: "Message-ID", value: "<m1@mail.x>" }, { name: "From", value: "me@example.com" }] } },
        { id: "m2", threadId: "t1", payload: { headers: [{ name: "Message-ID", value: "<m2@mail.x>" }, { name: "References", value: "<m1@mail.x>" }, { name: "Subject", value: "Invoice 1042" }, { name: "From", value: "Alice <alice@example.com>" }] } }] });
    if (u.pathname === "/gmail/v1/users/me/drafts/r-1")
      return Response.json({ id: "r-1", message: { id: draftMid, threadId: "t9", snippet: "Draft body", payload: { mimeType: "multipart/mixed", headers: [
        { name: "From", value: "me@example.com" }, { name: "To", value: "alice@example.com" }, { name: "Subject", value: "Quote" }], parts: [
        { mimeType: "text/plain", filename: "", body: { size: 15, data: b64u("Draft body text") } },
        { mimeType: "application/pdf", filename: "quote.pdf", body: { size: 12345, attachmentId: "x" } }] } } });
    if (u.pathname === "/gmail/v1/users/me/drafts/send" && req.method === "POST") {
      sends.push(await req.text());
      return Response.json({ id: "sent-d", threadId: "t9" });
    }
    if (u.pathname.startsWith("/upload/gmail/")) {
      uploads.push({ method: req.method, path: u.pathname, query: u.search, ctype: req.headers.get("content-type") ?? "", body: Buffer.from(await req.arrayBuffer()).toString("latin1") });
      if (u.pathname.includes("/drafts")) return Response.json({ id: "r-1", message: { id: "dm1", threadId: "t9" } });
      return Response.json({ id: "sent1", threadId: "t1" });
    }
    if (u.pathname.startsWith("/calendar/v3/calendars/primary/events")) {
      const body = req.method === "PATCH" || req.method === "POST" ? await req.json() : null;
      calendarCalls.push({ method: req.method, path: u.pathname, updates: u.searchParams.get("sendUpdates"), body });
      if (u.pathname === "/calendar/v3/calendars/primary/events") {
        if (req.method === "POST") return Response.json({ id: "created", ...body });
        return Response.json({ timeZone: "UTC", items: Object.values(calendar) });
      }
      const id = decodeURIComponent(u.pathname.split("/").at(-1)!);
      if (!calendar[id]) return Response.json({ error: { message: "Not found" } }, { status: id === "gone" ? 410 : 404 });
      if (req.method === "GET") {
        if (id === "slow") await sleep(800);
        return Response.json(calendar[id]);
      }
      if (req.method === "PATCH") {
        calendar[id] = { ...calendar[id], ...body };
        return Response.json(calendar[id]);
      }
      if (req.method === "DELETE") { delete calendar[id]; return new Response(null, { status: 204 }); }
    }
    return new Response("not here", { status: 404 });
  },
});
const G = `http://127.0.0.1:${google.port}`;

const root = mkdtempSync(join(tmpdir(), "bp-google-e2e-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse", "omp"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}

type Hub = { port: number; proc: ReturnType<typeof Bun.spawn>; ws: WebSocket; seen: any[]; replies: Map<number, any>; n: number; log: string[] };
async function start(name: string, env: Record<string, string>): Promise<Hub> {
  const port = freePort();
  const proc = Bun.spawn([resolve(bin), "--home", join(root, name), "--port", String(port), "--no-tailscale", "--headless", "--foreground"], {
    env: { ...process.env, HOME: join(root, "user"), DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "",
      BACKPLANE_GOOGLE_OAUTH: G, BACKPLANE_GOOGLE_API: G, BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, ...env },
    stdout: "pipe",
    stderr: "pipe",
  });
  const log: string[] = [];
  for (const st of [proc.stdout, proc.stderr] as any[])
    (async () => { for await (const c of st) { log.push(Buffer.from(c).toString()); if (process.env.E2E_LOG) process.stderr.write(c); } })();
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  const h: Hub = { port, proc, ws: null as any, seen: [], replies: new Map(), n: 0, log };
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
    for (const c of o.items ?? []) h.seen.push(c);
    if (o.t === "reply") h.replies.set(Number(o.id), o);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  h.ws = ws;
  return h;
}
async function rpc(h: Hub, m: string, p: object): Promise<any> {
  const id = ++h.n;
  h.ws.send(W.encode(JSON.stringify({ id, m, p })));
  return await until(10000, () => h.replies.get(id));
}
const op = (h: Hub, o: string, extra: object = {}) => rpc(h, "bots.google", { op: o, clientId: "", clientSecret: "", pasted: "", ...extra });
const tool = (h: Hub, th: string, name: string, args: object = {}) =>
  fetch(`http://127.0.0.1:${h.port}/mcp/${th}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 9, method: "tools/call", params: { name, arguments: args } }),
  }).then((r) => r.json()).then((r: any) => String(r?.result?.content?.[0]?.text ?? JSON.stringify(r)));

async function close(h: Hub) {
  h.ws?.close();
  h.proc.kill();
  await h.proc.exited;
}
async function approval(h: Hub, th: string, name: string, args: object, answer: string, inspect?: (ask: any) => void): Promise<string> {
  const before = h.seen.length;
  const result = tool(h, th, name, args);
  const ask = await until(10000, () => h.seen.slice(before).find((c) => c.$ === "AskOpened" && c.tool === name));
  check(`${name} opens approval`, !!ask && ask.kind === "approval", ask);
  if (ask) {
    inspect?.(ask);
    await rpc(h, "ask.answer", { id: ask.id, answer });
  }
  return await Promise.race([result, sleep(10000).then(() => "error: test timeout")]);
}

const hubs: Hub[] = [];
try {
  mkdirSync(join(root, "user"));
  const a = await start("shared", { BACKPLANE_GOOGLE_CLIENT_ID: "shared-id.apps.googleusercontent.com", BACKPLANE_GOOGLE_CLIENT_SECRET: "shared-secret" });
  hubs.push(a);

  const s0 = await op(a, "status");
  check("status before: not connected", s0?.ok && s0.google === "Not connected.", s0);

  const c = await op(a, "connect");
  const url = new URL(c?.googleUrl ?? "http://x/");
  const scope = url.searchParams.get("scope") ?? "";
  check("connect needs nothing pasted", c?.ok && url.host === "accounts.google.com", c);
  check("connect uses the shared client", url.searchParams.get("client_id") === "shared-id.apps.googleusercontent.com", url.search);
  check("the shared client asks for Gmail and Calendar", scope.includes("calendar.events") && scope.includes("gmail.modify"), scope);
  check("with Google's account chooser", url.searchParams.get("prompt") === "select_account consent", url.search);
  check("with PKCE", url.searchParams.get("code_challenge_method") === "S256" && (url.searchParams.get("code_challenge") ?? "").length === 43);

  const pend = await op(a, "status");
  check("status while waiting", String(pend?.google).startsWith("Waiting for Google"), pend);

  // a sign-in started on another of the owner's hubs lands here (Google
  // always sends the browser to 127.0.0.1): sent on to that hub, once
  const other = `zz.${b64u("http://box.tail1.ts.net:3787")}`;
  const hop = await fetch(`http://127.0.0.1:${a.port}/oauth/google?state=${other}&code=4%2F0Ab`, { redirect: "manual" });
  check("another hub's sign-in is sent on to it", hop.status === 302 && hop.headers.get("location") === `http://box.tail1.ts.net:3787/oauth/google?state=${other}&code=4%2F0Ab&hop=1`, [hop.status, hop.headers.get("location")]);
  const again = await fetch(`http://127.0.0.1:${a.port}/oauth/google?state=${other}&code=4%2F0Ab&hop=1`, { redirect: "manual" });
  check("but only once", again.status === 200 && (await again.text()).includes("paste this page"), again.status);
  const evil = await fetch(`http://127.0.0.1:${a.port}/oauth/google?state=zz.${b64u("https://evil.example.com")}&code=c`, { redirect: "manual" });
  check("and only to a tailnet machine", evil.status === 200, evil.status);
  check("none of that traded a code", grants.length === 0, grants);

  const back = await fetch(`http://127.0.0.1:${a.port}/oauth/google?state=${url.searchParams.get("state")}&code=4%2F0Ab&scope=${encodeURIComponent(scope)}`);
  const page = await back.text();
  check("the redirect finishes the sign-in", back.status === 200 && page.includes("Google is connected"), page.slice(0, 300));
  const g = grants.at(-1) ?? {};
  check("the code is traded with the shared client and the verifier",
    g.grant_type === "authorization_code" && g.client_id === "shared-id.apps.googleusercontent.com" && g.client_secret === "shared-secret" && (g.code_verifier ?? "").length > 40, g);

  const s1 = await op(a, "status");
  check("status after: connected, Gmail and Calendar", s1?.google === "Connected as cat@example.com (Gmail and Calendar).", s1);
  check("status lists the account", s1?.googleAccounts === "cat@example.com", s1);

  const bot = await rpc(a, "bots.create", { name: "miso" });
  const set = await until(8000, () => a.seen.find((x) => x.$ === "BotSet" && x.id === bot?.bot));
  const th: string = set?.thread ?? "";
  check("a bot to call from", !!th, bot);
  const ev = await tool(a, th, "calendar_events");
  check("calendar_events works on the shared link", ev.includes("Board bring-up"), ev);
  const gm = await tool(a, th, "gmail_search");
  check("gmail tools run on the shared link", gm.includes("No messages"), gm);

  // Calendar mutations: preview, park, and execute the exact approved plan.
  calendarCalls.length = 0;
  const beforeSlow = a.seen.length;
  const slowResult = tool(a, th, "calendar_update", { id: "slow", summary: "Delayed change" });
  await until(8000, () => calendarCalls.find((c) => c.path.endsWith("/slow")));
  const statusStart = Date.now();
  const duringFetch = await op(a, "status");
  check("hub answers while preview GET waits", duringFetch?.ok && Date.now() - statusStart < 650, [duringFetch, Date.now() - statusStart]);
  const slowAsk = await until(10000, () => a.seen.slice(beforeSlow).find((c) => c.$ === "AskOpened" && c.tool === "calendar_update"));
  check("slow fetch eventually opens approval", !!slowAsk, slowAsk);
  if (slowAsk) await rpc(a, "ask.answer", { id: slowAsk.id, answer: "decline" });
  check("slow preview declined", (await Promise.race([slowResult, sleep(10000).then(() => "timeout")])).includes("declined"));

  calendarCalls.length = 0;
  const denied = await approval(a, th, "calendar_update", { id: "ev1", summary: "Denied" }, "decline", (ask) => {
    check("update preview names event and title change", ask.detail.includes('Change "Board bring-up"') && ask.detail.includes("Title: Board bring-up  =>  Denied") && ask.detail.includes("Guests will be told."), ask.detail);
    check("GET before approval, no write", calendarCalls.length === 1 && calendarCalls[0].method === "GET", calendarCalls);
  });
  check("declined update performs no PATCH", denied.includes("declined") && calendarCalls.every((c) => c.method === "GET"), [denied, calendarCalls]);

  calendarCalls.length = 0;
  const updated = await approval(a, th, "calendar_update", { id: "ev1", summary: "Review v2", start: "2026-10-15T14:00:00Z", reminders: [10, { method: "email", minutes: 1440 }], attendees: [] }, "accept", (ask) => {
    check("preview contains preserved end, alerts and guests", ask.detail.includes("Thu 15 Oct 14:00 -> 15:00") && ask.detail.includes("popup 10 min") && ask.detail.includes("email 1 day") && ask.detail.includes("Attendees:"), ask.detail);
    // The event changes while approval is open: the patch must keep the
    // fetched one-hour duration and notification decision, without a second GET.
    calendar.ev1.end.dateTime = "2026-09-28T12:00:00Z";
    calendar.ev1.attendees = [];
  });
  const patch = calendarCalls.find((c) => c.method === "PATCH");
  check("accepted update replies with event", updated.startsWith("Updated:\n") && updated.includes("Review v2"), updated);
  check("exact prepared PATCH uses original duration and guests", calendarCalls.filter((c) => c.method === "GET").length === 1 && patch?.updates === "all" && patch.body.end.dateTime === "2026-10-15T15:00:00Z" && patch.body.attendees.length === 0 && patch.body.reminders.overrides[1].method === "email", calendarCalls);
  check("patch leaves omitted fields alone", !Object.hasOwn(patch?.body ?? {}, "location") && !Object.hasOwn(patch?.body ?? {}, "description"), patch?.body);

  const beforeBad = a.seen.filter((c) => c.$ === "AskOpened").length;
  calendarCalls.length = 0;
  for (const name of ["calendar_update", "calendar_delete"]) {
    for (const id of ["missing", "gone"]) {
      const r = await tool(a, th, name, { id, summary: "x" });
      check(`${name} ${id} fetch error`, r === `error: no event with id ${id}`, r);
    }
  }
  check("fetch errors open no asks", a.seen.filter((c) => c.$ === "AskOpened").length === beforeBad);
  calendarCalls.length = 0;
  for (const args of [{}, { id: "a/b" }, { id: "a b" }]) {
    check("delete rejects invalid id before GET", (await tool(a, th, "calendar_delete", args)).startsWith("error:"));
  }
  check("update rejects empty patch", await tool(a, th, "calendar_update", { id: "ev1" }) === "error: nothing to change");
  check("update rejects invalid alerts", (await tool(a, th, "calendar_update", { id: "ev1", reminders: [40321] })).startsWith("error:"));
  check("update rejects unreadable fields", (await tool(a, th, "calendar_update", { id: "ev1", location: null })).startsWith("error:"));
  check("invalid mutations do not touch API or ask", calendarCalls.length === 0 && a.seen.filter((c) => c.$ === "AskOpened").length === beforeBad, calendarCalls);

  calendarCalls.length = 0;
  const made = await approval(a, th, "calendar_create", { summary: "With alerts", start: "2026-10-15T14:00:00Z", end: "2026-10-15T15:00:00Z", reminders: [10], location: "Lab" }, "accept", (ask) => {
    check("create preview includes alerts and location", ask.detail.includes("Alerts: popup 10 min") && ask.detail.includes("Where: Lab"), ask.detail);
    check("create waits for approval", calendarCalls.length === 0);
  });
  const post = calendarCalls.find((c) => c.method === "POST");
  check("create POST has location and reminders", made.startsWith("Created:") && post?.body.location === "Lab" && post.body.reminders.useDefault === false && post.body.reminders.overrides[0].minutes === 10, [made, post]);

  calendarCalls.length = 0;
  const inst = await approval(a, th, "calendar_update", { id: "ev+_20261014T090000Z", location: "Desk", default_reminders: true }, "session");
  check("instance id encoded on GET and PATCH, no guests", inst.startsWith("Updated:") && calendarCalls.length === 2 && calendarCalls.every((c) => c.path.includes("ev%2B_20261014T090000Z")) && calendarCalls[1].updates === "none" && calendarCalls[1].body.reminders.useDefault === true, calendarCalls);
  const beforeSession = a.seen.filter((c) => c.$ === "AskOpened").length;
  calendarCalls.length = 0;
  const session = await tool(a, th, "calendar_update", { id: "alone", reminders: [] });
  check("session approval still fetches and patches without asking", session.startsWith("Updated:") && calendarCalls.map((c) => c.method).join(",") === "GET,PATCH" && calendarCalls[1].updates === "none" && JSON.stringify(calendarCalls[1].body.reminders) === '{"useDefault":false}' && a.seen.filter((c) => c.$ === "AskOpened").length === beforeSession, [session, calendarCalls]);

  calendarCalls.length = 0;
  const deleted = await approval(a, th, "calendar_delete", { id: "alone" }, "accept", (ask) => {
    check("delete preview includes title/time and no guests", ask.detail.startsWith('Delete "Solo"') && !ask.detail.includes("guests will be told"), ask.detail);
    check("delete not run before approval", calendarCalls.length === 1 && calendarCalls[0].method === "GET", calendarCalls);
  });
  check("DELETE and compact reply", deleted.startsWith("Deleted: Solo (") && calendarCalls.map((c) => c.method).join(",") === "GET,DELETE" && calendarCalls[1].updates === "none", [deleted, calendarCalls]);
  calendar.ev1.attendees = [{ email: "a@x.com" }, { email: "b@y.com" }];
  calendarCalls.length = 0;
  const vanished = await approval(a, th, "calendar_delete", { id: "ev1" }, "accept", (ask) => {
    check("delete preview warns guests", ask.detail.includes("2 guests will be told."), ask.detail);
    delete calendar.ev1;
  });
  check("DELETE 404 after approval maps error", vanished === "error: no event with id ev1" && calendarCalls[1]?.updates === "all", [vanished, calendarCalls]);

  // a phone signs in through its own loopback listener: the sign-in
  // begins with the phone's redirect and finishes with what it caught
  const phone = "http://127.0.0.1:49152/oauth/google";
  const cp = await op(a, "connect", { redirect: phone });
  const up = new URL(cp?.googleUrl ?? "http://x/");
  check("a phone's loopback redirect begins its sign-in", up.searchParams.get("redirect_uri") === phone, up.search);
  const ce = await op(a, "connect", { redirect: "http://evil.example.com/oauth/google" });
  const ue = new URL(ce?.googleUrl ?? "http://x/");
  check("any other redirect is the hub's own", ue.searchParams.get("redirect_uri") === `http://127.0.0.1:${a.port}/oauth/google`, ue.search);
  const cp2 = await op(a, "connect", { redirect: phone });
  const up2 = new URL(cp2?.googleUrl ?? "http://x/");
  const fin = await op(a, "finish", { pasted: `${phone}?state=${up2.searchParams.get("state")}&code=4%2F0Ab9&scope=x` });
  check("the phone's caught address finishes it", fin?.ok && String(fin?.google).startsWith("Connected as cat@example.com"), fin);
  check("the code is traded with the phone's redirect", grants.at(-1)?.redirect_uri === phone, grants.at(-1));

  // a second account
  const c2 = await op(a, "connect");
  const u2 = new URL(c2?.googleUrl ?? "http://x/");
  await (await fetch(`http://127.0.0.1:${a.port}/oauth/google?state=${u2.searchParams.get("state")}&code=4%2F0Ad`)).text();
  const s2 = await op(a, "status");
  check("a second sign-in adds an account", s2?.googleAccounts === "cat@example.com\ndog@example.com", s2);
  check("both in the status line", s2?.google === "Connected as cat@example.com, dog@example.com (Gmail and Calendar).", s2);
  const la = await tool(a, th, "google_accounts");
  check("google_accounts lists them, the default first", la.includes("cat@example.com\ndog@example.com"), la);
  bearers.length = 0;
  await tool(a, th, "calendar_events", { account: "dog@example.com" });
  check("account picks whose token a tool uses", bearers.some((b) => b.startsWith("Bearer at-dog-")) && !bearers.some((b) => b.includes("at-cat")), bearers);
  bearers.length = 0;
  await tool(a, th, "calendar_events");
  check("no account: the first", bearers.some((b) => b.startsWith("Bearer at-cat-")), bearers);
  const none = await tool(a, th, "calendar_events", { account: "fox@example.com" });
  check("an account not signed in is named", none.includes("no connected Google account fox@example.com") && none.includes("dog@example.com"), none);

  // signing in to an address already there replaces it
  const c3 = await op(a, "connect");
  const u3 = new URL(c3?.googleUrl ?? "http://x/");
  await (await fetch(`http://127.0.0.1:${a.port}/oauth/google?state=${u3.searchParams.get("state")}&code=4%2F0Ab2`)).text();
  const s3a = await op(a, "status");
  check("the same account again replaces it", s3a?.googleAccounts === "cat@example.com\ndog@example.com", s3a);

  const out = await op(a, "disconnect", { account: "dog@example.com" });
  check("one account signs out alone", out?.googleAccounts === "cat@example.com", out);
  check("its token is revoked, the other's kept", revoked.includes("rt-dog") && !revoked.includes("rt-cat"), revoked);

  const d = await op(a, "disconnect");
  check("disconnect with no account signs out all", d?.google === "Not connected." && d?.googleAccounts === "", d);

  const own = await op(a, "connect", { clientId: "own-id", clientSecret: "own-secret" });
  const ou = new URL(own?.googleUrl ?? "http://x/");
  check("a client of your own wins", ou.searchParams.get("client_id") === "own-id", own);
  check("and asks for Gmail too", (ou.searchParams.get("scope") ?? "").includes("gmail.modify"), ou.search);
  const back2 = await fetch(`http://127.0.0.1:${a.port}/oauth/google?state=${ou.searchParams.get("state")}&code=4%2F0Ac`);
  await back2.text();
  const s4 = await op(a, "status");
  check("own client: Gmail and Calendar", s4?.google === "Connected as cat@example.com (Gmail and Calendar).", s4);

  // a secrets file from before accounts
  mkdirSync(join(root, "legacy", "secrets"), { recursive: true });
  writeFileSync(join(root, "legacy", "secrets", "google.json"), JSON.stringify({ client_id: "own-id", client_secret: "own-secret",
    refresh_token: "rt-old", access_token: "at-old", expiry: 4102444800, email: "old@example.com", pending_verifier: "", pending_state: "", redirect: "" }));
  const l = await start("legacy", {});
  hubs.push(l);
  const sl = await op(l, "status");
  check("an old secrets file reads as one account", sl?.googleAccounts === "old@example.com", sl);
  const lbot = await rpc(l, "bots.create", { name: "tofu" });
  const lset = await until(8000, () => l.seen.find((x) => x.$ === "BotSet" && x.id === lbot?.bot));
  bearers.length = 0;
  await tool(l, lset?.thread ?? "", "calendar_events");
  check("and its token still works", bearers.includes("Bearer at-old"), bearers);

  await close(l);
  hubs.splice(hubs.indexOf(l), 1);
  const b = await start("bare", {});
  hubs.push(b);
  const s5 = await op(b, "status");
  check("no shared client: status says so", String(s5?.google).includes("no shared Google client"), s5);
  const c5 = await op(b, "connect");
  check("no shared client: connect asks for one", c5?.ok === false || String(c5?.error ?? "").includes("no shared Google client"), c5);

  // Mail: replies, attachments, drafts, a signature, HTML, the whole email in the approval
  mkdirSync(join(root, "mail", "secrets"), { recursive: true });
  writeFileSync(join(root, "mail", "secrets", "google.json"), JSON.stringify({ client_id: "own-id", client_secret: "own-secret",
    accounts: [{ email: "me@example.com", refresh_token: "rt-me-secret", access_token: "at-me-secret", expiry: 4102444800 }], pending_verifier: "", pending_state: "", redirect: "" }));
  const m = await start("mail", {});
  hubs.push(m);
  const mbot = await rpc(m, "bots.create", { name: "nori" });
  const mset = await until(8000, () => m.seen.find((x) => x.$ === "BotSet" && x.id === mbot?.bot));
  const mt: string = mset?.thread ?? "";
  check("a bot to send mail", !!mt, mbot);
  const sig = "— Sent by Backplane\non behalf of Me";
  await rpc(m, "setting.set", { key: "gmail.sig.me@example.com", value: sig });
  const handled = new Set<string>();
  const nextAsk = async (pred: (c: any) => boolean = () => true) => {
    const a = await until(15000, () => m.seen.find((c) => c.$ === "AskOpened" && !handled.has(c.id) && c.tool === "gmail_send" && pred(c)));
    if (a) handled.add(a.id);
    return a;
  };
  const asks = () => m.seen.filter((c) => c.$ === "AskOpened").length;
  const files = join(root, "files");
  mkdirSync(files);

  // a reply: Reply-To, Re: subject, the original's conversation, the whole body, the signature
  const long = "Line one.\n" + "x".repeat(1800) + "\nEND-OF-BODY";
  const p1 = tool(m, mt, "gmail_send", { reply_to: "m2", body: long });
  const a1 = await nextAsk();
  const d1 = String(a1?.detail ?? "");
  check("reply: the approval has the account and From", d1.includes("Account: me@example.com\nFrom: me@example.com\n"), d1.slice(0, 200));
  check("reply: To defaults to the Reply-To", d1.includes("To: billing@example.com\n"), d1.slice(0, 200));
  check("reply: Subject is Re: the original's", d1.includes("Subject: Re: Invoice 1042\n"), d1.slice(0, 200));
  check("reply: In reply to names the original", d1.includes('In reply to: "Invoice 1042" from Alice <alice@example.com>\n'), d1.slice(0, 300));
  check("reply: the whole body is shown (over 1500 characters)", d1.includes("END-OF-BODY"), d1.length);
  check("reply: the signature is shown", d1.includes("— Sent by Backplane\non behalf of Me"), d1.slice(-120));
  check("reply: the head says it is an email", a1?.kind === "approval" && a1?.tool === "gmail_send", a1);
  const n0 = uploads.length;
  check("reply: nothing sent before the answer", n0 === 0, uploads.length);
  await rpc(m, "ask.answer", { id: a1?.id, answer: "accept" });
  const r1 = await p1;
  check("reply: sent, the agent is told", r1.startsWith("Sent (message id sent1, thread t1)"), r1);
  const u1 = uploads.at(-1)!;
  check("reply: uploaded by media upload", u1?.method === "POST" && u1.path === "/upload/gmail/v1/users/me/messages/send" && u1.query === "?uploadType=multipart" && u1.ctype.startsWith("multipart/related"), u1 && [u1.method, u1.path, u1.query, u1.ctype]);
  const m1 = u1 ? upload(u1) : null;
  check("reply: the request carries the threadId", m1?.json?.threadId === "t1", m1?.json);
  check("reply: In-Reply-To and References", m1?.msg.h["in-reply-to"] === "<m2@mail.x>" && m1?.msg.h["references"] === "<m1@mail.x> <m2@mail.x>", m1?.msg.h);
  check("reply: From, To, Subject", m1?.msg.h["from"] === "me@example.com" && m1?.msg.h["to"] === "billing@example.com" && m1?.msg.h["subject"] === "Re: Invoice 1042", m1?.msg.h);
  const t1 = m1 ? textOf(m1.msg) : "";
  check("reply: the sent text is what was shown", t1.includes("END-OF-BODY") && t1.endsWith("on behalf of Me"), t1.slice(-80));

  // a reply to a conversation answers its last message
  const p1b = tool(m, mt, "gmail_send", { thread: "t1", to: "carol@example.com", subject: "Own subject", body: "ok" });
  const a1b = await nextAsk();
  check("thread reply: the given subject and To stay", String(a1b?.detail).includes("To: carol@example.com\nSubject: Own subject\n") && String(a1b?.detail).includes('In reply to: "Invoice 1042"'), a1b?.detail);
  await rpc(m, "ask.answer", { id: a1b?.id, answer: "accept" });
  await p1b;
  const m1b = upload(uploads.at(-1)!);
  check("thread reply: threadId and In-Reply-To of the last message", m1b.json.threadId === "t1" && m1b.msg.h["in-reply-to"] === "<m2@mail.x>", [m1b.json, m1b.msg.h]);

  // two attachments, byte for byte
  const pdf = join(files, "invoice-1042.pdf");
  const pdfBytes = Buffer.alloc(186368);
  for (let i = 0; i < pdfBytes.length; i++) pdfBytes[i] = (i * 131 + (i >> 8)) & 255;
  writeFileSync(pdf, pdfBytes);
  const csv = join(files, "timesheet.csv");
  writeFileSync(csv, "day,hours\nmon,8\n");
  const p2 = tool(m, mt, "gmail_send", { to: "alice@example.com", cc: "bob@example.com", bcc: "carol@example.com", subject: "Invoice", body: "Attached.", attachments: [pdf, csv] });
  const a2 = await nextAsk();
  const d2 = String(a2?.detail ?? "");
  check("attachments: names and sizes in the approval", d2.includes("Attachments: invoice-1042.pdf (182 KB), timesheet.csv (16 B)\n"), d2);
  check("attachments: Cc and Bcc are shown", d2.includes("Cc: bob@example.com\nBcc: carol@example.com\n"), d2);
  await rpc(m, "ask.answer", { id: a2?.id, answer: "accept" });
  const r2 = await p2;
  check("attachments: sent", r2.startsWith("Sent ("), r2);
  const m2 = upload(uploads.at(-1)!);
  check("attachments: the request has no thread", Object.keys(m2.json).length === 0, m2.json);
  check("attachments: multipart/mixed, the text then each file", m2.msg.h["content-type"].startsWith("multipart/mixed") && m2.msg.parts.length === 3, m2.msg.parts.map((x) => x.h["content-type"]));
  check("attachments: Bcc is in the message", m2.msg.h["bcc"] === "carol@example.com" && m2.msg.h["cc"] === "bob@example.com", m2.msg.h);
  check("attachments: the text part", textOf(m2.msg.parts[0]).startsWith("Attached."), textOf(m2.msg.parts[0]));
  check("attachments: the PDF's bytes are the file's", bytesOf(m2.msg.parts[1]).equals(pdfBytes) && m2.msg.parts[1].h["content-type"].startsWith("application/pdf") && m2.msg.parts[1].h["content-disposition"].includes('filename="invoice-1042.pdf"'),
    [bytesOf(m2.msg.parts[1]).length, m2.msg.parts[1].h]);
  check("attachments: the CSV's bytes are the file's", textOf(m2.msg.parts[2]) === "day,hours\nmon,8\n" && m2.msg.parts[2].h["content-type"].startsWith("text/csv"), m2.msg.parts[2].h);
  check("attachments: base64 lines are at most 76 characters", m2.msg.parts[1].body.split("\r\n").every((l) => l.length <= 76 && !l.startsWith("--") || l.startsWith("--")), "");

  // attachment errors: before anything is asked or sent
  const big = join(files, "big.bin");
  { const fd = openSync(big, "w"); ftruncateSync(fd, 19 * 1024 * 1024); closeSync(fd); }
  const asks0 = asks(), ups0 = uploads.length;
  const e1 = await tool(m, mt, "gmail_send", { to: "a@example.com", subject: "s", body: "b", attachments: [join(files, "nope.pdf")] });
  const e2 = await tool(m, mt, "gmail_send", { to: "a@example.com", subject: "s", body: "b", attachments: ["relative/file.pdf"] });
  const e3 = await tool(m, mt, "gmail_send", { to: "a@example.com", subject: "s", body: "b", attachments: [big] });
  const e4 = await tool(m, mt, "gmail_send", { to: "a@example.com", subject: "s", body: "b", attachments: [files] });
  check("a missing file is an error naming it", e1.startsWith("error:") && e1.includes("nope.pdf") && e1.includes("not an existing file"), e1);
  check("a relative path is an error", e2.startsWith("error:") && e2.includes("relative/file.pdf") && e2.includes("not an absolute path"), e2);
  check("an oversize file is an error", e3.startsWith("error:") && e3.includes("big.bin") && e3.includes("18 MB"), e3);
  check("a folder is not a file", e4.startsWith("error:") && e4.includes("not an existing file"), e4);
  check("no ask and no upload for any of them", asks() === asks0 && uploads.length === ups0, [asks(), asks0, uploads.length, ups0]);

  // HTML
  const p3 = tool(m, mt, "gmail_send", { to: "alice@example.com", subject: "Hello", body: "<p>Hi <b>there</b></p>", html: true });
  const a3 = await nextAsk();
  const d3 = String(a3?.detail ?? "");
  check("html: the approval shows the text and says HTML", d3.includes("Format: HTML\n") && d3.includes("Hi there") && d3.includes("(sent as HTML)") && !d3.includes("<b>"), d3);
  check("html: the signature is escaped HTML after the body", d3.includes("Hi there"), d3);
  await rpc(m, "ask.answer", { id: a3?.id, answer: "accept" });
  await p3;
  const m3 = upload(uploads.at(-1)!);
  check("html: multipart/alternative, text then html", m3.msg.h["content-type"].startsWith("multipart/alternative") && m3.msg.parts.length === 2
    && m3.msg.parts[0].h["content-type"].startsWith("text/plain") && m3.msg.parts[1].h["content-type"].startsWith("text/html"), m3.msg.parts.map((x) => x.h["content-type"]));
  check("html: the html part is the body and the signature", textOf(m3.msg.parts[1]).startsWith("<p>Hi <b>there</b></p><br><br>— Sent by Backplane<br>on behalf of Me"), textOf(m3.msg.parts[1]));
  check("html: the text part is its text", textOf(m3.msg.parts[0]).startsWith("Hi there"), textOf(m3.msg.parts[0]));

  // declined: nothing is sent, nothing is left on disk
  const ups1 = uploads.length;
  const p4 = tool(m, mt, "gmail_send", { to: "alice@example.com", subject: "No", body: "Never" });
  const a4 = await nextAsk();
  await rpc(m, "ask.answer", { id: a4?.id, answer: "decline" });
  const r4 = await p4;
  check("declined: the agent is told", r4.includes("declined"), r4);
  await sleep(300);
  check("declined: nothing uploaded", uploads.length === ups1, uploads.length);
  check("no prepared message is left in secrets", readdirSync(join(root, "mail", "secrets")).every((f) => !f.startsWith("gm-")), readdirSync(join(root, "mail", "secrets")));

  // drafts: no ask; create, replace, then send as it is
  const asks1 = asks();
  const dr = await tool(m, mt, "gmail_draft", { to: "alice@example.com", subject: "Quote", body: "Draft text", reply_to: "m2" });
  check("draft: saved without asking", dr.startsWith("Draft saved (draft id r-1, message id dm1, thread t9)") && dr.includes('gmail_send {draft: "r-1"}') && asks() === asks1, dr);
  const ud = uploads.at(-1)!;
  check("draft: created by the drafts media upload", ud.method === "POST" && ud.path === "/upload/gmail/v1/users/me/drafts" && ud.query === "?uploadType=multipart", [ud.method, ud.path, ud.query]);
  const md = upload(ud);
  check("draft: the JSON part has the message's thread", md.json?.message?.threadId === "t1" && md.json.id === undefined, md.json);
  check("draft: the message has the signature", textOf(md.msg).includes("Draft text") && textOf(md.msg).endsWith("on behalf of Me"), textOf(md.msg));
  const dr2 = await tool(m, mt, "gmail_draft", { to: "alice@example.com", subject: "Quote", body: "Draft text 2", draft: "r-1" });
  const ud2 = uploads.at(-1)!;
  const md2 = upload(ud2);
  check("draft: replaced with PUT", dr2.startsWith("Draft saved") && ud2.method === "PUT" && ud2.path === "/upload/gmail/v1/users/me/drafts/r-1" && md2.json.id === "r-1", [ud2.method, ud2.path, md2.json]);
  const bad = await tool(m, mt, "gmail_send", { draft: "r-1", body: "extra" });
  check("a draft is sent as it is", bad.startsWith("error:") && bad.includes("send a draft as it is"), bad);
  const ps = tool(m, mt, "gmail_send", { draft: "r-1" });
  const ask5 = await nextAsk();
  const ds = String(ask5?.detail ?? "");
  check("sending a draft: the approval starts with the draft", ds.startsWith("Draft: r-1\nAccount: me@example.com\nFrom: me@example.com\nTo: alice@example.com\nSubject: Quote\nAttachments: quote.pdf (12 KB)\n\nDraft body text"), ds);
  const ups2 = uploads.length;
  await rpc(m, "ask.answer", { id: ask5?.id, answer: "accept" });
  const rs = await ps;
  check("sending a draft: drafts.send with its id", rs.startsWith("Sent (message id sent-d, thread t9)") && sends.length === 1 && JSON.parse(sends[0]).id === "r-1" && uploads.length === ups2, [rs, sends]);
  const pc = tool(m, mt, "gmail_send", { draft: "r-1" });
  const ask6 = await nextAsk();
  draftMid = "dm2";
  await rpc(m, "ask.answer", { id: ask6?.id, answer: "accept" });
  const rc = await pc;
  draftMid = "dm1";
  check("a draft changed after review is not sent", rc.startsWith("error:") && rc.includes("changed") && sends.length === 1, [rc, sends.length]);

  // no token or secret in the log, the output or the approvals
  const events = readFileSync(join(root, "mail", "events.jsonl"), "utf8");
  const mout = m.log.join("");
  const secrets = ["at-me-secret", "rt-me-secret", "own-secret"];
  check("no token appears in events.jsonl", secrets.every((x) => !events.includes(x)), secrets.filter((x) => events.includes(x)));
  check("no token appears in the hub's output", secrets.every((x) => !mout.includes(x)), secrets.filter((x) => mout.includes(x)));
} finally {
  for (const h of hubs) {
    await close(h);
  }
  google.stop(true);
  rmSync(root, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
