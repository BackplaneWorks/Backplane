// The draw board end to end (src/web/draw.js): a headless hub on a throwaway
// home serving WEBDIR, and the web client in headless Chrome. Its geometry
// first (hit, lengths, resize), then the board as a person uses it: the
// composer's Draw button opens it; two drags make two boxes of different
// colours; a box's middle drags it, a side drags its edge; a click on a
// side takes a typed length; Delete removes and Ctrl+Z brings back; Attach
// closes it and the picture (a 960x600 PNG) is the thread's next attachment.
//
//   bun test/tools/draw_web_test.ts BINARY WEBDIR PLAYWRIGHT_MODULE CHROME
//
// Never the live hub: its own home and port, --no-tailscale, loopback only.
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import * as D from "../../src/web/draw.js";

const [binary, web, pw, chrome] = process.argv.slice(2);
if (!binary || !web || !pw || !chrome) throw new Error("usage: draw_web_test.ts BINARY WEBDIR PLAYWRIGHT_MODULE CHROME");
let fails = 0;
const check = (ok: boolean, what: string, got?: unknown) => {
  console.log(`${ok ? "ok" : "FAIL"} ${what}${ok ? "" : ": " + String(JSON.stringify(got)).slice(0, 300)}`);
  if (!ok) fails++;
};

// ---- geometry
const r = { x: 100, y: 100, w: 200, h: 100, color: "#000", dims: {} };
check(D.hit(r, 100, 150) === "l" && D.hit(r, 300, 150) === "r" && D.hit(r, 200, 100) === "t" && D.hit(r, 200, 200) === "b", "a point on each side is that side");
check(D.hit(r, 200, 150) === "in" && D.hit(r, 50, 50) === "", "the middle is in, outside is nothing");
check(D.pick([r, { ...r, x: 150 }], 200, 150)?.i === 1, "the topmost box is picked");
check(JSON.stringify(D.parseLength("40 mm")) === '{"n":40,"unit":"mm"}' && D.parseLength("12.5cm")?.n === 12.5 && D.parseLength("x") === null && D.parseLength("0") === null, "lengths parse");
const w = D.withLength(r, "r", { n: 30, unit: "mm" });
check(w.x === 100 && w.w === 30 * D.UNIT && w.dims.w === "30 mm", "a typed width keeps the left side", w);
const l = D.withLength(r, "l", { n: 30, unit: "" });
check(l.x + l.w === 300 && l.w === 30 * D.UNIT && l.dims.w === "30", "typed on the left side, the right side stays", l);
const s = D.resize({ ...w }, "r", 50, 0);
check(s.w >= 6 && s.x === 100 && !s.dims.w, "a side dragged past the other never flips, and drops the typed width", s);

// ---- the hub
const root = mkdtempSync(join(tmpdir(), "bp-draw-"));
const home = join(root, "home"), user = join(root, "user");
mkdirSync(home); mkdirSync(user); mkdirSync(join(root, "proj"));
const now = Math.floor(Date.now() / 1000);
writeFileSync(join(home, "events.jsonl"), [
  { $: "SettingSet", key: "restart.continue", value: "off", at: now },
  { $: "SettingSet", key: "onboard.done", value: "1", at: now },
  { $: "SettingSet", key: "onboard.uber", value: "1", at: now },
  { $: "ProjectCreated", id: "p1", title: "proj", root: join(root, "proj"), at: now },
  { $: "ThreadCreated", id: "t1", project: "p1", title: "Sketch", env: "local", branch: "", worktree: "", provider: "claude", model: "", at: now },
].map((o) => JSON.stringify(o)).join("\n") + "\n");
const port = (() => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; })();
const hub = Bun.spawn([resolve(binary), "--headless", "--foreground", "--home", home, "--port", String(port), "--web", resolve(web), "--no-tailscale"], {
  env: { HOME: user, PATH: "/usr/bin:/bin", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", DISPLAY: "", WAYLAND_DISPLAY: "" }, stdout: "ignore", stderr: "ignore",
});
try {
  for (let i = 0; i < 120; i++) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {} await Bun.sleep(250); }
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const { chromium } = await import(pw);
  const b = await chromium.launch({ executablePath: chrome });
  const p = await b.newPage({ viewport: { width: 1400, height: 900 } });
  const errors: string[] = [];
  p.on("pageerror", (e: Error) => errors.push(String(e)));
  await p.goto(`http://127.0.0.1:${port}/#token=${token}`);
  await p.waitForSelector(".thread", { timeout: 60000 });
  await p.click(".thread:has-text('Sketch')");
  await p.waitForSelector("button[data-draw]", { timeout: 20000 });
  check((await p.locator("button[data-draw]").getAttribute("title"))?.includes("sketch") === true, "the composer has a Draw button beside Attach");
  await p.click("button[data-draw]");
  await p.waitForSelector(".draw-back canvas");
  const box = (await p.locator(".draw-back canvas").boundingBox())!;
  const k = box.width / D.W;
  const at = (x: number, y: number) => [box.x + x * k, box.y + y * k] as const;
  const drag = async (x0: number, y0: number, x1: number, y1: number) => {
    await p.mouse.move(...at(x0, y0)); await p.mouse.down();
    for (let i = 1; i <= 6; i++) await p.mouse.move(...at(x0 + ((x1 - x0) * i) / 6, y0 + ((y1 - y0) * i) / 6));
    await p.mouse.up();
  };
  const px = (x: number, y: number) => p.evaluate(([x, y]) => Array.from((document.querySelector(".draw-back canvas") as HTMLCanvasElement).getContext("2d")!.getImageData(x, y, 1, 1).data.slice(0, 3)).join(","), [x, y]);
  const white = "255,255,255";

  await drag(100, 100, 300, 250);
  await drag(400, 100, 600, 300);
  const c1 = await px(200, 175), c2 = await px(500, 200);
  check(c1 !== white && c2 !== white && c1 !== c2, "two drags draw two boxes, each a new colour", { c1, c2 });
  await drag(200, 175, 250, 215);
  check((await px(110, 110)) === white && (await px(340, 280)) === c1, "dragging a box's middle moves it", { old: await px(110, 110), now: await px(340, 280) });
  await drag(250, 290, 250, 340);
  check((await px(250, 320)) === c1, "dragging its bottom side resizes it");
  await p.mouse.click(...at(600, 200));
  await p.waitForSelector(".draw-len:not([hidden])");
  await p.keyboard.press("Control+A");
  await p.keyboard.type("30 mm");
  await p.keyboard.press("Enter");
  check((await px(560, 200)) === white && (await px(510, 200)) === c2, "a typed length sets the side (30 mm: 120 px wide)", { past: await px(560, 200), inside: await px(510, 200) });
  await p.mouse.click(...at(460, 300));
  await p.waitForSelector(".draw-len:not([hidden])");
  await p.keyboard.press("Control+A");
  await p.keyboard.type("25");
  await p.keyboard.press("Enter");
  check((await px(460, 150)) === c2 && (await px(460, 250)) === white, "a typed height keeps the top side (25: 100 px tall)", { inside: await px(460, 150), past: await px(460, 250) });
  await p.mouse.click(...at(450, 150));
  await p.keyboard.press("Delete");
  check((await px(450, 150)) === white, "Delete removes the picked box");
  await p.keyboard.press("Control+z");
  check((await px(450, 150)) === c2, "Ctrl+Z brings it back");
  await p.screenshot({ path: join(root, "board.png") });
  await p.click(".draw-back button.primary");
  await p.waitForSelector(".draw-back", { state: "detached" });
  const dir = join(home, "attachments", "t1");
  let files: string[] = [];
  for (let i = 0; i < 40 && !files.length; i++) { await Bun.sleep(250); try { files = readdirSync(dir).filter((f) => f.endsWith(".png")); } catch {} }
  check(files.length === 1 && /(^|-)drawing-\d{8}-\d{6}\.png$/.test(files[0]), "Attach uploads the picture to the thread", files);
  if (files.length) {
    const png = readFileSync(join(dir, files[0]));
    check(png.readUInt32BE(0) === 0x89504e47 && png.readUInt32BE(16) === D.W && png.readUInt32BE(20) === D.H, "a 960x600 PNG", [png.readUInt32BE(16), png.readUInt32BE(20)]);
  }
  await p.waitForTimeout(500);
  const composer = await p.locator(".composer").innerText();
  check(/drawing-.*\.png/.test(composer), "the next message lists it as attached", composer.slice(0, 200));
  check(errors.length === 0, "no page errors", errors);
  await b.close();
} finally {
  hub.kill();
  await hub.exited;
  if (!process.env.KEEP) rmSync(root, { recursive: true, force: true });
  else console.log("kept", root);
}
console.log(fails ? `${fails} failed` : "all passed");
process.exit(fails ? 1 : 0);
