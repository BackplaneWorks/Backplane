// Exercise the real launch script and native guard across the crash window
// after an intent was saved but before its process id existed. Fixtures end
// themselves; every wait has a deadline, and this test never sends a signal.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
const root = mkdtempSync(join(tmpdir(), "bp-work-intent-"));
const id = "w-prelaunch", dir = join(root, "work", id), lock = join(root, "fixture.lock");
mkdirSync(dir, { recursive: true });
const source = readFileSync(new URL("../../src/server/server.bend", import.meta.url), "utf8");
const guardSource = readFileSync(new URL("../../src/server/lockio.bend", import.meta.url), "utf8");
function script(source: string, name: string) {
  const match = source.match(new RegExp("def " + name.replaceAll(".", "\\.") + "\\(\\) -> String:\\n  \"(.*)\"\\n"));
  if (!match) throw new Error(name + " not found");
  return JSON.parse('"' + match[1] + '"') as string;
}
const launch = script(source, "Watch.launch.sh"), cleanup = script(guardSource, "Guard.cleanup");
const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
function check(ok: boolean, message: string) {
  if (!ok) throw new Error(message);
  console.log("ok " + message);
}
async function until(predicate: () => boolean, label: string, ms = 6000) {
  const end = Date.now() + ms;
  while (!predicate()) {
    if (Date.now() >= end) throw new Error("fixture deadline: " + label);
    await Bun.sleep(25);
  }
}
async function bounded<T>(p: Promise<T>, label: string, ms = 6000): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  return Promise.race([p, new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error("fixture deadline: " + label)), ms); })]).finally(() => clearTimeout(timer!));
}
try {
  writeFileSync(join(dir, "intent"), "");
  const ready = join(root, "guard-ready"), granted = join(root, "next-granted"), runs = join(root, "runs");
  const guard = Bun.spawn(["flock", lock, "sh", "-c", "touch " + quote(ready) + "; cat >/dev/null\n" + cleanup, "sh", root, id],
    { stdin: "pipe", stdout: "ignore", stderr: "inherit" });
  await until(() => existsSync(ready), "original native guard");
  const waiter = Bun.spawn(["flock", lock, "sh", "-c", "touch " + quote(granted)], { stdout: "ignore", stderr: "inherit" });
  guard.stdin.end(); // Old hub disappears before it wrote any pid.
  await Bun.sleep(250);
  check(!existsSync(granted), "prelaunch intent retains native exclusion after hub loss");
  check(!existsSync(join(dir, "pid")), "crash fixture has no process to adopt yet");
  const command = "printf 'once\\n' >> " + quote(runs) + "; sleep 1; printf done";
  // The extra wait keeps this fixture's launcher alive to reap its own child.
  const recovered = Bun.spawn(["sh", "-c", launch + "\nwait\n", "sh", dir, root, command], { stdout: "ignore", stderr: "inherit" });
  await until(() => existsSync(join(dir, "pid")) && existsSync(runs), "same intent relaunched");
  const duplicate = Bun.spawn(["sh", "-c", launch, "sh", dir, root, command], { stdout: "ignore", stderr: "inherit" });
  check(await bounded(duplicate.exited, "idempotent rearm") === 0, "repeated observation recognizes the already launched intent");
  check(readFileSync(runs, "utf8") === "once\n", "same work id executes the command once");
  check(!existsSync(granted), "recovered command remains protected by the original native guard");
  check(await bounded(recovered.exited, "recovered fixture ends") === 0, "recovered fixture finishes without interruption");
  await until(() => existsSync(join(dir, "code")), "saved real outcome");
  check(readFileSync(join(dir, "code"), "utf8").trim() === "0", "real final outcome remains available to watcher");
  check(await bounded(guard.exited, "original guard releases") === 0, "original guard releases after recovered command ends");
  check(await bounded(waiter.exited, "next waiter granted") === 0 && existsSync(granted), "next waiter acquires only after protected work finishes");
} finally {
  rmSync(root, { recursive: true, force: true });
}
