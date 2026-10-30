// Exercise the real shell helper against a provider of lost heartbeats.
// The command must finish itself, preserving its captured identity and status.
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
const root = mkdtempSync(join(tmpdir(), "bp-helper-liveness-"));
let holder = "", renews = 0;
const server = Bun.serve({ hostname: "127.0.0.1", port: 0, async fetch(req) {
  const body = await req.json() as any;
  holder = body.holder;
  const op = new URL(req.url).pathname;
  if (op.endsWith("/renew")) renews++;
  return Response.json({ held: op.endsWith("/acquire"), position: 0 });
}});
let nonce = "";
try {
  const p = Bun.spawn([resolve("scripts/bp-lock.sh"), "helper-test", "--", "sh", "-c", "sleep 32; printf completed"], {
    env: { ...process.env, BACKPLANE_LOCK_URL: `http://127.0.0.1:${server.port}`, BACKPLANE_LOCK_DIR: root }, stdout: "pipe", stderr: "pipe"
  });
  const limit = Date.now() + 5000;
  while (!holder && Date.now() < limit) await Bun.sleep(25);
  nonce = join("/tmp", holder.split(":").at(-1) ?? "invalid");
  const identityDeadline = Date.now() + 5000;
  while (!existsSync(join(nonce, "cmd.id")) && Date.now() < identityDeadline) await Bun.sleep(25);
  if (!existsSync(join(nonce, "cmd.pid")) || !readFileSync(join(nonce, "cmd.id"), "utf8").trim()) throw new Error("captured command identity missing");
  let timer: ReturnType<typeof setTimeout>;
  const deadline = new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error("helper did not finish within the fixture deadline")), 45000); });
  const code = await Promise.race([p.exited, deadline]).finally(() => clearTimeout(timer!));
  const [out, err] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text()]);
  if (code !== 0 || out !== "completed") throw new Error(`busy command was interrupted: exit ${code}, ${out}`);
  if (!renews || !err.includes("work continues")) throw new Error(`missing heartbeat warning: ${renews}, ${err}`);
  console.log("ok missing lease heartbeat warns without interrupting busy command");
  console.log("ok command self-registers captured PID and start identity before exec");
} finally {
  server.stop(true);
  rmSync(root, { recursive: true, force: true });
  if (/\/bp-lock-client\.[A-Za-z0-9]+$/.test(nonce)) rmSync(nonce, { recursive: true, force: true });
}
