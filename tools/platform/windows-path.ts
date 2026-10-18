// Paths passed by the Cygwin hub must be native paths in Windows Bun helpers.
// Convert filesystem arguments only; protocol text and model output stay as-is.
import { execFileSync } from "node:child_process";

const mapped = new Map<string, string>();
function cygpath(flag: string, path: string): string {
  const key = flag + "\0" + path;
  const kept = mapped.get(key);
  if (kept !== undefined) return kept;
  const value = execFileSync("cygpath.exe", [flag, "--", path], {
    encoding: "utf8", timeout: 10000, windowsHide: true,
  }).trimEnd();
  if (!value) throw new Error(`Cannot convert Cygwin path: ${path}`);
  mapped.set(key, value);
  return value;
}

export function nativePath(path: string): string {
  return process.platform === "win32" && path.startsWith("/") ? cygpath("-w", path) : path;
}

export function hubPath(path: string): string {
  return process.platform === "win32" && /^[a-z]:[\\/]/i.test(path) ? cygpath("-u", path) : path;
}
