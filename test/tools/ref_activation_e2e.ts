// Actual markdown nodes -> host DOM patch -> delegated browser click -> App.act.
// Also drives native link geometry -> Hit.find -> Row.press -> shared selection.
// Build: bend test/ref_activation/index.html -o build/ref-activation
// Run: bun test/tools/ref_activation_e2e.ts build/ref-activation
import { readFileSync, existsSync } from "node:fs";
import { resolve, extname } from "node:path";
const { chromium } = await import(process.env.REF_PLAYWRIGHT_MODULE || "../../tools/browser/node_modules/playwright-core/index.mjs");
const bundle = resolve(process.argv[2] || "build/ref-activation");
const host = readFileSync("src/web/host.js", "utf8");
function section(start: string, end: string) {
  const a = host.indexOf(start), b = host.indexOf(end, a);
  if (a < 0 || b < 0) throw new Error(`Missing production section: ${start}`);
  return host.slice(a, b);
}
const dom = section("function* each(list)", "// Timing, for a bench only");
const value = section("function valueOf(el)", "\n}") + "\n}";
const events = section("for (const ev of EVENTS)", "// A page the person drives");
const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch(req) {
  const path = resolve(bundle, "." + new URL(req.url).pathname.replace(/\/$/, "/index.html"));
  if (!path.startsWith(bundle + "/") || !existsSync(path)) return new Response("missing", { status: 404 });
  return new Response(Bun.file(path), { headers: { "Content-Type": extname(path) === ".js" ? "text/javascript" : "text/html" } });
} });
const browser = await chromium.launch({ executablePath: process.env.CHROME || "/usr/bin/google-chrome", headless: true, args: ["--no-sandbox"] });
let failures = 0;
function check(name: string, ok: boolean, got: unknown) {
  console.log(`${ok ? "ok" : "FAIL"} ${name}: ${JSON.stringify(got)}`);
  if (!ok) failures++;
}
try {
  const page = await browser.newPage();
  const errors: string[] = [];
  page.on("pageerror", (e: Error) => errors.push(e.message));
  await page.goto(`http://127.0.0.1:${server.port}/`);
  await page.waitForFunction(() => (globalThis as any).RefActivation);
  await page.evaluate(({ dom, value, events }: any) => {
    const F = (globalThis as any).RefActivation;
    const is = (x: any, name: string) => x.$.split(".").at(-1) === name;
    const witness = F.render("[witness](backplane://thread/t1792085069-132986)");
    let el = witness.head;
    if (el.tag === "p") el = el.kids.head;
    const T = { El: el.$ };
    const api = new Function("App", "T", "is", "dispatch", `${dom}\n${value}\n${events}\nreturn {create, each};`)(F, T, is, (a: string, v: string) => {
      (globalThis as any).calls.push([a, v]);
      (globalThis as any).current = F.activate((globalThis as any).current, a, v);
    });
    (globalThis as any).setup = (text: string) => {
      (globalThis as any).calls = [];
      (globalThis as any).current = F.fresh();
      const root = document.getElementById("root")!;
      root.replaceChildren();
      for (const node of api.each(F.render(text))) root.append(api.create(node));
    };
  }, { dom, value, events });
  const target = "t1792085069-132986";
  const cases = [
    ["canonical URI", `backplane://thread/${target}`, target],
    ["legacy URI", `backplane://open?thread=${target}`, target],
    ["canonical markdown", `[Raul](backplane://thread/${target})`, target],
    ["legacy markdown", `[Raul](backplane://open?thread=${target})`, target],
    ["encoded remote markdown", `[Other machine](backplane://thread/peer%7E${target})`, `peer~${target}`],
    ["remote legacy", `backplane://open?thread=peer~${target}`, `peer~${target}`],
    ["unresolved explicit", "backplane://thread/missing", "t1-1"],
  ];
  for (const [name, text, expected] of cases) {
    await page.evaluate((text: string) => (globalThis as any).setup(text), text);
    const link = page.locator("a.ref");
    check(`${name} rendered anchor`, await link.count() === 1, await link.count());
    if (await link.count() !== 1) continue;
    const before = page.url();
    await link.click();
    const got = await page.evaluate(() => ({ selected: (globalThis as any).RefActivation.selected((globalThis as any).current), calls: (globalThis as any).calls }));
    check(`${name} actual delegated click selection`, got.selected === expected && got.calls.length === 1 && got.calls[0][0] === "ref-go", got);
    check(`${name} in-place URL`, page.url() === before, page.url());
    const native = await page.evaluate((text: string) => ({ hit: (globalThis as any).RefActivation.native_hit(text), selected: (globalThis as any).RefActivation.native_tap(text) }), text);
    check(`${name} native rendered hit and press`, native.hit.startsWith("ref-go backplane://thread/") && native.selected === expected, native);
  }
  await page.evaluate((text: string) => (globalThis as any).setup(text), target);
  check("bare id remains noninteractive", await page.locator("a").count() === 0, await page.locator("a").count());
  check("no browser errors", errors.length === 0, errors);
} finally {
  await browser.close();
  server.stop(true);
}
if (failures) process.exit(1);
