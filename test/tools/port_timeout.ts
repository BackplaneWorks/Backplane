// Run the production Port.tmo emitted by test/native/port_timeout.bend.
// Force the macOS Perl fallback on every platform, using only temp files
// and captured test PIDs. No hub, provider, network or user home is used.
//   bun test/tools/port_timeout.ts build/port-timeout.sh [...]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const files = process.argv.slice(2).map(path => resolve(path));
if (!files.length) throw new Error("usage: port_timeout.ts EMITTED_SCRIPT [...]");
const root = realpathSync(mkdtempSync(join(tmpdir(), "bp-port-timeout-")));
const fallback = join(root, "perl only"), home = join(root, "home");
mkdirSync(fallback); mkdirSync(home);
symlinkSync("/usr/bin/perl", join(fallback, "perl"));
const env = { PATH: fallback, HOME: home, BACKPLANE_HOME: join(home, "backplane-test"), LANG: "C" };
let checks = 0, failures = 0;
const captured = new Set<number>();
function check(name: string, ok: boolean, detail?: unknown) {
  checks++;
  console.log(`${ok ? "ok" : "FAIL"} ${name}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
  if (!ok) failures++;
}
function stopped(pid: number) {
  const ps = spawnSync("/bin/ps", ["-o", "stat=", "-p", String(pid)], { encoding: "utf8" });
  return ps.status !== 0 || !ps.stdout.trim() || ps.stdout.trim().startsWith("Z");
}
function pids(path: string) {
  const values = existsSync(path) ? readFileSync(path, "utf8").trim().split(/\s+/).map(Number).filter(n => Number.isSafeInteger(n) && n > 1) : [];
  values.forEach(pid => captured.add(pid));
  return values;
}
async function run(script: string, args: string[], path = fallback, afterStart?: () => Promise<void>) {
  const start = performance.now();
  const child = Bun.spawn(["/bin/sh", "-c", `${script}\ntmo "$@"`, "sh", ...args], { cwd: root, env: { ...env, PATH: path }, stdin: "ignore", stdout: "pipe", stderr: "pipe" });
  // The fixture commands finish in <=4 seconds even on the old wrapper.
  // This watchdog guards an unexpected harness failure without name kills.
  const watchdog = setTimeout(() => child.kill("SIGKILL"), 10000);
  try {
    if (afterStart) await afterStart();
    const [code, out, err] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
    return { code, out, err, ms: performance.now() - start };
  } finally { clearTimeout(watchdog); }
}
try {
  for (const [index, file] of files.entries()) {
    const script = readFileSync(file, "utf8"), label = `${index + 1} ${basename(file)}`;
    let r = await run(script, ["2", "/usr/bin/perl", "-e", "print join(qq(\\0), @ARGV)", "--", "a b", "", "quote'\"", "$literal"]);
    check(`${label}: argument bytes and empty arguments survive`, r.code === 0 && r.out === ["a b", "", "quote'\"", "$literal"].join("\0"), r);
    r = await run(script, ["2", "/bin/sh", "-c", "printf normal; exit 23"]);
    check(`${label}: normal exit code and stdout survive`, r.code === 23 && r.out === "normal", r);
    r = await run(script, ["2", join(root, "does not exist")]);
    check(`${label}: failed exec reports 127`, r.code === 127, r);
    r = await run(script, ["2", "/bin/sh", "-c", "kill -TERM $$"]);
    check(`${label}: signal exit is preserved`, r.code === 143, r);
    r = await run(script, ["0", "/bin/sh", "-c", "printf no-deadline"]);
    check(`${label}: zero disables deadline`, r.code === 0 && r.out === "no-deadline", r);

    r = await run(script, ["1", "/bin/sleep", "4"]);
    check(`${label}: direct timeout reports 124`, r.code === 124 && r.ms >= 850 && r.ms < 2900, r);
    r = await run(script, ["1", "/bin/sh", "-c", "/bin/sleep 4; printf descendant-finished"]);
    check(`${label}: descendant output pipes close by deadline`, r.code === 124 && r.out === "" && r.ms < 2900, r);

    for (const leaderIgnores of [false, true]) {
      const pidFile = join(root, `pids-${index}-${leaderIgnores}`);
      const command = `use strict; use warnings; $|=1; open my $f, ">", $ARGV[0] or die $!; print $f "$$\\n"; close $f; ${leaderIgnores ? '$SIG{TERM}="IGNORE";' : ''} my $p=fork; defined($p) or die $!; if (!$p) { $SIG{TERM}="IGNORE"; open my $g, ">>", $ARGV[0] or die $!; print $g "$$\\n"; close $g; sleep 4; print "late"; exit 0; } waitpid($p,0);`;
      r = await run(script, ["1", "/usr/bin/perl", "-e", command, "--", pidFile]);
      const children = pids(pidFile);
      check(`${label}: ${leaderIgnores ? "TERM-ignoring group" : "exited leader's child"} gets KILL grace`, r.code === 124 && r.out === "" && r.ms < 2900, r);
      check(`${label}: only captured test descendants are stopped`, children.length === 2 && children.every(stopped), children);
    }

    const interruptedPids = join(root, `interrupted-${index}`);
    const interruptCommand = 'open my $f, ">", $ARGV[0] or die $!; print $f "$$ ",getppid(),"\\n"; close $f; $SIG{TERM}="IGNORE"; sleep 4; print "late";';
    r = await run(script, ["9", "/usr/bin/perl", "-e", interruptCommand, "--", interruptedPids], fallback, async () => {
      const until = performance.now() + 2000;
      while ((!existsSync(interruptedPids) || readFileSync(interruptedPids, "utf8").trim().split(/\s+/).length !== 2) && performance.now() < until) await Bun.sleep(10);
      const [commandPid, supervisorPid] = pids(interruptedPids);
      if (!commandPid || !supervisorPid) throw new Error("test command did not report its captured supervisor PID");
      process.kill(supervisorPid, "SIGTERM");
    });
    check(`${label}: supervisor forwards TERM and completes KILL grace`, r.code === 143 && r.out === "" && r.ms < 2400, r);
    check(`${label}: interrupted command has stopped`, pids(interruptedPids).every(stopped));

    // Exercise both preferred dispatch branches without depending on which
    // coreutils packages are installed on the test machine.
    for (const name of ["timeout", "gtimeout"]) {
      const bin = join(root, `dispatch-${index}-${name}`);
      mkdirSync(bin);
      const executable = join(bin, name);
      writeFileSync(executable, `#!/bin/sh\nprintf '%s\\n' '${name}' "$@"\n`);
      chmodSync(executable, 0o755);
      r = await run(script, ["7", "command with spaces", "a b", ""], bin);
      check(`${label}: prefers ${name} with unchanged argv`, r.code === 0 && r.out === `${name}\n-k\n1\n7\ncommand with spaces\na b\n\n`, r);
    }
  }
} finally {
  for (const pid of captured) {
    if (!stopped(pid)) { try { process.kill(pid, "SIGKILL"); } catch {} }
  }
  rmSync(root, { recursive: true, force: true });
}
console.log(`${checks} assertions, ${failures} failures`);
if (failures) process.exitCode = 1;
