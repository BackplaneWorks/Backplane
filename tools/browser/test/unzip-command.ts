// The Chrome download is unpacked by Windows' own tar.exe by its full path, never a `tar` from PATH (Git's GNU tar reads "C:" as a host):
// bun test/unzip-command.ts. On Windows it also unpacks a real zip from a C:\ path with Git's usr\bin first on PATH, as the hub's children have it.
import { mkdtempSync, writeFileSync, readFileSync, existsSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { unzipCommand } from "../src/chromium.ts";
let bad = 0;
const check = (name: string, ok: boolean, ev: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}: ${ev}`); if (!ok) bad++; };
const w = unzipCommand("C:\\x\\chrome.zip", "C:\\x\\out", "win32", "C:\\Windows");
check("Windows names System32's tar.exe", w[0] === "C:\\Windows\\System32\\tar.exe" && w.slice(1).join(" ") === "-xf C:\\x\\chrome.zip -C C:\\x\\out", w.join(" "));
const l = unzipCommand("/x/chrome.zip", "/x/out", "linux");
check("elsewhere unzip", l.join(" ") === "unzip -q /x/chrome.zip -d /x/out", l.join(" "));
if (process.platform === "win32") {
  const d = mkdtempSync(join(tmpdir(), "bp-unzip-")); const src = join(d, "src"), out = join(d, "out");
  Bun.spawnSync(["cmd", "/c", "mkdir", src, out]); writeFileSync(join(src, "chrome.exe"), "not chrome\n");
  const tar = `${process.env.SystemRoot}\\System32\\tar.exe`;
  const mk = Bun.spawnSync([tar, "-a", "-c", "-f", join(d, "chrome.zip"), "-C", src, "chrome.exe"]);
  const git = ["C:\\Program Files\\Git\\usr\\bin", "C:\\Program Files (x86)\\Git\\usr\\bin"].find(existsSync);
  const env = { ...process.env, PATH: `${git ?? ""};${process.env.PATH}` };
  const un = Bun.spawnSync(unzipCommand(join(d, "chrome.zip"), out), { env });
  const got = existsSync(join(out, "chrome.exe")) ? readFileSync(join(out, "chrome.exe"), "utf8") : "";
  check("a real zip from a C:\\ path unpacks with Git's usr\\bin first on PATH", mk.exitCode === 0 && un.exitCode === 0 && got === "not chrome\n",
    `zip made ${mk.exitCode}; unpack ${un.exitCode} ${un.stderr.toString().trim()}; Git usr\\bin ${git ?? "not installed"}; file ${JSON.stringify(got)}`);
  rmSync(d, { recursive: true, force: true });
}
process.exit(bad ? 1 : 0);
