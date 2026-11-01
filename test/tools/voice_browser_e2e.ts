// Drive the real web client against a test hub with the local engine, with
// a fake microphone playing demo.wav, and record a video plus a timeline of
// the composer's text (to show the interim→final flow is calm).
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync, readFileSync, existsSync, chmodSync, symlinkSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";

// WAV is a public/synthetic fixture: 16 kHz mono, four sentences containing
// KiCad, nRF52840, SPI, a differential pair and ESP32. No private speech.
//   bun test/tools/voice_browser_e2e.ts MODE BINARY WEB WIREDIR LOCAL_DIR WAV OUT
// MODE hub records through a fake pw-record; client records in Chromium.
// The client route uses another loopback address to exercise hubHere=false.
// This is a transport simulation, not a physical second-device test.
const [mode, binary, web, wire, localDir, wav, output] = process.argv.slice(2);
if (!["hub", "client"].includes(mode) || !output) throw Error("usage: MODE BINARY WEB WIREDIR LOCAL_DIR WAV OUT");
const { chromium } = await import(process.env.BACKPLANE_PLAYWRIGHT_MODULE || resolve("tools/browser/node_modules/playwright-core/index.mjs"));
const out = resolve(output), WAV = resolve(wav);
mkdirSync(out, { recursive: true });
for (const f of [resolve(wire)]) for (const g of require("fs").readdirSync(f).filter((x: string) => x.endsWith(".js"))) (0, eval)(readFileSync(`${f}/${g}`, "utf8"));
const W = (globalThis as any).Wire;

const home = mkdtempSync(join(tmpdir(), "bp-voice-demo-"));
mkdirSync(join(home, "voice"), { recursive: true });
symlinkSync(resolve(localDir), join(home, "voice/local"));
const fake = join(home, "fakebin");
mkdirSync(fake);
// the hub's "microphone": demo.wav in real time at the rate asked for
writeFileSync(join(fake, "pw-record"), `#!/bin/sh
rate=16000
while [ $# -gt 0 ]; do [ "$1" = --rate ] && rate=$2; shift; done
ffmpeg -nostdin -loglevel error -re -i "$BP_VOICE_TEST_WAV" -f s16le -ac 1 -ar $rate -
exec ffmpeg -nostdin -loglevel error -re -f lavfi -i anullsrc=r=$rate:cl=mono -f s16le -
`);
chmodSync(join(fake, "pw-record"), 0o755);
mkdirSync(join(home, "agents"));

const port = 3799 + Math.floor(Math.random() * 100);
const host = mode === "client" ? "127.0.0.2" : "127.0.0.1";
const hub = Bun.spawn([resolve(binary), "--home", home, "--port", String(port), "--host", host, "--no-tailscale", "--web", resolve(web), "--foreground"], {
  env: { ...process.env, BP_VOICE_TEST_WAV: WAV, DISPLAY: "", PATH: `${fake}:${process.env.PATH}`, BACKPLANE_AGENTS: join(home, "agents"), BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9" },
  stdout: "ignore", stderr: Bun.file(join(out, `hub-${mode}.log`)),
});
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
for (let i = 0; i < 100 && !existsSync(join(home, "token")); i++) await sleep(100);
process.on("exit", () => hub.kill());
const token = readFileSync(join(home, "token"), "utf8").trim();
await sleep(500);

// a project, a thread, the dictionary (over the hub's socket, as a client)
const ws = new WebSocket(`ws://${host}:${port}/ws?token=${token}`);
ws.binaryType = "arraybuffer";
const msgs: any[] = [];
ws.onmessage = (e) => msgs.push(JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer))));
await new Promise((r) => (ws.onopen = r));
let n = 0;
const send = (m: string, p: any) => ws.send(W.encode(JSON.stringify({ id: ++n, m, p })));
for (const [k, v] of [["onboard.done", "1"], ["voice.e.kicad", "KiCad"], ["voice.e.spi", "SPI"], ["voice.e.esp32", "ESP32"], ["voice.e.nrf52840", "nRF52840"], ["voice.l.differential pair", "diff pair\t2\t0"]]) send("setting.set", { key: k, value: v });
send("project.add", { path: home });
await sleep(800);
const items = msgs.flatMap((o) => o.items ?? []);
const proj = items.find((c: any) => c.$ === "ProjectCreated");
send("thread.create", { project: proj.id, title: "Dictation" });
await sleep(800);
// Keep the observer open to verify learned corrections persist in the log.

const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM || `${process.env.HOME}/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome`,
  args: [`--unsafely-treat-insecure-origin-as-secure=http://${host}:${port}`, "--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream", `--use-file-for-fake-audio-capture=${WAV}%noloop`, "--autoplay-policy=no-user-gesture-required"],
});
const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, recordVideo: { dir: out, size: { width: 1280, height: 800 } }, permissions: ["microphone"] });
const page = await ctx.newPage();
await page.goto(`http://${host}:${port}/#token=${token}`);
await page.waitForTimeout(3000);
for (let i = 0; i < 3; i++) await page.getByRole("button", { name: "Skip" }).first().click({ timeout: 1500 }).catch(() => {});
await page.getByText("Dictation", { exact: true }).first().click().catch(() => {});
await page.waitForTimeout(1500);
await page.screenshot({ path: join(out, `start-${mode}.png`) });
process.on("exit", () => hub.kill());
console.log("mic buttons:", await page.evaluate(() => [...document.querySelectorAll("button")].filter((b) => /mic/.test(b.className)).map((b) => b.className + " visible=" + (b.offsetParent !== null)).join(" | ")));
// the composer's text and the strip, every 100 ms
const timeline: { t: number; draft: string; strip: string }[] = [];
const t0 = Date.now();
const sample = async () => {
  const d = await page.evaluate(() => ({
    draft: (document.getElementById("composer") as HTMLTextAreaElement | null)?.value ?? "",
    strip: document.querySelector(".voice-strip .voice-head")?.textContent ?? "",
    fixes: [...document.querySelectorAll(".voice-fixes button")].map((b) => b.textContent).join(" | "),
  }));
  timeline.push({ t: Date.now() - t0, ...d } as any);
};
try { await page.locator("button.mic").first().click({ timeout: 5000 }); } catch (e) { console.log("click failed"); hub.kill(); await browser.close(); process.exit(1); }
const end = Date.now() + 30000;
while (Date.now() < end) {
  await sample();
  await page.waitForTimeout(100);
}
await page.screenshot({ path: join(out, `final-${mode}.png`) });
await page.locator("button.mic").first().click().catch(() => {});
await page.waitForTimeout(1500);
await sample();
await page.screenshot({ path: join(out, `after-${mode}.png`) });
// The correction chips must edit the locally owned draft and remember undo.
const undo = page.locator('.voice-fixes button').filter({ hasText: 'ESP32 ← ASP32' });
assert.equal(await undo.count(), 1);
await undo.click();
await page.waitForTimeout(1000);
assert((await page.locator('#composer').inputValue()).includes('ASP32'));
assert(msgs.flatMap(o => o.items ?? []).some(c => c.$ === 'SettingSet' && c.key === 'voice.l.asp32' && c.value.endsWith('\t1')));
await page.screenshot({ path: join(out, `undo-${mode}.png`) });
writeFileSync(join(out, `events-${mode}.json`), JSON.stringify(msgs.filter(o => o.t === 'voice'), null, 1));
ws.close();
await ctx.close();
await browser.close();
hub.kill();
writeFileSync(join(out, `timeline-${mode}.json`), JSON.stringify(timeline, null, 1));
// calmness: how often text before the tail changed (it should never)
let rewrites = 0, changes = 0;
let prev = "";
for (const s of timeline) {
  if (s.draft !== prev) {
    changes++;
    let k = 0;
    while (k < prev.length && k < s.draft.length && prev[k] === s.draft[k]) k++;
    // a change that starts more than 3 words back from the old end is a rewrite
    const back = prev.slice(k).trim().split(/\s+/).filter(Boolean).length;
    if (back > 3) rewrites++;
    prev = s.draft;
  }
}
const states = [...new Set(timeline.map((s) => s.strip))];
const summary = { mode, changes, rewritesOver3Words: rewrites, states, final: timeline[timeline.length - 1] };
console.log(JSON.stringify(summary, null, 1));
assert(changes > 10, "live text should update during speech");
assert.equal(rewrites, 0, "settled text must survive redraws");
for (const term of ["KiCad", "nRF52840", "SPI", "diff pair", "ESP32"]) assert(summary.final.draft.includes(term), "missing term " + term);
assert(summary.final.draft.startsWith("Open the board"));
assert(summary.final.draft.endsWith("ESP32."));
assert(!summary.final.strip, "Stop must finish the session");

