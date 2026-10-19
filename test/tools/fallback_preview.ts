// Isolated UI fixture hub. Every provider CLI is a stand-in; no user state.
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/fallback-server", web = "build/fallback-web", ready = "build/fallback-preview-ready.json"] = process.argv.slice(2);
const root = mkdtempSync(join(tmpdir(), "bp-fallback-preview-"));
const home = join(root, "home"), fake = join(root, "bin");
for (const p of [home, fake, join(root, ".codex")]) mkdirSync(p);
copyFileSync("test/fixtures/fallback-settings/dark.jsonl", join(home, "events.jsonl"));
const models = JSON.parse(JSON.parse(readFileSync("test/fixtures/fallback-settings/info.jsonl", "utf8")).info.models);
writeFileSync(join(home, "models.cache"), JSON.stringify(models) + "\nFixture providers\n");
writeFileSync(join(root, ".codex/models_cache.json"), JSON.stringify({ models: [{ slug: "gpt-6.1-sol" }] }));
for (const cli of ["claude", "codex", "grok", "muse", "gh", "tailscale", "kicad-cli", "freecad"]) {
  writeFileSync(join(fake, cli), "#!/bin/sh\nexit 127\n"); chmodSync(join(fake, cli), 0o755);
}
const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
const port = listener.port; listener.stop(true);
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--web", resolve(web), "--headless", "--foreground", "--no-tailscale"], {
  env: { ...process.env, HOME: root, PATH: `${fake}:${process.env.PATH}`, BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_LOCK_DIR: join(root, "locks") },
  stdout: Bun.file(join(root, "stdout")), stderr: Bun.file(join(root, "stderr")),
});
try {
  let up = false;
  for (let i = 0; i < 100; i++) {
    try { up = (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch {}
    if (up) break;
    await Bun.sleep(50);
  }
  if (!up) throw new Error("isolated preview hub did not start");
  writeFileSync(ready, JSON.stringify({ url: `http://127.0.0.1:${port}/`, root, pid: proc.pid }));
  console.log(`Fixture UI ready at http://127.0.0.1:${port}/`);
  await new Promise<void>(resolve => process.once("SIGTERM", resolve));
} finally {
  if (proc.exitCode === null) proc.kill("SIGTERM");
  await proc.exited;
}
