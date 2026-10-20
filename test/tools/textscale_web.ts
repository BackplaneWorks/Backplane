// Production CSS typography regression, no hub or installed app. This is a
// browser component check; it makes no physical-phone or FPS claim.
// bun test/tools/textscale_web.ts --pw /path/to/playwright-core [--chrome PATH]
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const args = process.argv.slice(2);
const option = (name: string, fallback = "") => {
  const at = args.indexOf(`--${name}`);
  return at >= 0 ? args[at + 1] : fallback;
};
const pw = option("pw");
if (!pw) throw new Error("Pass an existing playwright-core path with --pw");
const { chromium } = await import(pw);
const out = resolve(option("out", "build/text-scaling/web"));
mkdirSync(out, { recursive: true });
const css = readFileSync(resolve("src/web/style.css"), "utf8");
const browser = await chromium.launch({ headless: true,
  executablePath: option("chrome") || undefined, args: ["--disable-gpu"] });
const results: unknown[] = [];
const check = (test: boolean, message: string) => { if (!test) throw new Error(message); };
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 900 }, deviceScaleFactor: 1 });
  await page.setContent(`<!doctype html><meta name="viewport" content="width=device-width, initial-scale=1"><style>${css}</style>
    <div id="root"><div class="app picked" style="--text-scale:100">
      <aside class="sidebar"><div class="brand">Backplane</div><div class="thread">Text size review</div></aside>
      <main class="thread-view"><div class="thread-head"><span class="title">Text size and overflow</span></div>
      <div class="timeline"><div class="msg"><div class="body"><p id="paragraph">The text uses real browser font metrics. Increasing the font at the same viewport width must reflow this paragraph while preserving selection and click targets.</p><p id="token">averylongunbrokentokenthatmustwrapwithnothingleftoutsidetheconversationpanel012345678901234567890123456789</p><table><thead><tr><th>Setting</th><th>Behavior</th></tr></thead><tbody><tr><td>Text size</td><td>Persistent and bounded</td></tr></tbody></table></div></div></div>
      <div class="composer"><textarea aria-label="Message">First line. Second line holds a selection. More text reflows when the window resizes.</textarea><div class="composer-foot"><button id="send">Send message</button></div>
      <div class="model-menu" style="display:none"><button class="row" id="model">A longer model name for font reflow</button></div></div>
      </main></div></div>`);
  await page.evaluate(() => {
    document.querySelector("#send")!.addEventListener("click", () => { document.body.dataset.sent = "yes"; });
    document.querySelector("#model")!.addEventListener("click", () => { document.body.dataset.model = "selected"; });
  });
  await page.evaluate(() => document.fonts.ready);
  let normalFont = 0;
  for (const width of [1200, 390]) {
    await page.setViewportSize({ width, height: 900 });
    for (const scale of [100, 150]) {
      await page.evaluate((value: number) => {
        (document.querySelector(".app") as HTMLElement).style.setProperty("--text-scale", String(value));
      }, scale);
      const metrics = await page.evaluate(() => {
        const p = document.querySelector("#paragraph")!;
        const a = document.querySelector("textarea") as HTMLTextAreaElement;
        a.focus(); a.setSelectionRange(12, 27);
        const token = document.querySelector("#token")!;
        return { font: parseFloat(getComputedStyle(p).fontSize), height: p.getBoundingClientRect().height,
          viewport: innerWidth, dpi: devicePixelRatio, overflow: document.documentElement.scrollWidth,
          tokenWidth: token.getBoundingClientRect().width, tokenScroll: token.scrollWidth,
          selected: a.value.slice(a.selectionStart, a.selectionEnd) };
      });
      if (width === 1200 && scale === 100) normalFont = metrics.font;
      check(Math.abs(metrics.font - normalFont * scale / 100) < 0.1, `font applied twice or missing: ${width}/${scale}`);
      check(metrics.viewport === width && metrics.dpi === 1, "text size changed viewport or device scale");
      check(metrics.overflow <= width + 1, `horizontal viewport overflow at ${width}/${scale}`);
      check(metrics.tokenScroll <= metrics.tokenWidth + 1, `unbroken token overflow at ${width}/${scale}`);
      check(metrics.selected === "Second line hol", "textarea selection changed with text size");
      await page.locator("#send").click();
      check(await page.evaluate(() => document.body.dataset.sent === "yes"), "button hit did not match drawn control");
      await page.screenshot({ path: `${out}/text-${width}-${scale}.png` });
      await page.locator(".model-menu").evaluate((e: HTMLElement) => { e.style.display = "flex"; });
      await page.locator("#model").click();
      check(await page.evaluate(() => document.body.dataset.model === "selected"), "menu hit did not match drawn control");
      await page.locator(".model-menu").evaluate((e: HTMLElement) => { e.style.display = "none"; });
      results.push({ width, scale, ...metrics });
    }
  }
  writeFileSync(`${out}/results.json`, JSON.stringify(results, null, 2) + "\n");
  console.log("PASS web text size, bounds, reflow, selection, click/menu targets at 100/150 and desktop/mobile browser widths");
} finally {
  await browser.close();
}
