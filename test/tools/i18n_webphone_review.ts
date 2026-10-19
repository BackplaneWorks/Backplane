// Reproducible Japanese visual/IME review against a freshly built, isolated hub.
// bun test/tools/i18n_webphone_review.ts BINARY WEB_DIR SHOTS_DIR [3820..3826] [only-scene]
// Seeds idle synthetic events; never sends a message or starts an agent.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [binary, webDir, outDir, rawPort = "3820", onlyScene = ""] = process.argv.slice(2);
if (!binary || !webDir || !outDir) throw new Error("BINARY WEB_DIR SHOTS_DIR required");
const bin = resolve(binary), web = resolve(webDir), out = resolve(outDir), port = Number(rawPort);
if (bin.startsWith(join(homedir(), ".local/share/backplane"))) throw new Error("installed binary forbidden");
if (!Number.isInteger(port) || port < 3820 || port > 3826) throw new Error("use port 3820..3826");
const pw = resolve("tools/browser/node_modules/playwright-core");
if (!existsSync(pw)) throw new Error("Run bun install --frozen-lockfile in tools/browser first");
const { chromium } = await import(pw);
const cache = join(homedir(), ".cache/ms-playwright");
const candidates = existsSync(cache) ? readdirSync(cache).sort().reverse().flatMap((d) => [
  join(cache, d, "chrome-headless-shell-linux64/chrome-headless-shell"),
  join(cache, d, "chrome-linux64/chrome"), join(cache, d, "chrome-linux/chrome"),
]) : [];
const executablePath = process.env.BACKPLANE_CHROME || candidates.find(existsSync);
if (!executablePath) throw new Error("no headless Chromium installed");
const root = mkdtempSync(join(tmpdir(), "bp-i18n43b-"));
const home = join(root, "home"), proj = join(root, "project");
mkdirSync(home); mkdirSync(proj); mkdirSync(out, { recursive: true });
const at = Math.floor(Date.now() / 1000);
const events = [
  { $: "ProjectCreated", id: "p43", title: "試作基板", root: proj, at },
  { $: "ThreadCreated", id: "t43", project: "p43", title: "New thread", env: "local", branch: "", worktree: "", provider: "claude", model: "", at },
  { $: "MessagePosted", thread: "t43", msg: "m43", role: "user", text: "接続を確認してください。", at },
  { $: "MessagePosted", thread: "t43", msg: "a43", role: "assistant", text: "配線を確認しました。", at },
  { $: "ThreadCreated", id: "bt43", project: "p43", title: "ミケ", env: "local", branch: "", worktree: "", provider: "claude", model: "", at },
  { $: "BotSet", id: "b43", name: "ミケ", thread: "bt43", look: 0, persona: "設計を手伝うボット", at },
  { $: "RoomSet", id: "r43", name: "設計", members: "b43", at },
  { $: "SettingSet", key: "onboard.done", value: "1", at },
  { $: "SettingSet", key: "onboard.uber", value: "1", at },
  { $: "SettingSet", key: "restart.continue", value: "off", at },
];
writeFileSync(join(home, "events.jsonl"), events.map((e) => JSON.stringify(e)).join("\n") + "\n");
// A fixture must stay inert even if a future UI action accidentally submits.
const stubs = join(root, "provider-stubs"), invoked = join(root, "provider-invoked");
mkdirSync(stubs);
for (const name of ["claude", "codex", "grok", "muse", "antigravity"]) {
  writeFileSync(join(stubs, name), `#!/bin/sh
# Detection is expected on startup; these stubs never reach a provider.
case "$*" in
  --version|models|"serve --no-session-log") exit 1 ;;
  "-p --input-format stream-json --output-format stream-json --verbose")
    IFS= read -r first
    case "$first" in *'"request_id":"models"'*) exit 1 ;; esac ;;
esac
printf "provider turn attempted\\n" >> "$BACKPLANE_TEST_PROVIDER_INVOKED"
exit 1
`, { mode: 0o755 });
}
try {
  await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(600) });
  throw new Error("test port already occupied");
} catch (e: any) { if (e.message === "test port already occupied") throw e; }
const hub = Bun.spawn([bin, "--foreground", "--home", home, "--port", String(port), "--no-tailscale", "--web", web], {
  cwd: process.cwd(), env: { ...process.env, PATH: `${stubs}:${process.env.PATH || ""}`, DISPLAY: "", BACKPLANE_HOME: home, BACKPLANE_NO_UPDATE: "1", BACKPLANE_TEST_PROVIDER_INVOKED: invoked },
  stdout: Bun.file(join(root, "hub.log")), stderr: Bun.file(join(root, "hub.err")),
});
let browser: any;
const pageErrors: string[] = [];
try {
  browser = await chromium.launch({ executablePath, headless: true, args: ["--disable-gpu"] });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  page.on("pageerror", (e: Error) => pageErrors.push(String(e)));
  // Playwright waits on a condition; no message is ever sent to the fixture hub.
  await page.goto(`http://127.0.0.1:${port}/?lang=ja`, { waitUntil: "networkidle", timeout: 60000 });
  await page.locator('[data-on-click="select"][data-value="t43"]').first().waitFor();
  const children = Bun.spawnSync(["ps", "--ppid", String(hub.pid), "-o", "pid=,args="]).stdout.toString();
  if (/\b(?:claude|codex)(?:\s|$)/.test(children)) throw new Error("unexpected provider child");
  writeFileSync(join(out, "web-ja-children.txt"), children || "No child processes\n");
  const overflow: Record<string, unknown>[] = [];
  async function shot(what: string, theme: string, width: number) {
    if (onlyScene && what !== onlyScene) return;
    // The live handlers must ignore both standard composition and legacy 229.
    const ime = await page.evaluate(() => {
        const fields = [...document.querySelectorAll("#composer,[data-enter]")];
        return fields.flatMap((input) => [new KeyboardEvent("keydown", { key: "Enter", isComposing: true, bubbles: true, cancelable: true }),
          new KeyboardEvent("keydown", { key: "Enter", keyCode: 229, bubbles: true, cancelable: true })]
          .map((event) => { input.dispatchEvent(event); return !event.defaultPrevented; }));
      });
    if (ime.some((ok: boolean) => !ok)) throw new Error("IME Enter reached a submit handler");
    await page.screenshot({ path: join(out, `web-ja-${what}-${theme}-${width}.png`), fullPage: true });
    overflow.push(await page.evaluate(({ what, theme, width }: any) => {
      const bad = [...document.querySelectorAll(".set-label,.set-note,.bform-btns button,.set-ctl button")].filter((e) => {
        const r = e.getBoundingClientRect(); return r.width > 0 && e.scrollWidth > e.clientWidth + 2;
      }).map((e) => e.textContent);
      return { what, theme, width, pageOverflow: document.documentElement.scrollWidth > innerWidth + 2, clipped: bad };
    }, { what, theme, width }));
    writeFileSync(join(out, `web-ja-${onlyScene ? onlyScene + "-" : ""}overflow.json`), JSON.stringify(overflow, null, 2));
  }
  async function closeSettings() {
    const close = page.locator('[data-on-click="flag"][data-value="settings"]').filter({ hasText: "閉じる" });
    if (await close.count()) await close.first().click();
  }
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 1000 });
    for (const theme of ["dark", "light"]) {
      await closeSettings();
      if (width === 390 && await page.locator(".app.picked").count()) await page.locator("main button.back").first().click();
      await page.locator('[data-on-click="flag"][data-value="settings"]').first().click();
      await page.locator(`[data-on-click="setting"][data-value="ui.theme=${theme}"]`).click();
      await page.locator(`.app${theme === "light" ? ".light" : ":not(.light)"}`).waitFor();
      const settingsHeight = await page.locator(".settings").evaluate((el: HTMLElement) => {
        el.scrollTop = 0; return { total: el.scrollHeight, visible: el.clientHeight };
      });
      await shot("settings", theme, width);
      let part = 2;
      for (let top = settingsHeight.visible; top < settingsHeight.total; top += settingsHeight.visible) {
        await page.locator(".settings").evaluate((el: HTMLElement, top: number) => { el.scrollTop = top; }, top);
        await shot(`settings-${String(part++).padStart(2, "0")}`, theme, width);
      }
      await closeSettings();
      await page.locator('[data-on-click="select"][data-value="t43"]').first().click();
      await page.locator("#composer").waitFor();
      await shot("thread", theme, width);
      // Home/sidebar: the back button reveals the project list at narrow widths.
      if (width === 390) await page.locator('[data-on-click="select"][data-value=""]').first().click();
      await shot("home", theme, width);
      // At phone width the context menu belongs to the visible project list.
      await page.locator('[data-on-contextmenu="row-menu"][data-value="t43"]').first().evaluate((el: Element) => el.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true })));
      await shot("menu", theme, width);
      await page.locator('[data-on-click="menu-close"]').first().click({ position: { x: width - 10, y: 900 } });
      await page.locator('[data-on-click="bot"][data-value="b43"]').first().click();
      await shot("bot", theme, width);
    }
  }
  if (pageErrors.length) throw new Error(pageErrors.join("\n"));
  if (existsSync(invoked)) throw new Error("a fixture action attempted to start an agent");
  const after = readFileSync(join(home, "events.jsonl"), "utf8").trim().split("\n").map(JSON.parse);
  if (after.filter((e) => e.$ === "MessagePosted").length !== 2 || after.some((e) => e.$ === "TurnChanged" && e.state === "running"))
    throw new Error("fixture sent a message or started a turn");
  writeFileSync(join(out, `web-ja-${onlyScene ? onlyScene + "-" : ""}overflow.json`), JSON.stringify(overflow, null, 2));
  writeFileSync(join(out, `web-ja-${onlyScene ? onlyScene + "-" : ""}review.json`), JSON.stringify({ root, binary: bin, port, pageErrors, shots: overflow.length }, null, 2));
  if (overflow.some((scene: any) => scene.pageOverflow || scene.clipped.length)) throw new Error("visible UI overflow: see web-ja-overflow.json");
  console.log(JSON.stringify({ root, out, shots: overflow.length, overflow }));
} catch (error) {
  const page = browser?.contexts()[0]?.pages()[0];
  if (page) {
    await page.screenshot({ path: join(out, "web-ja-failure.png"), fullPage: true }).catch(() => {});
    writeFileSync(join(out, "web-ja-failure.html"), await page.content());
  }
  writeFileSync(join(out, "web-ja-failure.json"), JSON.stringify({ root, binary: bin, port, pageErrors, error: String(error) }, null, 2));
  throw error;
} finally {
  if (browser) await browser.close();
  hub.kill(); // Only the child started above. Never signal by name or process tree.
  await hub.exited;
}
