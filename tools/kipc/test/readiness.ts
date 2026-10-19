// Buffered stdout readiness: only a socket created by this launch is announced.
// Children exit on their own; no signals or real KiCad runtime are required.
import { createServer } from "node:net";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const repo = new URL("../../../", import.meta.url).pathname;
let failures = 0;
for (const mode of ["fresh", "occupied", "no-new-socket"] as const) {
  const dir = mkdtempSync(join(tmpdir(), "bp-ready-"));
  const requested = join(dir, "api.sock");
  const alternative = join(dir, "api-999999.sock");
  const old = mode === "fresh" ? null : createServer();
  if (old) await new Promise<void>(resolve => old.listen(requested, resolve));
  const expected = mode === "no-new-socket" ? null : mode === "occupied" ? alternative : requested;
  const fixture = join(dir, "fixture.ts");
  writeFileSync(fixture, expected
    ? `import {createServer} from 'node:net'; const s=createServer(); s.listen(${JSON.stringify(expected)}); setTimeout(()=>s.close(()=>process.exit(0)),1200);`
    : "setTimeout(()=>process.exit(0),1200);");
  const helper = Bun.spawn([process.execPath, repo + "tools/kipc/src/main.ts"], { stdin: "pipe", stdout: "pipe", stderr: "inherit" });
  helper.stdin.write(JSON.stringify({ id: 1, op: "spawn", key: mode, argv: [process.execPath, fixture, "--socket", requested] }) + "\n");
  const events: any[] = [];
  let pending = "";
  const deadline = setTimeout(() => helper.stdin.end(), 5000);
  for await (const chunk of helper.stdout) {
    pending += new TextDecoder().decode(chunk);
    let at: number;
    while ((at = pending.indexOf("\n")) >= 0) {
      const event = JSON.parse(pending.slice(0, at));
      pending = pending.slice(at + 1);
      events.push(event);
      if (event.event === "exit") helper.stdin.end();
    }
  }
  clearTimeout(deadline);
  const code = await helper.exited;
  if (old) await new Promise<void>(resolve => old.close(() => resolve()));
  const lines = events.filter(e => e.event === "line" && e.text.startsWith("KiCad API server listening at "));
  const exit = events.findIndex(e => e.event === "exit" && e.code === 0);
  const ok = code === 0 && exit >= 0 && (expected === null
    ? lines.length === 0
    : lines.length === 1 && lines[0].text === `KiCad API server listening at ${expected}` && events.indexOf(lines[0]) < exit);
  console.log(`${ok ? "ok" : "FAIL"} ${mode}: buffered readiness uses only the new server socket${ok ? "" : " " + JSON.stringify(events)}`);
  if (!ok) failures++;
  rmSync(dir, { recursive: true, force: true });
}
process.exit(failures ? 1 : 0);
