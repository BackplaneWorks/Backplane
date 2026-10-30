// Run the exact native guard script under a real POSIX flock, without a hub
// build. Closing its input models a hub loss; fixtures end by themselves.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
const source = readFileSync(new URL("../../src/server/lockio.bend", import.meta.url), "utf8");
const match = source.match(/def Guard\.cleanup\(\) -> String:\n  "(.*)"\n/);
if (!match) throw new Error("Guard.cleanup script missing");
const cleanup = JSON.parse('"' + match[1] + '"') as string;
const root = mkdtempSync(join(tmpdir(), "bp-guard-liveness-"));
const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
let assertions = 0;
function check(ok: boolean, message: string) {
  if (!ok) throw new Error(message);
  assertions++; console.log("ok " + message);
}
async function until(predicate: () => boolean, message: string, ms = 5000) {
  const end = Date.now() + ms;
  while (!predicate()) {
    if (Date.now() > end) throw new Error("fixture deadline: " + message);
    await Bun.sleep(25);
  }
}
async function bounded<T>(p: Promise<T>, message: string, ms = 6000): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  return Promise.race([p, new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error("fixture deadline: " + message)), ms); })]).finally(() => clearTimeout(timer!));
}
async function identity(pid: number) {
  const script = `if [ -r /proc/$1/stat ]; then b=$(cat /proc/sys/kernel/random/boot_id); st=$(sed 's/^.*) //' /proc/$1/stat | cut -d' ' -f20); printf '%s:%s\\n' "$b" "$st"; else ps -o lstart= -p "$1" | tr -s ' ' '_'; fi`;
  const p = Bun.spawn(["sh", "-c", script, "sh", String(pid)], { stdout: "pipe", stderr: "inherit" });
  const text = (await new Response(p.stdout).text()).trim();
  if (await p.exited !== 0 || !text) throw new Error("fixture identity unavailable");
  return text;
}
async function guardScenario(name: string, client: boolean, late: boolean) {
  const id = "w-" + name;
  const d = client ? mkdtempSync("/tmp/bp-lock-client.") : join(root, "work", id);
  mkdirSync(d, { recursive: true });
  const holder = client ? "pid:0@fixture:" + d.split("/").at(-1)! : id;
  writeFileSync(join(d, "intent"), "");
  if (client) { writeFileSync(join(d, "owner.pid"), "0"); writeFileSync(join(d, "owner.id"), "gone"); }
  const fixture = Bun.spawn(["sleep", "3"], { stdout: "ignore", stderr: "ignore" });
  const captured = await identity(fixture.pid);
  const register = () => {
    writeFileSync(join(d, client ? "cmd.pid" : "pid"), String(fixture.pid));
    writeFileSync(join(d, client ? "cmd.id" : "pidid"), captured);
  };
  if (!late) register();
  const ready = join(root, name + ".ready"), next = join(root, name + ".next"), lock = join(root, name + ".lock");
  const guard = Bun.spawn(["flock", lock, "sh", "-c", `touch ${quote(ready)}; cat >/dev/null\n` + cleanup, "sh", root, holder], { stdin: "pipe", stdout: "ignore", stderr: "inherit" });
  await until(() => existsSync(ready), name + " acquired flock");
  const waiter = Bun.spawn(["flock", lock, "sh", "-c", "touch " + quote(next)], { stdout: "ignore", stderr: "inherit" });
  guard.stdin.end();
  await Bun.sleep(250);
  check(fixture.exitCode === null, name + " leaves busy command alive after hub loss");
  check(!existsSync(next), name + " retains native exclusion after hub loss");
  if (late) register();
  await bounded(fixture.exited, name + " command ends itself");
  check(await bounded(waiter.exited, name + " next flock") === 0, name + " releases only after captured command exits");
  check(await bounded(guard.exited, name + " guard exits") === 0, name + " guard finishes cleanly");
  if (client) rmSync(d, { recursive: true, force: true });
}
try {
  await guardScenario("owned-work", false, false);
  await guardScenario("shell-client", true, false);
  await guardScenario("launch-race", true, true);
  check(!/kill -(TERM|KILL)/.test(cleanup), "lease cleanup contains no process termination command");
  console.log(`${assertions} guard liveness checks passed`);
} finally { rmSync(root, { recursive: true, force: true }); }
