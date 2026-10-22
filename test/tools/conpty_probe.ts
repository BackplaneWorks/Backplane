// Windows only: pty.js's ConPTY path on its own (no hub, no Bend), to tell a
// pseudoconsole wiring fault from a program's own: cmd.exe and the package's
// bash each echo a line through a pseudoconsole; what each wrote, its exit
// code and BACKPLANE_DEBUG_CONPTY's trace are printed.
//
//   bun build --compile --target=bun-windows-x64 test/tools/conpty_probe.ts --outfile conpty_probe.exe
//   conpty_probe.exe [BASH.EXE]
import src from "../../src/server/effects/pty.js" with { type: "text" };

if (process.platform !== "win32") {
  console.log("conpty_probe: Windows only");
  process.exit(0);
}
process.env.BACKPLANE_DEBUG_CONPTY = "1";
// the functions the ConPTY path is made of, taken from pty.js as they are
const from = src.indexOf("function bp_conpty_lib(");
const to = src.indexOf("function bp_conpty_wire(");
if (from < 0 || to < 0) {
  console.log("conpty_probe: pty.js has no ConPTY path");
  process.exit(2);
}
const make = new Function("require", src.slice(from, to) + "\nreturn bp_conpty;");
const conpty = make(require);

async function run(label: string, argv: string[]) {
  let out = "";
  const env: Record<string, string> = {};
  for (const [k, v] of Object.entries(process.env)) if (v !== undefined) env[k] = v;
  env.TERM = "xterm-256color";
  let child;
  try {
    child = conpty(argv, process.cwd(), env, 80, 24);
  } catch (e) {
    console.log(`${label}: spawn failed: ${(e as Error).message}`);
    return;
  }
  child.terminal.data = (_t: unknown, d: Uint8Array) => { out += Buffer.from(d).toString("latin1"); };
  let done = false;
  child.exited.then(() => { done = true; });
  for (let i = 0; i < 100 && !done; i++) await Bun.sleep(100);
  if (!done) child.kill(15);
  await Bun.sleep(200);
  const seen = out.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, "").replace(/\x1b\][^\x07]*\x07/g, "").trim();
  console.log(`${label}: exit ${child.exitCode} (${done ? "ended" : "killed after 10 s"}), ${out.length} bytes, text: ${JSON.stringify(seen.slice(0, 200))}`);
}

const root = process.env.SystemRoot || "C:\\Windows";
await run("cmd", [root + "\\System32\\cmd.exe", "/c", "echo conpty-cmd-ok"]);
const bash = process.argv[2] || (Bun.which("bash") ?? "bash.exe");
await run("bash", [bash, "-lc", "echo conpty-bash-ok"]);
process.exit(0);
