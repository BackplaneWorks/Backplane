import { chmod, mkdir, mkdtemp, readFile, readdir, rm, stat, symlink, utimes, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

// The binary runs Pdf.sh through the real native subprocess effect. --script
// also exercises the original command when checking this regression's control.
const argv = process.argv.slice(2);
const script = argv[0] === "--script";
const runner = resolve(script ? argv[1] : argv[0]);
if (!argv[0] || (script && !argv[1])) throw new Error("usage: bun test/tools/pdf_cache.ts BINARY | --script SCRIPT");
const root = await mkdtemp(join(tmpdir(), "backplane-pdf-cache-"));
const bin = join(root, "bin");
const home = join(root, "home");
const cache = join(root, "cache with spaces");
const path = join(root, "drawing with spaces.pdf");
const calls = join(root, "calls");
let assertions = 0;
function check(ok: boolean, name: string) {
  if (!ok) throw new Error(name);
  console.log(`ok ${name}`);
  assertions++;
}
async function shellTool(name: string, text: string) {
  const file = join(bin, name);
  await writeFile(file, `#!/bin/sh\n${text}\n`);
  await chmod(file, 0o755);
}
const env = { ...process.env, HOME: home, BACKPLANE_HOME: home, PATH: `${bin}:${process.env.PATH}`, PDF_TEST_CALLS: calls };
async function run(page = "1", extra: Record<string, string> = {}, input = path, dir = cache) {
  const cmd = script ? ["/bin/sh", runner, input, page, dir] : [runner, input, page, dir];
  const proc = Bun.spawn(cmd, { env: { ...env, ...extra }, stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text(), proc.exited]);
  return { code, out: out.trimEnd(), err };
}
async function count() { return (await readFile(calls, "utf8").catch(() => "")).length; }
function entry(out: string) { return out.split("\t")[0]; }
async function data(out: string) { return readFile(entry(out), "utf8"); }
async function cleanCache() { await rm(cache, { recursive: true, force: true }); }

try {
  await mkdir(bin);
  await mkdir(home);
  await shellTool("pdfinfo", "printf 'Pages: 3\\n'");
  await shellTool("pdftoppm", `printf x >> "$PDF_TEST_CALLS"
for last do :; done
case "\${PDF_TEST_MODE:-good}" in
  partial) printf PARTIAL > "$last.png"; exit 1 ;;
  empty) : > "$last.png"; exit 0 ;;
  fail) exit 1 ;;
esac
printf 'PNG page %s: ' "$4" > "$last.png"
for arg do case "$arg" in *.pdf) cat "$arg" >> "$last.png" ;; esac; done`);
  await writeFile(path, "first drawing\n");
  const stamp = new Date(1700000000000);
  await utimes(path, stamp, stamp);
  let result = await run();
  check(result.code === 0, "first page renders");
  check(result.out.endsWith("\t3"), "page count is retained");
  check((await data(result.out)).includes("first drawing"), "PNG contains current drawing");
  check(await count() === 1, "renderer called once");
  const initial = result.out;
  result = await run();
  check(result.code === 0 && result.out === initial, "unchanged PDF reuses cache entry");
  check(await count() === 1, "cache hit skips renderer");
  await writeFile(path, "other drawing\n"); // Same bytes and the same exact timestamp.
  await utimes(path, stamp, stamp);
  result = await run();
  check(result.code === 0, "same-second replacement renders");
  check(result.out !== initial, "same-size replacement has a new cache identity");
  check((await data(result.out)).includes("other drawing"), "replacement shows current drawing");
  check(await count() === 2, "replacement calls renderer again");
  const changed = result.out;
  result = await run("2");
  check(result.code === 0 && result.out !== changed, "pages have separate entries");
  check(await count() === 3, "second page calls renderer");
  const other = join(root, "another path.pdf");
  await writeFile(other, "other drawing\n");
  result = await run("1", {}, other);
  check(result.code === 0 && result.out !== changed, "equal contents at different paths stay distinct");
  await cleanCache();
  const beforeFailure = await count();
  result = await run("1", { PDF_TEST_MODE: "partial" });
  check(result.code !== 0 && result.out.includes("could not draw page 1"), "partial failed render reports error");
  check((await readdir(cache)).length === 0, "failed render publishes no PNG and removes its temporary directory");
  result = await run();
  check(result.code === 0 && (await data(result.out)).includes("other drawing"), "retry draws a complete PNG");
  check(await count() === beforeFailure + 2, "retry invokes renderer instead of serving its partial output");
  check((await readdir(cache)).every(name => !name.startsWith(".pdf.")), "successful render removes temporary directory");
  await cleanCache();
  result = await run("1", { PDF_TEST_MODE: "empty" });
  check(result.code !== 0, "successful renderer with empty output is rejected");
  check((await readdir(cache)).length === 0, "empty output is not published");
  result = await run();
  await writeFile(entry(result.out), "");
  const beforeEmpty = await count();
  result = await run();
  check(result.code === 0 && (await stat(entry(result.out))).size > 0, "empty old cache entry is rebuilt");
  check(await count() === beforeEmpty + 1, "empty cache entry calls renderer");
  result = await run("1", {}, join(root, "missing.pdf"));
  check(result.code !== 0 && result.out.includes("no such file"), "missing PDF reports error");
  const blocked = join(root, "not a directory");
  await writeFile(blocked, "blocked");
  result = await run("1", {}, path, blocked);
  check(result.code !== 0, "cache directory failure reports error");
  await shellTool("cksum", "echo checksum-read-failed >&2; exit 1");
  result = await run();
  check(result.code !== 0 && result.out.includes("could not read PDF"), "content read/checksum failure reports error");
  await rm(join(bin, "cksum"));
  const noTools = join(root, "no-poppler");
  await mkdir(noTools);
  await symlink("/bin/sh", join(noTools, "sh"));
  result = await run("1", { PATH: noTools });
  check(result.code === 3 && result.out.includes("nopoppler"), "missing Poppler retains setup diagnostic");
  if (argv.includes("--poppler")) {
    const realEnv = { PATH: process.env.PATH! };
    check(!!Bun.which("pdftoppm") && !!Bun.which("pdfinfo"), "real Poppler tools are available");
    function pdf(width: string) {
      const stream = `0 0 ${width} 100 re f\n`;
      const objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 1000 1000] /Contents 4 0 R >>",
        `<< /Length ${stream.length} >>\nstream\n${stream}endstream`,
      ];
      let text = "%PDF-1.4\n";
      const offsets = [0];
      for (const [i, object] of objects.entries()) {
        offsets.push(text.length);
        text += `${i + 1} 0 obj\n${object}\nendobj\n`;
      }
      const xref = text.length;
      text += `xref\n0 ${offsets.length}\n0000000000 65535 f \n`;
      text += offsets.slice(1).map(n => `${n.toString().padStart(10, "0")} 00000 n \n`).join("");
      return text + `trailer\n<< /Size ${offsets.length} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
    }
    await cleanCache();
    await writeFile(path, pdf("100"));
    await utimes(path, stamp, stamp);
    const first = await run("1", realEnv);
    check(first.code === 0 && first.out.endsWith("\t1"), "real Poppler draws page and reports its count");
    const png = await readFile(entry(first.out));
    check(png.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])), "real output is a PNG");
    const size = (await stat(path)).size;
    await writeFile(path, pdf("800"));
    await utimes(path, stamp, stamp);
    check((await stat(path)).size === size, "real replacement preserves the PDF byte count");
    const second = await run("1", realEnv);
    check(second.code === 0 && second.out !== first.out, "real same-second replacement gets a new page");
    check(!(await readFile(entry(second.out))).equals(png), "real same-second replacement changes the rendered image");
    const repeat = await run("1", realEnv);
    check(repeat.code === 0 && repeat.out === second.out, "real unchanged PDF reuses its new cache entry");
  }
  console.log(`${assertions} PDF cache assertions passed`);
} finally {
  await rm(root, { recursive: true, force: true });
}
