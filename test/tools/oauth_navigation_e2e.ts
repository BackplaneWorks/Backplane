// Isolated real-native-hub OAuth navigation regression; synthetic credentials only.
// bun test/tools/oauth_navigation_e2e.ts OWN_BINARY RECEIPT.json
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, chmodSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createHash } from "node:crypto";

const [binaryArg, receiptArg] = process.argv.slice(2);
if (!binaryArg || !receiptArg) throw new Error("OWN_BINARY and RECEIPT.json required");
const binary = resolve(binaryArg), receipt = resolve(receiptArg);
const root = mkdtempSync(join(tmpdir(), "bp-oauth-navigation-"));
chmodSync(root, 0o700);
const home = join(root, "hub"), user = join(root, "user"), agents = join(root, "standins");
for (const p of [home, user, agents, join(home, "secrets")]) mkdirSync(p, { recursive: true, mode: 0o700 });
const reserve = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const port = reserve.port;
reserve.stop(true);
const base = `http://127.0.0.1:${port}`;
const state = "synthetic-release-state-329c9b42";
const secretFile = join(home, "secrets/google.json");
const seed = () => writeFileSync(secretFile, JSON.stringify({
  client_id: "synthetic.apps.googleusercontent.com", client_secret: "synthetic-client-secret",
  pending_verifier: "synthetic-pkce-verifier-0123456789012345678901234567890123456789",
  pending_state: state, redirect: `${base}/oauth/google`, accounts: [],
}), { mode: 0o600 });
seed();
writeFileSync(join(home, "token"), "36fa8ebdac7266cf30cd5f462c07805e", { mode: 0o600 });
let grants = 0;
const stub = Bun.serve({ hostname: "127.0.0.1", port: 0, async fetch(req) {
  const u = new URL(req.url);
  if (u.pathname === "/token" && req.method === "POST") {
    const data = new URLSearchParams(await req.text());
    if (data.get("code") !== "synthetic-code" || data.get("grant_type") !== "authorization_code")
      return Response.json({ error: "invalid_grant" }, { status: 400 });
    grants++;
    const body = Buffer.from(JSON.stringify({ email: "fixture@example.invalid" })).toString("base64url");
    return Response.json({ access_token: "synthetic-access", refresh_token: "synthetic-refresh", expires_in: 3600,
      id_token: `e30.${body}.synthetic`, token_type: "Bearer" });
  }
  return Response.json({ resultSizeEstimate: 0, emailAddress: "fixture@example.invalid" });
} });
const stubBase = `http://127.0.0.1:${stub.port}`;
const proc = Bun.spawn([binary, "--threads", "2", "--home", home, "--host", "127.0.0.1", "--port", String(port),
  "--no-tailscale", "--headless", "--foreground"], {
  env: { HOME: user, PATH: "/usr/bin:/bin", DISPLAY: "", WAYLAND_DISPLAY: "",
    BACKPLANE_AGENTS: agents, BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_PEERS: "", BACKPLANE_KEYRING_LOCKED: "0",
    BACKPLANE_GOOGLE_OAUTH: stubBase, BACKPLANE_GOOGLE_API: stubBase,
    BACKPLANE_TELEMETRY: "off", DO_NOT_TRACK: "1", BEND_NO_TELEMETRY: "1" },
  stdout: Bun.file(join(root, "stdout")), stderr: Bun.file(join(root, "stderr")),
});
const checks: { name: string; pass: boolean; status?: number }[] = [];
const probeDeadline = Date.now() + 45000;
const check = (name: string, pass: boolean, status?: number) => {
  checks.push({ name, pass, ...(status === undefined ? {} : { status }) });
  console.log(`${pass ? "PASS" : "FAIL"} ${name}${status === undefined ? "" : ` status=${status}`}`);
};
const nav = { "Sec-Fetch-Site": "cross-site", "Sec-Fetch-Mode": "navigate", "Sec-Fetch-Dest": "document", "Sec-Fetch-User": "?1" };
const callback = (s: string, headers: Record<string, string>, method = "GET") =>
  fetch(`${base}/oauth/google?code=synthetic-code&state=${encodeURIComponent(s)}`, {
    method, headers, redirect: "manual", signal: AbortSignal.timeout(Math.max(1, Math.min(8000, probeDeadline - Date.now()))),
  });
let error: string | null = null;
try {
  const deadline = Date.now() + 20000;
  for (;;) {
    try { if ((await fetch(`${base}/hello`, { signal: AbortSignal.timeout(1000) })).ok) break; } catch {}
    if (Date.now() > deadline) throw new Error("isolated hub startup timeout");
    await Bun.sleep(75);
  }
  const negatives: [string, string, Record<string, string>, string][] = [
    ["wrong state cannot exchange", "incorrect-state", nav, "GET"],
    ["missing state cannot exchange", "", nav, "GET"],
    ["cross-origin script fetch cannot exchange", state, { ...nav, Origin: "https://foreign.invalid", "Sec-Fetch-Mode": "cors", "Sec-Fetch-Dest": "empty" }, "GET"],
    ["cross-site iframe cannot exchange", state, { ...nav, "Sec-Fetch-Dest": "iframe" }, "GET"],
    ["POST callback cannot exchange", state, nav, "POST"],
    ["DNS-rebound Host cannot exchange", state, { ...nav, Host: "foreign.invalid" }, "GET"],
    ["proxy cannot inherit loopback exception", state, { ...nav, "Tailscale-User-Login": "fixture@example.invalid" }, "GET"],
  ];
  for (const [name, s, headers, method] of negatives) {
    seed(); const before = grants;
    const response = await callback(s, headers, method); await response.text();
    check(name, grants === before, response.status);
    check(`${name}: pending sign-in preserved`, JSON.parse(readFileSync(secretFile, "utf8")).pending_state === state);
  }
  seed();
  const response = await callback(state, nav); const html = await response.text();
  check("top-level cross-site callback completes without pairing token", response.status === 200 && html.includes("Google is connected"), response.status);
  check("valid callback exchanges exactly once", grants === 1);
  check("valid callback clears pending state", JSON.parse(readFileSync(secretFile, "utf8")).pending_state === "");
  if (grants === 1) {
    const replay = await callback(state, nav); await replay.text();
    check("replayed callback cannot exchange twice", grants === 1, replay.status);
  }
  for (const name of ["stdout", "stderr"]) {
    const text = existsSync(join(root, name)) ? readFileSync(join(root, name), "utf8") : "";
    check(`${name} excludes synthetic code/state/secret`, ![state, "synthetic-code", "synthetic-client-secret", "synthetic-refresh"].some(s => text.includes(s)));
  }
} catch (e) { error = e instanceof Error ? e.message : "test exception"; }
finally {
  proc.kill("SIGTERM");
  const exited = await Promise.race([proc.exited.then(() => true), Bun.sleep(5000).then(() => false)]);
  if (!exited) { proc.kill("SIGKILL"); await proc.exited; }
  stub.stop(true);
  let gone = false;
  try { process.kill(proc.pid, 0); } catch { gone = true; }
  check("owned native PID exited", gone);
  const result = { scope: "Own native binary; synthetic Google backend, isolated home, HTTP request metadata; not device OAuth",
    binary, binary_sha256: createHash("sha256").update(readFileSync(binary)).digest("hex"),
    pid: proc.pid, isolated_root: root, checks, error, pass: !error && checks.every(x => x.pass) };
  writeFileSync(receipt, JSON.stringify(result, null, 2) + "\n", { mode: 0o600 });
  process.exitCode = result.pass ? 0 : 1;
}
