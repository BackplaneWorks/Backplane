import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { Readable } from "node:stream";

export async function until<T>(fn: () => T | Promise<T>, what: string, ms = 15000): Promise<NonNullable<T>> {
  const end = Date.now() + ms;
  while (Date.now() < end) { const v = await fn(); if (v) return v as NonNullable<T>; await Bun.sleep(25); }
  throw new Error(`timeout: ${what}`);
}
export function check(what: string, ok: unknown, detail?: unknown): asserts ok {
  if (!ok) throw new Error(`${what}: ${JSON.stringify(detail)}`);
  console.log(`ok  ${what}`);
}
export function jsonProcess(cmd: string[], env = process.env) {
  const proc = Bun.spawn(cmd, { env, stdin: "pipe", stdout: "pipe", stderr: "inherit" });
  const events: any[] = [], lines: any[] = [];
  let id = 0;
  const waits = new Map<number, (v: any) => void>();
  void (async () => {
    const rl = createInterface({ input: Readable.fromWeb(proc.stdout as any), crlfDelay: Infinity });
    for await (const line of rl) {
      const v = JSON.parse(line); lines.push(v);
      if (v.event) events.push(v); else if (v.id !== undefined) { waits.get(v.id)?.(v); waits.delete(v.id); }
    }
  })();
  const write = (line: string) => { proc.stdin.write(line + "\n"); proc.stdin.flush(); };
  async function call(op: string, args = {}): Promise<any> {
    const i = ++id;
    const p = new Promise<any>((resolve, reject) => {
      const timer = setTimeout(() => { waits.delete(i); reject(new Error(`timeout ${op}`)); }, 35000);
      waits.set(i, (v) => { clearTimeout(timer); resolve(v); });
    });
    write(JSON.stringify({ id: i, op, ...args }));
    const r = await p;
    if (!r.ok) throw new Error(`${op}: ${r.error}`);
    return r.result;
  }
  return { proc, events, lines, write, call };
}
export async function privateDisplay() {
  // CEF_DISPLAYS: the private displays to choose from (never 0)
  const choices = (process.env.CEF_DISPLAYS ?? "96 97 98 99").split(/\s+/).filter(Boolean).map(Number).filter((n) => n > 0);
  const display = choices.find((n) => !existsSync(`/tmp/.X11-unix/X${n}`) && !existsSync(`/tmp/.X${n}-lock`));
  if (!display) throw new Error(`no free private Xvfb display (${choices.join(" ")})`);
  const root = mkdtempSync(join(tmpdir(), "bp-cef-test-"));
  const env = { ...process.env, DISPLAY: `:${display}` };
  const xvfbPath = process.env.XVFB ?? (Bun.which("Xvfb") || join(homedir(), ".local/xvfb-root/usr/bin/Xvfb"));
  const xvfb = Bun.spawn([xvfbPath, env.DISPLAY, "-screen", "0", "1600x1000x24", "-nolisten", "tcp"], { stdout: "ignore", stderr: "inherit" });
  let parent: ReturnType<typeof jsonProcess> | null = null;
  let native: ReturnType<typeof jsonProcess> | null = null;
  let nativeXid = 0;
  async function cleanup() {
    if (native && native.proc.exitCode === null && parent) {
      parent.write(`close ${nativeXid}`); await native.proc.exited;
    }
    if (parent) { parent.proc.stdin.end(); await parent.proc.exited; }
    // The only signalled process is this private Xvfb PID, created above.
    if (xvfb.exitCode === null) xvfb.kill("SIGTERM");
    await xvfb.exited;
    rmSync(root, { recursive: true, force: true });
  }
  try {
    await until(() => {
      if (xvfb.exitCode !== null) throw new Error(`our Xvfb exited (${xvfb.exitCode})`);
      try { return Number(readFileSync(`/tmp/.X${display}-lock`, "utf8").trim()) === xvfb.pid && existsSync(`/tmp/.X11-unix/X${display}`); }
      catch { return false; }
    }, "our Xvfb socket and PID lock");
    const exe = join(root, "x11");
    const cc = Bun.spawn(["cc", new URL("./x11.c", import.meta.url).pathname, "-o", exe, "-lX11", "-ldl"], { stdout: "inherit", stderr: "inherit" });
    if (await cc.exited) throw new Error("could not compile X11 test parent");
    parent = jsonProcess([exe], env);
    const p = await until(() => parent!.lines.find((v) => v.parent), "test parent");
    async function x(command: string) {
      const before = parent!.lines.length;
      parent!.write(command);
      return until(() => parent!.lines[before], command);
    }
    if (process.env.CEF_NATIVE_PARENT) {
      native = jsonProcess([process.env.CEF_NATIVE_PARENT], env);
      nativeXid = (await until(() => native!.lines.find(v => v.parent), "real Win.open parent")).parent;
      await x(`target ${nativeXid}`);
    }
    return { root, env, parent: nativeXid || p.parent as number, nativeEvents: native?.lines ?? [], x, cleanup };
  } catch (e) { await cleanup(); throw e; }
}
