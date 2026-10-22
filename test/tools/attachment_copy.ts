// The native fixture calls the production Media.file path directly. --hub
// sends attach.file through a real hub. Only generated files and homes
// are used; provider commands are stand-ins and no agent is started.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, openSync, closeSync, ftruncateSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { cbor, plain } from "../../src/web/wire.js";

const binary = resolve(process.argv[2] ?? "build/attachment-copy");
const hub = process.argv.includes("--hub");
const broken = process.argv.includes("--expect-broken");
const base = mkdtempSync(join(tmpdir(), "bp-attachment-copy-"));
const user = join(base, "user"), home = join(base, "home"), fake = join(base, "fake"), project = join(base, "project");
let checks = 0;
let proc: ReturnType<typeof Bun.spawn> | undefined, ws: WebSocket | undefined;
let stdout: Promise<string> | undefined, stderr: Promise<string> | undefined;
let id = 0;
const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
function check(ok: unknown, label: string) {
  if (!ok) throw new Error(`FAIL ${label}`);
  console.log(`ok ${label}`); checks++;
}
async function until<T>(probe: () => T | Promise<T>, ms = 15_000): Promise<T> {
  const end = Date.now() + ms;
  do { const value = await probe(); if (value) return value; await sleep(25); } while (Date.now() < end);
  throw new Error("timed out waiting for attachment reply");
}
// Standard CBOR text-key requests, decoded by the real hub protocol.
function head(m: number, n: number): number[] {
  return n < 24 ? [m * 32 + n] : n < 256 ? [m * 32 + 24, n] : n < 65536 ? [m * 32 + 25, n >> 8, n & 255] : [m * 32 + 26, n >>> 24, n >>> 16 & 255, n >>> 8 & 255, n & 255];
}
function encode(v: any): number[] {
  if (typeof v === "string") { const b = [...Buffer.from(v)]; return [...head(3, b.length), ...b]; }
  if (typeof v === "number") return head(0, v);
  if (Array.isArray(v)) return [...head(4, v.length), ...v.flatMap(encode)];
  return [...head(5, Object.keys(v).length), ...Object.entries(v).flatMap(([k, x]) => [...encode(k), ...encode(x)])];
}
try {
  if (process.getuid?.() === 0) throw new Error("run as an ordinary user so chmod 000 exercises a real read failure");
  for (const p of [user, home, fake, project]) mkdirSync(p);
  for (const name of ["claude", "codex", "grok", "muse", "agy", "pi", "gh", "tailscale", "curl", "wget", "ssh", "kicad-cli", "freecad", "busctl"]) {
    writeFileSync(join(fake, name), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, name), 0o755);
  }
  const env: Record<string, string> = {
    HOME: user, BACKPLANE_HOME: home, XDG_CONFIG_HOME: join(user, ".config"), XDG_CACHE_HOME: join(user, ".cache"), XDG_DATA_HOME: join(user, ".local/share"),
    PATH: `${fake}:/usr/bin:/bin`, TMPDIR: base, LANG: "C.UTF-8", DISPLAY: "", WAYLAND_DISPLAY: "",
    BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: "1", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
    BACKPLANE_LOCK_DIR: join(base, "locks"), BACKPLANE_DEPLOY_LOCKS: join(base, "deploy-locks"),
    HTTP_PROXY: "http://127.0.0.1:1", HTTPS_PROXY: "http://127.0.0.1:1", ALL_PROXY: "http://127.0.0.1:1", NO_PROXY: "127.0.0.1,localhost",
  };
  let copy: (path: string) => Promise<any>;
  if (hub) {
    const events = [
      { $: "ProjectCreated", id: "p", title: "Example", root: project, at: 1 },
      { $: "ThreadCreated", id: "t", project: "p", title: "Example", env: "local", branch: "", worktree: "", provider: "claude", model: "", at: 2 },
      { $: "SettingSet", key: "restart.continue", value: "off", at: 3 },
    ];
    writeFileSync(join(home, "events.jsonl"), events.map(e => JSON.stringify(e)).join("\n") + "\n");
    const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
    const port = listener.port; listener.stop(true);
    if (port < 3799) throw new Error("test port must be at least 3799");
    proc = Bun.spawn([binary, "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], { env, cwd: base, stdout: "pipe", stderr: "pipe" });
    stdout = new Response(proc.stdout).text(); stderr = new Response(proc.stderr).text();
    await until(async () => { try { return (await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(300) })).ok; } catch { return false; } }, 30_000);
    const code = readFileSync(new URL("../../src/core/cbor.bend", import.meta.url), "utf8");
    const dict = (name: string) => JSON.parse(code.match(new RegExp(`def Cbor\\.${name}\\(\\)[\\s\\S]*?\\n  (\\[[\\s\\S]*?\\])`))![1]);
    const keys = dict("keys"), words = dict("words"), frames: any[] = [];
    ws = new WebSocket(`ws://127.0.0.1:${port}/ws`); ws.binaryType = "arraybuffer";
    ws.onmessage = e => frames.push(plain(cbor(new Uint8Array(e.data as ArrayBuffer), keys, words)));
    await until(() => frames.some(f => f?.t === "log"));
    copy = async path => {
      const request = ++id;
      ws!.send(new Uint8Array(encode({ id: request, m: "attach.file", p: { thread: "t", path } })));
      return await until(() => frames.find(f => f?.t === "reply" && f.id === request));
    };
  } else {
    copy = async path => {
      const r = Bun.spawnSync([binary, path, home], { env, cwd: base, stdout: "pipe", stderr: "pipe", timeout: 30_000 });
      if (r.exitCode !== 0) throw new Error(`attachment fixture exited ${r.exitCode}: ${r.stderr.toString()}`);
      return JSON.parse(r.stdout.toString());
    };
  }
  const folder = join(home, "attachments", "t");
  const files = () => existsSync(folder) ? readdirSync(folder).sort() : [];
  const source = join(project, "unreadable.txt");
  writeFileSync(source, "synthetic important bytes\n"); chmodSync(source, 0);
  let result = await copy(source);
  if (broken) {
    check(result.ok === true, "baseline falsely reports unreadable source as attached");
    check(readFileSync(result.result.attached).length === 0, "baseline creates an empty attachment for nonempty unreadable source");
  } else {
    check(result.ok === false && result.result.upload === true, "unreadable source returns the existing upload error shape");
    check(result.result.text.includes("could not read file") && result.result.text.includes(source), "read failure identifies the unreadable source");
    check(!result.result.attached && files().length === 0, "failed read creates no attachment");
  }
  chmodSync(source, 0o600);
  // A genuine zero-byte file remains a valid attachment.
  const empty = join(project, "empty.bin"); writeFileSync(empty, "");
  result = await copy(empty);
  check(result.ok === true && result.result.thread === "t", "readable empty source succeeds");
  check(readFileSync(result.result.attached).length === 0, "readable empty source stays empty");
  const bytes = Buffer.from([0, 10, 13, 127, 128, 195, 169, 255]);
  const binarySource = join(project, "binary.bin"); writeFileSync(binarySource, bytes);
  result = await copy(binarySource);
  check(result.ok === true, "readable binary source succeeds");
  const successful = result.result.attached;
  check(readFileSync(successful).equals(bytes), "copied binary bytes are exact");
  const before = files();
  for (const path of [join(project, "missing.bin"), project]) {
    result = await copy(path);
    check(result.ok === false && !result.result.attached && result.result.text.includes("not a file"), "missing and non-file sources remain rejected");
    check(JSON.stringify(files()) === JSON.stringify(before), "rejected source creates no file");
  }
  const large = join(project, "large.bin"), fd = openSync(large, "w"); ftruncateSync(fd, 10485761); closeSync(fd);
  result = await copy(large);
  check(result.ok === false && result.result.text.includes("limited to 10 MB"), "existing attachment size limit remains enforced");
  check(JSON.stringify(files()) === JSON.stringify(before), "oversize source creates no file");
  if (!broken) {
    chmodSync(binarySource, 0);
    result = await copy(binarySource);
    check(result.ok === false, "a later read failure is still rejected");
    check(readFileSync(successful).equals(bytes), "failed replacement preserves the earlier successful attachment");
    check(JSON.stringify(files()) === JSON.stringify(before), "failed replacement creates no extra file");
    chmodSync(binarySource, 0o600);
  }
  console.log(`${checks} attachment copy assertions passed${broken ? " (baseline reproduction)" : ""}`);
} finally {
  ws?.close();
  if (proc) {
    if (proc.exitCode === null && proc.signalCode === null) proc.kill("SIGTERM");
    await Promise.race([proc.exited, sleep(4_000)]);
    if (proc.exitCode === null && proc.signalCode === null) proc.kill("SIGKILL");
    await proc.exited;
    if (process.env.ATTACHMENT_TEST_DEBUG) { console.error(await stdout); console.error(await stderr); }
  }
  rmSync(base, { recursive: true, force: true });
}
