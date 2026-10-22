// Real Base file effects: open, size and read failures stay distinct from
// successful empty files. All paths are generated in a temporary folder.
import { chmodSync, closeSync, ftruncateSync, mkdirSync, mkdtempSync, openSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const binary = resolve(process.argv[2] ?? "build/file-bytes");
const base = mkdtempSync(join(tmpdir(), "bp-file-bytes-"));
let checks = 0;
function check(path: string, want: string, label: string) {
  const r = Bun.spawnSync([binary, path], { stdout: "pipe", stderr: "pipe", timeout: 15_000 });
  if (r.exitCode !== 0 || r.stdout.toString().trim() !== want) throw new Error(`FAIL ${label}: exit ${r.exitCode}, ${r.stdout.toString()}, ${r.stderr.toString()}`);
  console.log(`ok ${label}`); checks++;
}
try {
  if (process.getuid?.() === 0) throw new Error("run as an ordinary user so chmod 000 exercises a real read failure");
  const empty = join(base, "empty"), binaryFile = join(base, "binary"), denied = join(base, "denied"), directory = join(base, "directory"), huge = join(base, "huge");
  writeFileSync(empty, ""); writeFileSync(binaryFile, new Uint8Array([0, 10, 128, 255]));
  writeFileSync(denied, "not empty"); chmodSync(denied, 0); mkdirSync(directory);
  const fd = openSync(huge, "w"); ftruncateSync(fd, 4294967296); closeSync(fd);
  check(empty, "ok:", "empty file returns successful empty bytes");
  check(binaryFile, "ok:0,10,128,255", "binary file preserves bytes");
  check(join(base, "missing"), "error", "missing file preserves open failure");
  check(denied, "error", "unreadable file preserves open failure");
  check(directory, "error", "directory preserves read failure");
  check(huge, "error", "overflowing file size preserves size failure");
  console.log(`${checks} checked file assertions passed`);
} finally { rmSync(base, { recursive: true, force: true }); }
