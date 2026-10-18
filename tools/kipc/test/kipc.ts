// bun tools/kipc/test/kipc.ts: the helper against the stub server, over a
// real nng socket. The stub exits by itself (--idle-exit), nothing is killed.
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { createInterface } from "node:readline";

const root = new URL("../../../", import.meta.url).pathname;
const desc = root + "test/fixtures/kipc/api.desc";
const dir = mkdtempSync(tmpdir() + "/kipc-");
const sock = dir + "/api.sock";
let bad = 0;
const check = (name: string, ok: boolean, extra?: unknown) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : " " + JSON.stringify(extra)}`);
  if (!ok) bad++;
};

const stub = spawn("bun", [root + "test/tools/kipc_stub.ts", "api-server", "x.kicad_pro", "--socket", sock, "--parent-pid", String(process.pid), "--idle-exit", "30"], { env: { ...process.env, KIPC_DESC: desc }, stdio: ["ignore", "pipe", "inherit"] });
const ready = await new Promise<string>((res) => createInterface({ input: stub.stdout! }).once("line", res));
check("ready line", ready === `KiCad API server listening at ${sock}`, ready);

async function session(args: string[], reqs: object[]) {
  const h = spawn("bun", [root + "tools/kipc/src/main.ts", ...args], { stdio: ["pipe", "pipe", "inherit"] });
  const rl = createInterface({ input: h.stdout! });
  const replies: any[] = [];
  const done = new Promise<void>((res) => rl.on("line", (l) => { replies.push(JSON.parse(l)); if (replies.length === reqs.length) res(); }));
  for (const r of reqs) h.stdin!.write(JSON.stringify(r) + "\n");
  await done;
  h.stdin!.end();
  return replies.sort((a, b) => a.id - b.id);
}

const withDesc = await session(["--desc", desc], [
  { id: 1, sock, type: "kiapi.common.commands.Ping", message: {} },
  { id: 2, sock, type: "kiapi.common.commands.GetVersion", message: {} },
  { id: 3, sock, type: "kiapi.board.commands.GetItems", message: { types: ["track"] } },
  { id: 4, sock, type: "kiapi.board.commands.Nope", message: {} },
  { id: 5, sock, type: "kiapi.common.commands.BeginCommit", message: {} },
  { id: 6, sock: dir + "/missing.sock", type: "kiapi.common.commands.Ping", message: {} },
]);
check("ping", withDesc[0].ok === true, withDesc[0]);
check("version", withDesc[1].message?.version?.full_version === "10.99.0-bp.1", withDesc[1]);
check("items carry KIIDs", withDesc[2].message?.items?.length === 2 && withDesc[2].message.items[0].id.value.endsWith("1"), withDesc[2]);
check("unknown command", withDesc[3].ok === false, withDesc[3]);
check("unimplemented keeps its status", withDesc[4].ok === false && withDesc[4].status === "AS_UNIMPLEMENTED", withDesc[4]);
check("dead socket is an error", withDesc[5].ok === false && /dial/.test(withDesc[5].error), withDesc[5]);

const bare = await session([], [
  { id: 1, sock, type: "kiapi.common.commands.Ping", message: {} },
  { id: 2, sock, type: "kiapi.common.commands.GetVersion", message: {} },
  { id: 3, sock, type: "kiapi.board.commands.GetItems", message: {} },
]);
check("builtin ping", bare[0].ok === true, bare[0]);
check("builtin version", bare[1].message?.version?.full_version === "10.99.0-bp.1", bare[1]);
check("no schema for the rest", bare[2].ok === false && bare[2].error === "no API schema in this KiCad runtime", bare[2]);

const code = await new Promise<number | null>((res) => {
  stub.on("exit", res);
  void session(["--desc", desc], [{ id: 1, sock, type: "kiapi.backplane.Shutdown", message: {} }]);
});
check("shutdown exits 0", code === 0, code);
// the helper starts a server, reports its lines, and its exit (idle-exit, no signal)
{
  const sock2 = dir + "/b.sock";
  const h = spawn("bun", [root + "tools/kipc/src/main.ts", "--desc", desc], { env: { ...process.env, KIPC_DESC: desc }, stdio: ["pipe", "pipe", "inherit"] });
  const events: any[] = [];
  const replies: any[] = [];
  createInterface({ input: h.stdout! }).on("line", (l) => {
    const o = JSON.parse(l);
    (o.event ? events : replies).push(o);
  });
  h.stdin!.write(JSON.stringify({ id: 1, op: "spawn", key: "p1", argv: ["bun", root + "test/tools/kipc_stub.ts", "api-server", "x.kicad_pro", "--socket", sock2, "--parent-pid", String(process.pid), "--idle-exit", "1"] }) + "\n");
  const until = async (f: () => boolean) => {
    for (let i = 0; i < 100 && !f(); i++) await new Promise((r) => setTimeout(r, 100));
  };
  await until(() => events.some((e) => e.event === "exit"));
  check("spawn is answered", replies[0]?.ok === true, replies);
  check("server line is reported", events.some((e) => e.event === "line" && e.key === "p1" && e.text === `KiCad API server listening at ${sock2}`), events);
  check("idle-exit is a clean exit", events.some((e) => e.event === "exit" && e.key === "p1" && e.code === 0), events);
  h.stdin!.end();
  const bad = spawn("bun", [root + "tools/kipc/src/main.ts"], { stdio: ["pipe", "pipe", "inherit"] });
  const evs: any[] = [];
  createInterface({ input: bad.stdout! }).on("line", (l) => evs.push(JSON.parse(l)));
  bad.stdin!.write(JSON.stringify({ id: 2, op: "spawn", key: "p2", argv: ["/nonexistent/kicad-cli", "api-server"] }) + "\n");
  await until(() => evs.some((e) => e.event === "exit"));
  check("a missing program is exit 127", evs.some((e) => e.event === "exit" && e.key === "p2" && e.code === 127), evs);
  bad.stdin!.end();
}
rmSync(dir, { recursive: true, force: true });
process.exit(bad ? 1 : 0);
