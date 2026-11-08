// Finding Chrome to drive. Order: --chromium / $BACKPLANE_CHROME
// ($BACKPLANE_CHROMIUM too), an installed Google Chrome, then Google's
// Chrome for Testing build in Backplane's cache, which installChrome()
// downloads (no root needed) when there is no Chrome at all.

import { existsSync, mkdirSync, readdirSync, renameSync, rmSync } from "node:fs";
import { homedir, platform, arch } from "node:os";
import { delimiter, join } from "node:path";

const SYSTEM_NAMES = ["google-chrome-stable", "google-chrome", "chrome"];

const SYSTEM_PATHS = [
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/opt/google/chrome/chrome",
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
];

function onPath(name: string): string | null {
  for (const dir of (process.env.PATH ?? "").split(delimiter)) {
    if (!dir) continue;
    const p = join(dir, name);
    if (existsSync(p)) return p;
  }
  return null;
}

// Chrome for Testing: platform names and the executable inside its zip
function cftPlatform(): string | null {
  const p = platform(), a = arch();
  if (p === "linux" && a === "x64") return "linux64";
  if (p === "darwin") return a === "arm64" ? "mac-arm64" : "mac-x64";
  if (p === "win32") return a === "ia32" ? "win32" : "win64";
  return null;
}

function cftBinary(plat: string): string {
  if (plat.startsWith("mac")) return `chrome-${plat}/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing`;
  if (plat.startsWith("win")) return `chrome-${plat}/chrome.exe`;
  return `chrome-${plat}/chrome`;
}

function cacheRoot(): string {
  if (platform() === "darwin") return join(homedir(), "Library", "Caches", "backplane", "chrome");
  if (platform() === "win32") return join(process.env.LOCALAPPDATA ?? homedir(), "backplane", "chrome");
  return join(process.env.XDG_CACHE_HOME ?? join(homedir(), ".cache"), "backplane", "chrome");
}

// the newest Chrome for Testing already in the cache
function fromCache(): string | null {
  const plat = cftPlatform();
  if (!plat) return null;
  let versions: string[];
  try {
    versions = readdirSync(cacheRoot());
  } catch {
    return null;
  }
  versions.sort((x, y) => y.localeCompare(x, undefined, { numeric: true }));
  for (const v of versions) {
    const p = join(cacheRoot(), v, cftBinary(plat));
    if (existsSync(p)) return p;
  }
  return null;
}

// download the current Stable Chrome for Testing into the cache
export async function installChrome(log: (s: string) => void): Promise<string> {
  const plat = cftPlatform();
  if (!plat) throw new Error(`no Chrome build for ${platform()}-${arch()}: install Google Chrome`);
  const index = await fetch("https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json");
  if (!index.ok) throw new Error(`Chrome download index: HTTP ${index.status}`);
  const stable = (await index.json()).channels.Stable;
  const dl = stable.downloads.chrome.find((d: any) => d.platform === plat);
  if (!dl) throw new Error(`no Chrome for Testing download for ${plat}`);
  const dir = join(cacheRoot(), stable.version);
  const tmp = dir + ".part";
  rmSync(tmp, { recursive: true, force: true });
  mkdirSync(tmp, { recursive: true });
  log(`downloading Chrome ${stable.version} (${plat})`);
  const zip = await fetch(dl.url);
  if (!zip.ok) throw new Error(`Chrome download: HTTP ${zip.status}`);
  const file = join(tmp, "chrome.zip");
  await Bun.write(file, zip);
  const unzip = Bun.spawnSync(unzipCommand(file, tmp));
  if (unzip.exitCode !== 0) throw new Error(`could not unpack Chrome: ${unzip.stderr.toString().trim()}`);
  rmSync(file, { force: true });
  rmSync(dir, { recursive: true, force: true });
  renameSync(tmp, dir);
  const exe = join(dir, cftBinary(plat));
  if (!existsSync(exe)) throw new Error("Chrome unpacked without its executable");
  return exe;
}

/** How a zip is unpacked here. Windows: its own tar.exe by its full path (bsdtar reads zips and C:\ paths); a `tar` found on PATH may be
 *  Git's GNU tar, which takes "C:" for a remote host ("Cannot connect to C: resolve failed"), and Git's usr\bin is on PATH for agents. */
export function unzipCommand(file: string, into: string, os: string = platform(), root: string = process.env.SystemRoot || "C:\\Windows"): string[] {
  if (os === "win32") return [`${root}\\System32\\tar.exe`, "-xf", file, "-C", into];
  return ["unzip", "-q", file, "-d", into];
}

export function findChromium(): string | null {
  for (const env of [process.env.BACKPLANE_CHROME, process.env.BACKPLANE_CHROMIUM]) {
    if (env && existsSync(env)) return env;
  }
  for (const n of SYSTEM_NAMES) {
    const p = onPath(n);
    if (p) return p;
  }
  for (const p of SYSTEM_PATHS) if (existsSync(p)) return p;
  return fromCache();
}

export const INSTALL_HINT =
  "no Chrome found and it could not be downloaded: install Google Chrome, or set BACKPLANE_CHROME";
