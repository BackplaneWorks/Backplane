// The compiled hub's stack (scripts/js-stack-prelude.js), Linux: started
// with an 8 MiB stack limit, the hub runs with a 4 GiB soft limit and
// BUN_JSC_maxPerThreadStackUsage set (it executed itself once, same pid),
// its children start with 8 MiB again (host.js's host_child_stack: a
// stand-in `claude --version` the hub probes at start writes its own
// `ulimit -s`), and the command line still answers --version (0) and an
// unknown flag (2). An empty home in a fresh directory, a free port.
//
//   bun test/tools/js_stack_test.ts dist-js/linux-x64/backplane-serve
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const bin = resolve(process.argv[2] ?? "dist-js/linux-x64/backplane-serve");
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string, more: unknown = "") => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}${ok || more === "" ? "" : ": " + JSON.stringify(more).slice(0, 300)}`);
  if (!ok) fail++;
};
// run with an 8 MiB soft stack limit, as a login session gives
const small = (args: string[], extra: Record<string, string> = {}) => Bun.spawn(["sh", "-c", 'ulimit -S -s 8192; exec "$@"', "sh", bin, ...args], {
  env: (() => { const e: Record<string, string | undefined> = { ...process.env, ...extra }; delete e.BUN_JSC_maxPerThreadStackUsage; delete e.BACKPLANE_STACK_SET; return e; })(),
  stdout: "pipe", stderr: "pipe",
});

if (process.platform !== "linux") {
  console.log("js_stack: Linux only (/proc), skipped");
  process.exit(0);
}
const root = mkdtempSync(join(tmpdir(), "bp-stack-"));
let hub: ReturnType<typeof small> | null = null;
try {
  const v = small(["--version"]);
  const vout = await new Response(v.stdout).text();
  check((await v.exited) === 0 && vout.startsWith("backplane "), "--version answers 0 after the re-exec", vout.trim());
  const u = small(["--no-such-flag"]);
  check((await u.exited) === 2, "an unknown flag still exits 2");

  const fake = join(root, "bin");
  mkdirSync(fake);
  writeFileSync(join(fake, "claude"), `#!/bin/sh\nulimit -s > ${join(root, "claude.stack")}\necho 1.0.0\n`);
  chmodSync(join(fake, "claude"), 0o755);
  for (const n of ["codex", "grok", "muse"]) {
    writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
    chmodSync(join(fake, n), 0o755);
  }
  const port = (() => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; })();
  hub = small(["--home", join(root, "home"), "--port", String(port), "--no-tailscale", "--foreground"],
    { BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: "1", BACKPLANE_TELEMETRY: "off", DISPLAY: "", WAYLAND_DISPLAY: "" });
  let up = false;
  for (let i = 0; i < 150 && !up; i++) {
    try { up = (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch {}
    if (!up) await sleep(100);
  }
  check(up, "the hub serves /hello");
  const limits = readFileSync(`/proc/${hub.pid}/limits`, "utf8");
  const [, soft, hard] = /Max stack size\s+(\S+)\s+(\S+)/.exec(limits) ?? [];
  // 4 GiB, or the hard limit where that is less (Boat: 1 GiB)
  const goal = hard === "unlimited" || Number(hard) >= 4294967296 ? "4294967296" : hard;
  check(soft === goal, "the hub runs with a 4 GiB soft stack limit, or its hard limit (same pid)", `${soft} of ${hard}`);
  const env = readFileSync(`/proc/${hub.pid}/environ`, "utf8").split("\0");
  check(env.includes("BUN_JSC_maxPerThreadStackUsage=4000000000"), "and with Bun's JS stack bound set");
  const stackFile = join(root, "claude.stack");
  for (let i = 0; i < 150 && !existsSync(stackFile); i++) await sleep(100);
  const child = existsSync(stackFile) ? readFileSync(stackFile, "utf8").trim() : "";
  check(child === "8192", "a child of the hub starts with 8 MiB", child);
} catch (e) {
  check(false, "run", String(e));
} finally {
  if (hub !== null) {
    hub.kill();
    await hub.exited;
  }
  rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `js_stack: ${fail} failed` : "js_stack: ok");
process.exit(fail ? 1 : 0);
