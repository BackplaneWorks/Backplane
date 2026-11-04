// End to end: the Files tab's downloads (core/fm.bend's Fm.zip.*, server.bend's
// FilesHttp, effects/zip.js, host.js's Sock.send_download) on a real KiCad project.
// KiCad's ecc83 demo is copied into a temporary folder and made a git
// repository, with boards/, docs/ (a PDF, a non-ASCII name), mfg/ (gerbers
// from kicad-cli when it is installed, zipped), a file over 1 MB, KiCad's
// backups and a lock file. A headless hub on a temporary home (stand-in
// agents, no tailnet) adds it as a project; the zip of the project and of a
// folder are fetched with the token and checked with unzip -t and against
// the folder less what Fm.zip.rules leaves out; a file downloads exactly;
// no token is 401, a path out is 400, an unknown thread 404. Then the real
// web page opens the thread's Files tab: the board and schematic viewers
// still open, the bar and every row have their download, and clicking one
// downloads. Then another machine: a browser that reaches the hub by a name
// that is not this machine's (hub.e2e.test, resolved to it) pairs as a new
// device with an invitation and downloads with the cookie the hub set (and a
// revoked device no longer can); with ELECTRON set, the desktop app's own
// download handling (desktop/downloads.cjs, files_download_electron.cjs)
// connected to that remote hub saves both into Downloads, opening no window.
// Prints "ok ..." / "FAIL ...".
//
//   bun test/tools/files_download_e2e.ts BINARY WEBDIR WIREDIR [DEMO] [PLAYWRIGHT-CORE] [SHOT.png]
//   (CHROME=<a chrome-headless-shell> when playwright's own is not installed;
//   ELECTRON=<desktop/node_modules/electron's binary>)
//
// BINARY: scripts/build-js.sh src/server/main.bend build/backplane-serve;
// WEBDIR: bend src/web/index.html -o build/web (then scripts/js-unmarshal.py
// and scripts/js-native.py as scripts/dev.sh's build_web); WIREDIR: bend
// test/wire/index.html -o build/wire; DEMO: KiCad's demos/ecc83 (default
// /usr/share/kicad/demos/ecc83). KEEP=1 keeps the temporary folder.
import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync, statSync, lstatSync, writeFileSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";

const [bin, webDir, wire, demo = "/usr/share/kicad/demos/ecc83", pw = "playwright-core", shot = ""] = process.argv.slice(2);
if (!bin || !webDir || !wire) { console.error("usage: bun test/tools/files_download_e2e.ts BINARY WEBDIR WIREDIR [DEMO] [PLAYWRIGHT-CORE] [SHOT.png]"); process.exit(2); }
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 600)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function until<T>(ms: number, f: () => T | undefined | null | false | Promise<T | undefined | null | false>): Promise<T | undefined> {
  const end = Date.now() + ms;
  for (;;) {
    const v = await f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(100);
  }
}
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

// what a zip leaves out: core/fm.bend's rules, read as Fm.zip.skip reads them
const rules: string[] = JSON.parse("[" + /def Fm\.zip\.rules\(\) -> List<&2, String>:\n\s*\[([^\]]*)\]/.exec(readFileSync("src/core/fm.bend", "utf8"))![1] + "]");
const skip = (p: string) => {
  p = p.toLowerCase();
  const name = p.slice(p.lastIndexOf("/") + 1);
  return rules.some((r) => r[0] === "=" ? name === r.slice(1) : r[0] === "*" ? name.endsWith(r.slice(1)) : r[0] === "^" ? name.startsWith(r.slice(1)) : r[0] === "/" && p === r.slice(1));
};
// the folder as its zip should hold it: path -> bytes (folders "x/" -> null)
function expected(root: string, rel: string): Map<string, Buffer | null> {
  const out = new Map<string, Buffer | null>();
  const walk = (abs: string, proj: string, inside: string) => {
    for (const n of readdirSync(abs)) {
      const p = proj ? proj + "/" + n : n;
      const q = inside ? inside + "/" + n : n;
      if (skip(p)) continue;
      const st = lstatSync(join(abs, n));
      if (st.isSymbolicLink()) continue;
      if (st.isDirectory()) { out.set(q + "/", null); walk(join(abs, n), p, q); }
      else if (st.isFile()) out.set(q, readFileSync(join(abs, n)));
    }
  };
  walk(rel ? join(root, rel) : root, rel, "");
  return out;
}

const root = mkdtempSync(join(tmpdir(), "bp-files-dl-e2e-"));
const fake = join(root, "agents");
mkdirSync(fake);
const home = join(root, "home");
const project = join(root, "ecc83");
const run = (cmd: string, args: string[], cwd = project) => execFileSync(cmd, args, { cwd, stdio: ["ignore", "pipe", "pipe"] });
const cli = ["/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli", "kicad-cli"].find((c) => spawnSync(c, ["--version"]).status === 0);

cpSync(demo, project, { recursive: true });
run("git", ["init", "-q", "-b", "main"]);
run("git", ["-c", "user.name=e2e", "-c", "user.email=e2e@example.com", "add", "-A"]);
run("git", ["-c", "user.name=e2e", "-c", "user.email=e2e@example.com", "commit", "-qm", "ecc83 demo"]);
// the hub's own history store, as a step capture makes it (left out of every download)
run("git", ["init", "-q", "--bare", ".backplane/history.git"]);
writeFileSync(join(project, ".env"), "KICAD_TOKEN=secret\n");
mkdirSync(join(project, "boards"));
cpSync(join(project, "ecc83-pp.kicad_pcb"), join(project, "boards/ecc83-pp-rev2.kicad_pcb"));
mkdirSync(join(project, "docs"));
writeFileSync(join(project, "docs/README.md"), "# ECC83 push-pull\n\nBuild notes.\n");
writeFileSync(join(project, "docs/Schéma – notes ✓.md"), "# Schéma\n\nÜberprüft.\n");
mkdirSync(join(project, "mfg/gerbers"), { recursive: true });
if (cli) {
  run(cli, ["sch", "export", "pdf", "-o", "docs/ecc83-pp.pdf", "ecc83-pp.kicad_sch"]);
  run(cli, ["pcb", "export", "gerbers", "-o", "mfg/gerbers/", "ecc83-pp.kicad_pcb"]);
  run(cli, ["pcb", "export", "drill", "-o", "mfg/gerbers/", "ecc83-pp.kicad_pcb"]);
} else {
  writeFileSync(join(project, "mfg/gerbers/ecc83-pp-F_Cu.gtl"), "G04 stand-in*\n".repeat(2000));
}
if (spawnSync("zip", ["-v"]).status === 0) run("zip", ["-qrj", "mfg/ecc83-gerbers.zip", "mfg/gerbers"]);
writeFileSync(join(project, "mfg/panel-scan.bin"), randomBytes(1536 * 1024 + 123));
mkdirSync(join(project, "ecc83-pp-backups"));
writeFileSync(join(project, "ecc83-pp-backups/ecc83-pp-2026-11-01_120000.zip"), "PK backup");
writeFileSync(join(project, "~ecc83-pp.kicad_sch.lck"), "{}");
mkdirSync(join(project, "extra/empty"), { recursive: true });
symlinkSync("/etc", join(project, "extra/etc-link"));

const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--headless", "--foreground", "--web", resolve(webDir)], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9",
    BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake },
  stdout: "ignore",
  stderr: process.env.E2E_LOG ? "inherit" : "ignore",
});
console.log(`hub pid ${proc.pid} port ${port} home ${home}`);
const url = (p: string) => `http://127.0.0.1:${port}${p}`;
const enc = (s: string) => encodeURIComponent(s).replace(/%2F/g, "/");

try {
  const up = await until(20000, async () => { try { return (await fetch(url("/hello"))).status === 200; } catch { return false; } });
  if (!up) throw new Error("the hub did not start");
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const auth = { Authorization: `Bearer ${token}` };
  const order: any[] = [];
  const replies = new Map<number, any>();
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
    for (const c of o.items ?? []) order.push(c);
    if (o.t === "reply") replies.set(Number(o.id), o);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let n = 0;
  const rpc = async (m: string, p: object) => { const id = ++n; ws.send(W.encode(JSON.stringify({ id, m, p }))); return until(15000, () => replies.get(id)); };
  // no first-run tour over the page
  for (const key of ["onboard.done", "onboard.uber"]) await rpc("setting.set", { key, value: "1" });
  await rpc("setting.set", { key: "telemetry.enabled", value: "false" });
  const added = await rpc("project.add", { path: project });
  check("the demo is added as a project", added?.ok === true, added);
  const proj = order.find((c) => c.$ === "ProjectCreated" && (c.root === project || c.root === realpathSync(project)));
  const thread = order.find((c) => c.$ === "ThreadCreated" && c.project === proj?.id)?.id;
  check("the project starts with a thread", !!thread, order.map((c) => c.$));
  const listed = await rpc("files.dir", { thread, path: "" });
  check("files.dir lists the project", String(listed?.result?.fmEntries ?? "").includes("d\t0\tmfg"), listed);
  // the thread's folder: its worktree in a git project (Hub.cwd), what Files lists
  const folder: string = listed?.result?.fmRoot ?? project;

  // the project as a zip
  const zr = await fetch(url(`/files/zip?thread=${enc(thread)}&path=`), { headers: auth });
  check("the project's zip answers 200", zr.status === 200, zr.status);
  check("as application/zip", zr.headers.get("content-type") === "application/zip", zr.headers.get("content-type"));
  check("named for the project", zr.headers.get("content-disposition") === `attachment; filename="ecc83.zip"; filename*=UTF-8''ecc83.zip`, zr.headers.get("content-disposition"));
  const zipFile = join(root, "ecc83.zip");
  writeFileSync(zipFile, Buffer.from(await zr.arrayBuffer()));
  const test = spawnSync("unzip", ["-t", zipFile]);
  check("unzip -t passes", test.status === 0, test.stdout.toString().slice(-400));
  const out = join(root, "out");
  run("unzip", ["-q", zipFile, "-d", out], root);
  const want = expected(folder, "");
  const got = new Map<string, true>();
  const walk = (abs: string, inside: string) => { for (const nm of readdirSync(abs)) { const q = inside ? inside + "/" + nm : nm; if (statSync(join(abs, nm)).isDirectory()) { got.set(q + "/", true); walk(join(abs, nm), q); } else got.set(q, true); } };
  walk(join(out, "ecc83"), "");
  // the hub's own history (.backplane/history.git) appears in the thread's
  // folder with a step capture, which may land while the zip is made
  check("the hub's history is not in the zip", ![...got.keys()].some((k) => k.includes("history.git")));
  for (const k of [...want.keys()]) if (k.startsWith(".backplane/")) want.delete(k);
  for (const k of [...got.keys()]) if (k.startsWith(".backplane/")) got.delete(k);
  check("the zip holds exactly the project less what Fm.zip.rules leaves out", JSON.stringify([...got.keys()].sort()) === JSON.stringify([...want.keys()].sort()),
    { extra: [...got.keys()].filter((k) => !want.has(k)), missing: [...want.keys()].filter((k) => !got.has(k)) });
  const same = [...want].filter(([, b]) => b !== null).every(([p, b]) => readFileSync(join(out, "ecc83", p)).equals(b!));
  check("every file comes out byte for byte", same);
  const diff = spawnSync("diff", ["-r", "-x", ".git", "-x", "ecc83-pp-backups", "-x", "*.lck", "-x", "etc-link", "-x", ".backplane", "-x", ".env", folder, join(out, "ecc83")]);
  check("diff -r against the folder (less what is skipped) is empty", diff.status === 0, diff.stdout.toString().slice(0, 400));
  check("no .git in the zip", ![...got.keys()].some((k) => k.startsWith(".git")));
  check("no .env in the zip", !got.has(".env"));
  check("KiCad's backups and locks are left out", ![...got.keys()].some((k) => k.includes("-backups") || k.endsWith(".lck")));
  check("an empty folder is kept", got.has("extra/empty/"));
  check("the non-ASCII name is kept", got.has("docs/Schéma – notes ✓.md"));
  check("the file over 1 MB is in", got.has("mfg/panel-scan.bin"));

  // the project's own folder (its home, Hub.files.of): the threads'
  // worktrees inside it are left out
  const hr = await fetch(url(`/files/zip?thread=${enc("project-home-" + proj.id)}&path=`), { headers: auth });
  const homeZip = join(root, "home.zip");
  writeFileSync(homeZip, Buffer.from(await hr.arrayBuffer()));
  const homeList = spawnSync("unzip", ["-Z1", homeZip]).stdout.toString().split("\n").filter(Boolean);
  check("the project's folder zips", hr.status === 200 && spawnSync("unzip", ["-tq", homeZip]).status === 0, hr.status);
  check("without the threads' worktrees or .git", homeList.includes("ecc83/ecc83-pp.kicad_pcb") && !homeList.some((l) => l.includes(".backplane/worktrees") || l.includes("/.git/")), homeList.filter((l) => l.includes(".backplane") || l.includes(".git")));

  // a folder below
  const mr = await fetch(url(`/files/zip?thread=${enc(thread)}&path=mfg`), { headers: auth });
  const mfgZip = join(root, "mfg.zip");
  writeFileSync(mfgZip, Buffer.from(await mr.arrayBuffer()));
  check("a folder zips as <name>.zip", mr.status === 200 && (mr.headers.get("content-disposition") ?? "").includes('filename="mfg.zip"'), mr.headers.get("content-disposition"));
  check("its zip tests clean", spawnSync("unzip", ["-tq", mfgZip]).status === 0);
  const mfgList = spawnSync("unzip", ["-Z1", mfgZip]).stdout.toString().split("\n").filter(Boolean);
  check("everything in it is under mfg/", mfgList.length > 3 && mfgList.every((l) => l.startsWith("mfg/")), mfgList);

  // one file
  const gr = await fetch(url(`/files/get?thread=${enc(thread)}&path=${enc("mfg/panel-scan.bin")}`), { headers: auth });
  const big = Buffer.from(await gr.arrayBuffer());
  check("a file downloads as itself", gr.status === 200 && big.equals(readFileSync(join(folder, "mfg/panel-scan.bin"))), [gr.status, big.length]);
  check("as an attachment named for it", gr.headers.get("content-disposition") === `attachment; filename="panel-scan.bin"; filename*=UTF-8''panel-scan.bin`, gr.headers.get("content-disposition"));
  check("as octet-stream", gr.headers.get("content-type") === "application/octet-stream", gr.headers.get("content-type"));
  const ur = await fetch(url(`/files/get?thread=${enc(thread)}&path=${enc("docs/Schéma – notes ✓.md")}`), { headers: auth });
  check("a non-ASCII name downloads", ur.status === 200 && (await ur.text()).includes("Überprüft"), ur.status);
  check("with its name in RFC 5987 form", (ur.headers.get("content-disposition") ?? "").includes("filename*=UTF-8''Sch%C3%A9ma%20%E2%80%93%20notes%20%E2%9C%93.md"), ur.headers.get("content-disposition"));
  const rr = await fetch(url(`/files/get?thread=${enc(thread)}&path=${enc("ecc83-pp.kicad_pro")}`), { headers: { ...auth, Range: "bytes=0-9" } });
  check("a range of a file", rr.status === 206 && (await rr.text()).length === 10, rr.status);

  // refused
  check("no token: 401", (await fetch(url(`/files/zip?thread=${enc(thread)}&path=`))).status === 401);
  check("no token for a file: 401", (await fetch(url(`/files/get?thread=${enc(thread)}&path=docs/README.md`))).status === 401);
  check("a path out of the project: 400", (await fetch(url(`/files/zip?thread=${enc(thread)}&path=..`), { headers: auth })).status === 400);
  check("an absolute path: 400", (await fetch(url(`/files/get?thread=${enc(thread)}&path=/etc/hosts`), { headers: auth })).status === 400);
  check("a symlink out of the project: 404", (await fetch(url(`/files/get?thread=${enc(thread)}&path=extra/etc-link/hosts`), { headers: auth })).status === 404);
  check("an unknown thread: 404", (await fetch(url(`/files/zip?thread=nothing&path=`), { headers: auth })).status === 404);
  check("a thread elsewhere is never read here: 404", (await fetch(url(`/files/zip?thread=${enc("l1~" + thread)}&path=`), { headers: auth })).status === 404);
  check("a backslash path: 400", (await fetch(url(`/files/get?thread=${enc(thread)}&path=${enc("docs\\..\\..\\x")}`), { headers: auth })).status === 400);
  check("a drive path: 400", (await fetch(url(`/files/get?thread=${enc(thread)}&path=${enc("C:/Windows/win.ini")}`), { headers: auth })).status === 400);
  // the hub's own history, by any way of naming it (the project's home)
  const phome = enc("project-home-" + proj.id);
  for (const p of [".backplane/history.git/HEAD", "./.backplane/history.git/HEAD", ".git/HEAD", "ecc83-pp-backups/ecc83-pp-2026-11-01_120000.zip"])
    check(`what a download never holds is not served: ${p}`, (await fetch(url(`/files/get?thread=${phome}&path=${enc(p)}`), { headers: auth })).status === 404);
  const bz = await fetch(url(`/files/zip?thread=${phome}&path=${enc("./.backplane")}`), { headers: auth });
  const bzFile = join(root, "backplane.zip");
  writeFileSync(bzFile, Buffer.from(await bz.arrayBuffer()));
  const bzList = spawnSync("unzip", ["-Z1", bzFile]).stdout.toString();
  check("./.backplane zips without the history or the worktrees", bz.status === 200 && !/history\.git|worktrees/.test(bzList), bzList.slice(0, 300));
  check("a cross-site request: 401", (await fetch(url(`/files/get?thread=${enc(thread)}&path=docs/README.md`), { headers: { ...auth, "Sec-Fetch-Site": "cross-site" } })).status === 401);
  ws.close();

  // the web page
  const { chromium } = await import(pw);
  const browser = await chromium.launch({ headless: true, args: ["--disable-gpu"], executablePath: process.env.CHROME || undefined });
  try {
    const context = await browser.newContext({ viewport: { width: 1500, height: 920 }, acceptDownloads: true });
    const page = await context.newPage();
    const errors: string[] = [];
    page.on("pageerror", (e: any) => errors.push(e.message));
    await page.goto(url(`/#token=${token}`));
    await page.waitForSelector(".sidebar .thread", { timeout: 120000 });
    await page.locator(".sidebar .thread").first().click();

    // how many saturated pixels the biggest visible canvas shows: a drawn
    // board or schematic has copper, wires and symbols in colour, the
    // "Reading the file" line is grey
    const drawn = () => page.evaluate(() => {
      const cs = [...document.querySelectorAll("canvas")].filter((c) => c.checkVisibility() && c.width > 50 && c.height > 50);
      const c = cs.sort((a, b) => b.width * b.height - a.width * a.height)[0];
      if (!c) return 0;
      const t = document.createElement("canvas");
      t.width = 200; t.height = 150;
      const g = t.getContext("2d")!;
      g.drawImage(c, 0, 0, 200, 150);
      const d = g.getImageData(0, 0, 200, 150).data;
      let n = 0;
      for (let i = 0; i < d.length; i += 4) if (Math.max(d[i], d[i + 1], d[i + 2]) - Math.min(d[i], d[i + 1], d[i + 2]) > 40) n++;
      return n;
    });
    // the board and the schematic still open
    await page.locator('[data-on-click="view-kind"][data-value="board"]:visible').first().click();
    const boardShown = await until(60000, async () => (await drawn()) > 50);
    check("the board viewer opens and draws", !!boardShown, await drawn());
    if (shot) await page.screenshot({ path: shot.replace(/\.png$/, "-board.png") });
    const boardPixels = await drawn();
    await page.locator('[data-on-click="view-kind"][data-value="schematic"]:visible').first().click();
    // (the board's picture first goes away)
    await until(10000, async () => (await drawn()) !== boardPixels);
    const schShown = await until(60000, async () => { const n = await drawn(); return n > 50 && n !== boardPixels; });
    check("the schematic viewer opens and draws", !!schShown, await drawn());
    if (shot) await page.screenshot({ path: shot.replace(/\.png$/, "-schematic.png") });
    await page.locator('[data-on-click="fm"]:visible').first().click();
    await page.waitForSelector(".fm-list .fm-item", { timeout: 20000 });
    const rows = await page.locator(".fm-list .fm-item").count();
    const dls = await page.locator(".fm-list .fm-item a.fm-dl").count();
    check("every row has its download", rows > 5 && rows === dls, [rows, dls]);
    const bar = page.locator(".fm-bar a.fm-dl-bar");
    check("the bar has Download", (await bar.count()) === 1 && (await bar.innerText()).trim() === "Download", await bar.count());
    check("the bar zips the folder on show", (await bar.getAttribute("href")) === `/files/zip?thread=${enc(thread)}&path=`, await bar.getAttribute("href"));
    const mfgRow = page.locator(".fm-item", { has: page.locator(".fm-name", { hasText: /^mfg$/ }) });
    check("a folder's row zips it", (await mfgRow.locator("a.fm-dl").getAttribute("href")) === `/files/zip?thread=${enc(thread)}&path=mfg`);
    if (shot) await page.screenshot({ path: shot });
    const [download] = await Promise.all([page.waitForEvent("download", { timeout: 30000 }), bar.click()]);
    check("Download saves <project>.zip", download.suggestedFilename() === "ecc83.zip", download.suggestedFilename());
    const saved = join(root, "page.zip");
    await download.saveAs(saved);
    check("the page's zip tests clean", spawnSync("unzip", ["-tq", saved]).status === 0);
    const readme = page.locator(".fm-item", { has: page.locator(".fm-name", { hasText: /^ecc83-pp\.kicad_pro$/ }) });
    const [one] = await Promise.all([page.waitForEvent("download", { timeout: 30000 }), readme.locator("a.fm-dl").click()]);
    check("a row's download saves the file", one.suggestedFilename() === "ecc83-pp.kicad_pro", one.suggestedFilename());
    const savedOne = join(root, "one.kicad_pro");
    await one.saveAs(savedOne);
    check("byte for byte", readFileSync(savedOne).equals(readFileSync(join(folder, "ecc83-pp.kicad_pro"))));
    check("the row's download leaves the folder on show", (await page.locator(".fm-list .fm-item").count()) === rows);
    await mfgRow.locator("button.fm-row").click();
    await until(10000, async () => (await page.locator(".fm-bar a.fm-dl-bar").getAttribute("href"))?.endsWith("path=mfg"));
    check("a row's own button still opens its folder", (await page.locator(".fm-bar a.fm-dl-bar").getAttribute("href"))?.endsWith("path=mfg"), await page.locator(".fm-bar a.fm-dl-bar").getAttribute("href"));
    check("no page errors", errors.length === 0, errors);
  } finally {
    await browser.close();
  }

  // Another machine on the tailnet: the hub under a name that is not this
  // machine's (the browser resolves it to the hub), a page paired as a new
  // device with an invitation (Settings → Devices → Add a device), never
  // with the hub's token. Its downloads go with the cookie the hub set.
  const host = "hub.e2e.test";
  const remote = `http://${host}:${port}`;
  const invite = async () => {
    const r = await fetch(url("/auth/invitations"), { method: "POST", headers: auth });
    const j: any = await r.json();
    return new URLSearchParams(String(j.link ?? "").split("#")[1] ?? "").get("invite") ?? "";
  };
  const { chromium: rchromium } = await import(pw);
  const rb = await rchromium.launch({ headless: true, args: ["--disable-gpu", `--host-resolver-rules=MAP ${host} 127.0.0.1`], executablePath: process.env.CHROME || undefined });
  try {
    const secret = await invite();
    check("an invitation for another device", /^[a-f0-9]{32}$/.test(secret), secret);
    const rc = await rb.newContext({ viewport: { width: 1500, height: 920 }, acceptDownloads: true });
    const rp = await rc.newPage();
    const errors: string[] = [];
    rp.on("pageerror", (e: any) => errors.push(e.message));
    await rp.goto(`${remote}/pair#invite=${secret}`);
    await rp.getByRole("button", { name: "Connect this device" }).click({ timeout: 30000 });
    await rp.waitForSelector(".sidebar .thread", { timeout: 120000 });
    const cookie = (await rc.cookies(remote)).find((c: any) => c.name === "bp_token");
    check("the laptop holds a device credential, not the hub's token", !!cookie && /^[a-f0-9]{32}$/.test(cookie.value) && cookie.value !== token && cookie.httpOnly, cookie && { ...cookie, value: cookie.value === token ? "the token" : "other" });
    await rp.locator(".sidebar .thread").first().click();
    await rp.locator('[data-on-click="fm"]:visible').first().click();
    await rp.waitForSelector(".fm-list .fm-item a.fm-dl", { timeout: 20000 });
    const bar = rp.locator(".fm-bar a.fm-dl-bar");
    const href = (await bar.getAttribute("href")) ?? "";
    check("the link carries no credential", !/token|key|secret/i.test(href), href);
    const [download] = await Promise.all([rp.waitForEvent("download", { timeout: 30000 }), bar.click()]);
    const saved = join(root, "remote.zip");
    await download.saveAs(saved);
    check("a paired laptop downloads the project's zip", download.suggestedFilename() === "ecc83.zip" && spawnSync("unzip", ["-tq", saved]).status === 0, download.suggestedFilename());
    const big = rp.locator(".fm-item", { has: rp.locator(".fm-name", { hasText: /^ecc83-pp\.kicad_pcb$/ }) });
    const [one] = await Promise.all([rp.waitForEvent("download", { timeout: 30000 }), big.locator("a.fm-dl").click()]);
    const savedOne = join(root, "remote.kicad_pcb");
    await one.saveAs(savedOne);
    check("and one of its files", readFileSync(savedOne).equals(readFileSync(join(folder, "ecc83-pp.kicad_pcb"))));
    check("no page errors on the laptop", errors.length === 0, errors);
    // the same credential as a phone or a native client sends it
    const dev = cookie?.value ?? "";
    const q = `/files/get?thread=${enc(thread)}&path=docs/README.md`;
    check("the device's cookie lets it in", (await fetch(url(q), { headers: { Cookie: `bp_token=${dev}` } })).status === 200);
    check("the device's bearer lets it in", (await fetch(url(q), { headers: { Authorization: `Bearer ${dev}` } })).status === 200);
    check("a made-up device credential does not", (await fetch(url(q), { headers: { Cookie: `bp_token=${"0".repeat(32)}` } })).status === 401);
    const revoke = await fetch(url("/auth/devices"), { headers: auth });
    const devices: any = await revoke.json().catch(() => ({}));
    const id = (devices.devices ?? []).find((d: any) => !d.revoked)?.id;
    if (id) {
      await fetch(url("/auth/revoke"), { method: "POST", headers: { ...auth, "X-Backplane-Device": id } });
      check("a revoked device no longer downloads", (await fetch(url(q), { headers: { Cookie: `bp_token=${dev}` } })).status === 401);
    } else check("the device is listed", false, devices);
  } finally {
    await rb.close();
  }

  // The desktop app connected to that remote hub (ELECTRON=<electron>): its
  // own download handling (desktop/downloads.cjs) saves into Downloads.
  if (process.env.ELECTRON) {
    const out = join(root, "electron");
    const r = spawnSync(process.env.ELECTRON, [resolve("test/tools/files_download_electron.cjs"), remote, await invite(), "invite", out, `MAP ${host} 127.0.0.1`], { timeout: 180000, env: Object.fromEntries(Object.entries(process.env).filter(([k]) => k !== "ELECTRON_RUN_AS_NODE")) });
    const line = r.stdout.toString().split("\n").find((l) => l.startsWith("RESULT "));
    const rep: any = line ? JSON.parse(line.slice(7)) : { stderr: r.stderr.toString().slice(-800) };
    check("the desktop app pairs with the remote hub as a device", rep.paired === 200 && rep.cookie === "a device credential", rep);
    check("Download configures the native Save dialog", r.status === 0 && rep.finished?.length === 2 && rep.finished.every((f: any) => f.state === "completed") && rep.done.every((d: any) => !!d.chooser), rep);
    const zipAt = join(out, "downloads", "ecc83.zip");
    check("as <project>.zip, a sound zip", existsSync(zipAt) && spawnSync("unzip", ["-tq", zipAt]).status === 0, rep.finished);
    check("a row's file byte for byte", existsSync(join(out, "downloads", "ecc83-pp.kicad_pro")) && readFileSync(join(out, "downloads", "ecc83-pp.kicad_pro")).equals(readFileSync(join(folder, "ecc83-pp.kicad_pro"))));
    check("no new window and no navigation", rep.windows === 0 && rep.navigations?.length === 0 && rep.stayed === true, rep);
  } else console.log("skip the desktop app (ELECTRON unset)");
} catch (e) {
  check("run", false, String((e as any)?.stack ?? e));
} finally {
  proc.kill();
  await proc.exited;
  if (!process.env.KEEP) rmSync(root, { recursive: true, force: true });
  else console.log(`kept ${root}`);
}
process.exit(failed ? 1 : 0);
