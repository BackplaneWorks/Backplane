// Runtime discovery and hash failure are independent of any browser/display.
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { findCef, installCef } from "../src/cef.ts";
import { check } from "../../cef/test/harness.ts";
const root = mkdtempSync(join(tmpdir(), "bp-cef-runtime-test-"));
const priorCache = process.env.XDG_CACHE_HOME, priorHost = process.env.BACKPLANE_CEF;
process.env.XDG_CACHE_HOME = root;
delete process.env.BACKPLANE_CEF;
try {
  check("empty runtime URL errors clearly", await installCef().then(() => false, (e) => e.message === "no CEF runtime: set BACKPLANE_CEF"));
  const cached = join(root, "backplane/cef-runtime/154.0.34/backplane-cef");
  mkdirSync(join(cached, ".."), { recursive: true }); writeFileSync(cached, "fixture");
  check("find cache runtime", findCef() === cached);
  const envHost = join(root, "env-host"), argHost = join(root, "arg-host");
  writeFileSync(envHost, "fixture"); writeFileSync(argHost, "fixture");
  process.env.BACKPLANE_CEF = envHost;
  check("environment overrides cache", findCef() === envHost);
  check("explicit path overrides environment", findCef(argHost) === argHost);
  const archiveRoot = join(root, "archive"); mkdirSync(archiveRoot);
  writeFileSync(join(archiveRoot, "backplane-cef"), "runtime fixture", { mode: 0o755 });
  const archive = join(root, "runtime.tar.zst");
  const tar = Bun.spawn(["tar", "-I", "zstd -T1", "-cf", archive, "-C", archiveRoot, "."], { stdout: "ignore", stderr: "inherit" });
  if (await tar.exited) throw new Error("fixture packaging failed");
  const bytes = await Bun.file(archive).arrayBuffer();
  const hash = new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
  const server = Bun.serve({ port: 0, hostname: "127.0.0.1", fetch(req) { return new URL(req.url).pathname === "/valid" ? new Response(bytes) : new Response("tampered archive"); } });
  try {
    check("SHA mismatch rejects before unpack", await installCef(`http://127.0.0.1:${server.port}/runtime.tar.zst`, "0".repeat(64)).then(() => false, (e) => e.message === "CEF download: SHA-256 mismatch"));
    const installed = await installCef(`http://127.0.0.1:${server.port}/valid`, hash);
    check("verified archive installs into version cache", installed === cached && await Bun.file(installed).text() === "runtime fixture");
  } finally { server.stop(true); }
  console.log("all CEF runtime checks passed");
} finally {
  if (priorCache === undefined) delete process.env.XDG_CACHE_HOME; else process.env.XDG_CACHE_HOME = priorCache;
  if (priorHost === undefined) delete process.env.BACKPLANE_CEF; else process.env.BACKPLANE_CEF = priorHost;
  rmSync(root, { recursive: true, force: true });
}
