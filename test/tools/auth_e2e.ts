// Auth boundaries on a real native hub, using an isolated home and fake secret.
// bun test/tools/auth_e2e.ts BINARY WEBDIR [PLAYWRIGHT_MODULE CHROME]
import assert from "node:assert/strict";
import { connect, createServer } from "node:net";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { networkInterfaces, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [binary, web, playwrightModule, chrome] = process.argv.slice(2);
assert(binary && web, "BINARY and WEBDIR are required");
const root = mkdtempSync(join(tmpdir(), "bp-auth-e2e-"));
const home = join(root, "hub");
const user = join(root, "user");
const agents = join(root, "standins");
for (const p of [home, user, agents]) mkdirSync(p);
const token = "a19e6d834b105f0c2e7d948ab62350cf";
writeFileSync(join(home, "token"), token, { mode: 0o600 });
const port = await new Promise<number>((resolve, reject) => {
  const s = createServer();
  s.on("error", reject);
  s.listen(0, "127.0.0.1", () => {
    const p = (s.address() as { port: number }).port;
    s.close(() => resolve(p));
  });
});
const authority = `127.0.0.1:${port}`;
const origin = `http://${authority}`;
const proc = Bun.spawn([resolve(binary), "--home", home, "--host", "0.0.0.0", "--port", String(port), "--web", resolve(web), "--no-tailscale", "--foreground"], {
  env: {
    HOME: user, PATH: "/usr/bin:/bin", DISPLAY: "", WAYLAND_DISPLAY: "",
    XDG_CACHE_HOME: join(user, ".cache"), BACKPLANE_AGENTS: agents,
    BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_PEERS: "", BACKPLANE_KEYRING_LOCKED: "0",
    BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_DEPLOY_LOCKS: join(root, "no-lock"),
  },
  stdout: Bun.file(join(root, "stdout")), stderr: Bun.file(join(root, "stderr")),
});
const check = (label: string, actual: unknown, expected: unknown) => {
  assert.deepEqual(actual, expected, label);
  console.log(`ok ${label}`);
};
type Response = { status: number; headers: string };
function request(path: string, headers: Record<string, string> = {}, method = "GET", hostname = "127.0.0.1"): Promise<Response> {
  return new Promise((resolve, reject) => {
    const s = connect({ host: hostname, port });
    const timer = setTimeout(() => s.destroy(new Error("request timed out")), 5000);
    let text = "";
    s.on("error", reject);
    s.on("close", () => clearTimeout(timer));
    s.on("connect", () => s.write(`${method} ${path} HTTP/1.1\r\nHost: ${headers.Host ?? authority}\r\n${Object.entries(headers).filter(([k]) => k.toLowerCase() !== "host").map(([k, v]) => `${k}: ${v}\r\n`).join("")}Content-Length: 0\r\nConnection: close\r\n\r\n`));
    s.on("data", data => {
      text += data.toString("latin1");
      const end = text.indexOf("\r\n\r\n");
      if (end < 0) return;
      const status = Number(text.split(" ")[1]);
      s.destroy();
      resolve({ status, headers: text.slice(0, end) });
    });
  });
}
const ws = (headers: Record<string, string> = {}, suffix = "") => request(`/ws${suffix}`, {
  Upgrade: "websocket", "Sec-WebSocket-Version": "13", "Sec-WebSocket-Key": "dGhlIHNhbXBsZSBub25jZQ==", ...headers,
});
let browser: any;
try {
  const deadline = Date.now() + 15000;
  for (;;) {
    try { if ((await request("/hello")).status === 200) break; } catch {}
    if (Date.now() > deadline) throw new Error("isolated hub did not start");
    await Bun.sleep(50);
  }
  if (process.env.AUTH_E2E_REMAINING !== "1") {
    check("native loopback socket needs no shared token", (await ws()).status, 101);
    check("same-origin local browser connects", (await ws({ Origin: origin })).status, 101);
    check("foreign browser cannot inherit loopback trust", (await ws({ Origin: "https://foreign.invalid" })).status, 401);
    check("foreign browser with valid cookie still refused", (await ws({ Origin: "https://foreign.invalid", Cookie: `bp_token=${token}` })).status, 401);
    check("null browser origin refused", (await ws({ Origin: "null" })).status, 401);
    check("cross-site metadata refused without Origin", (await ws({ "Sec-Fetch-Site": "cross-site" })).status, 401);
    const proxy = { "Tailscale-User-Login": "test@example.invalid" };
    check("proxy does not inherit local trust", (await ws(proxy)).status, 401);
    check("exact legacy native query still accepted", (await ws(proxy, `?token=${token}`)).status, 101);
    check("query suffix cannot authenticate", (await ws(proxy, `?token=${token}tail`)).status, 401);
    check("wrong query name cannot authenticate", (await ws(proxy, `?not_token=${token}`)).status, 401);
    check("exact cookie accepted", (await ws({ ...proxy, Cookie: `other=x; bp_token=${token}; next=y` })).status, 101);
    check("cookie suffix cannot authenticate", (await ws({ ...proxy, Cookie: `bp_token=${token}tail` })).status, 401);
    check("cookie name cannot authenticate", (await ws({ ...proxy, Cookie: `not_bp_token=${token}` })).status, 401);
    check("pairing requires bearer even on loopback", (await request("/auth/pair", { Origin: origin }, "POST")).status, 401);
    check("pairing requires POST", (await request("/auth/pair", { Origin: origin, Authorization: `Bearer ${token}` })).status, 401);
    check("foreign-origin pairing refused", (await request("/auth/pair", { Origin: "https://foreign.invalid", Authorization: `Bearer ${token}` }, "POST")).status, 401);
    const pair = await request("/auth/pair", { Origin: origin, Authorization: `Bearer ${token}` }, "POST");
    check("same-origin bearer pairs", pair.status, 204);
    check("pair cookie HttpOnly and Strict", /Set-Cookie: bp_token=[^;]+; Path=\/; HttpOnly; SameSite=Strict; Max-Age=31536000\r?$/im.test(pair.headers), true);
    const tlsPair = await request("/auth/pair", { Origin: `https://${authority}`, Authorization: `Bearer ${token}` }, "POST");
    check("HTTPS pairing cookie Secure", /; Secure\r?$/m.test(tlsPair.headers), true);
    const rebound = await new Promise<Response>((resolve, reject) => {
      const s = connect({ host: "127.0.0.1", port });
      let text = "";
      s.setTimeout(5000, () => s.destroy(new Error("rebind test timed out")));
      s.on("error", reject);
      s.on("connect", () => s.write("GET /debug/perf HTTP/1.1\r\nHost: foreign.invalid\r\nConnection: close\r\n\r\n"));
      s.on("data", d => { text += d.toString(); if (text.includes("\r\n\r\n")) { s.destroy(); resolve({ status: Number(text.split(" ")[1]), headers: text }); } });
    });
    check("rebound hostname GET without Origin refused", rebound.status, 401);
  }
  check("rebound hostname cannot reach local build locks", (await request("/lock/status", { Host: "foreign.invalid" }, "POST")).status, 403);
  check("proxy cannot inherit local build-lock access", (await request("/lock/status", { "Tailscale-User-Login": "test@example.invalid" }, "POST")).status, 403);
  const lan = Object.values(networkInterfaces()).flat().find(a => a?.family === "IPv4" && !a.internal)?.address;
  assert(lan, "a non-loopback local interface is required for wildcard listener regression");
  check("wildcard listener does not trust nonloopback peer", (await request("/debug/perf", {}, "GET", lan)).status, 401);
  if (playwrightModule) {
    const { chromium } = await import(resolve(playwrightModule));
    browser = await chromium.launch({ headless: true, executablePath: chrome || undefined, args: ["--no-sandbox"] });
    const context = await browser.newContext();
    const page = await context.newPage();
    const requests: string[] = [];
    page.on("request", (r: any) => requests.push(r.url()));
    await page.goto(`${origin}/#token=${token}`);
    await page.waitForFunction(() => !location.hash && !!document.querySelector("#root")?.childElementCount);
    await page.waitForFunction(() => localStorage.getItem("backplane-token") === null);
    const cookies = await context.cookies(origin);
    check("real browser holds paired HttpOnly cookie", cookies.some((c: any) => c.name === "bp_token" && c.httpOnly && c.sameSite === "Strict"), true);
    check("document cannot read pairing cookie", await page.evaluate(() => document.cookie.includes("bp_token=")), false);
    check("browser transport URLs do not carry secret", requests.every(u => !u.includes(token)), true);
    await context.close();
    const legacy = await browser.newContext();
    await legacy.addInitScript((t: string) => localStorage.setItem("backplane-token", t), token);
    const legacyPage = await legacy.newPage();
    await legacyPage.goto(origin);
    await legacyPage.waitForFunction(() => localStorage.getItem("backplane-token") === null);
    check("real browser migrates legacy secret to HttpOnly cookie", (await legacy.cookies(origin)).some((c: any) => c.name === "bp_token" && c.httpOnly), true);
    await legacy.close();
  }
  for (const name of ["stdout", "stderr"]) check(`${name} excludes pairing secret`, readFileSync(join(root, name), "utf8").includes(token), false);
} finally {
  await browser?.close();
  proc.kill("SIGTERM");
  await proc.exited;
  rmSync(root, { recursive: true, force: true });
}
