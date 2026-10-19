// A stand-in for `kicad-cli api-server` (tests only): an nng REP server on
// the given socket that speaks the few commands of test/fixtures/kipc.
//   bun kipc_stub.ts api-server <pro> --socket <path> --parent-pid <pid> [--idle-exit <s>]
// Env: KIPC_DESC (descriptor set), KIPC_LIB (runtime lib dir), KIPC_LOG
// (a file that gets "<pid> <type>" per request), KIPC_VERSION.
// It exits by itself: when its parent is gone, after --idle-exit seconds
// without a request, after Shutdown (code 0), or on test.Crash (code 1).
import { existsSync, appendFileSync } from "node:fs";
import { load, Rep } from "../../tools/kipc/src/nng.ts";
import { load as loadSchema } from "../../tools/kipc/src/schema.ts";
import { decodeRequest, encodeResponse } from "../../tools/kipc/src/codec.ts";

const args = process.argv.slice(2);
const opt = (n: string) => {
  const i = args.indexOf(n);
  return i >= 0 ? args[i + 1] : "";
};
const sock = opt("--socket");
if (!sock) process.exit(2);
const parent = Number(opt("--parent-pid") || 0);
// KIPC_IDLE (seconds) overrides --idle-exit, so a test need not wait ten minutes
const idle = Number(process.env.KIPC_IDLE || opt("--idle-exit") || 0);
const version = process.env.KIPC_VERSION ?? "10.99.0-bp.4";
// KIPC_DIRTY: a project path part whose server has unsaved changes, so it
// refuses Shutdown without force (CONTRACT "B1 amended") and stays up
const dirty = !!process.env.KIPC_DIRTY && args.join(" ").includes(process.env.KIPC_DIRTY);
const schema = loadSchema(process.env.KIPC_DESC ?? "");
const token = `stub-${process.pid}`;

const rep = new Rep(load(process.env.KIPC_LIB));
rep.listen("ipc://" + sock);
console.log(`KiCad API server listening at ${sock}`);

const items = [
  { "@type": "kiapi.board.commands.Track", id: { value: "00000000-0000-0000-0000-000000000001" }, net: "GND", width: "250000" },
  { "@type": "kiapi.board.commands.Track", id: { value: "00000000-0000-0000-0000-000000000002" }, net: "VCC", width: "300000" },
];

function answer(type: string): { type?: string; message?: unknown; status?: number; error?: string } {
  switch (type) {
    case "kiapi.common.commands.Ping":
      return {};
    case "kiapi.common.commands.GetVersion":
      return { type: "kiapi.common.commands.GetVersionResponse", message: { version: { major: 10, minor: 99, patch: 0, full_version: version } } };
    case "kiapi.backplane.GetCapabilities":
      return {
        type: "kiapi.backplane.GetCapabilitiesResponse",
        message: {
          fork_version: version, api_version: 1,
          capabilities: ["api-server", "parent-pid", "idle-exit", "shutdown", "api-descriptor"],
          commands: ["kiapi.common.commands.Ping", "kiapi.common.commands.GetVersion", "kiapi.board.commands.GetItems", "kiapi.backplane.Shutdown"],
        },
      };
    case "kiapi.board.commands.GetItems":
      return { type: "kiapi.board.commands.GetItemsResponse", message: { items } };
    case "test.Crash":
      return {};
    case "kiapi.backplane.Shutdown":
      return dirty ? { status: 2, error: "a document has unsaved changes" } : {};
    default:
      return { status: 8, error: `${type} is not available in the stub` };
  }
}

let last = Date.now();
for (;;) {
  const bytes = rep.next(300);
  if (parent && !existsSync(`/proc/${parent}`)) process.exit(0);
  if (!bytes) {
    if (idle && Date.now() - last > idle * 1000) process.exit(0);
    continue;
  }
  last = Date.now();
  const req = decodeRequest(schema, bytes);
  if (process.env.KIPC_LOG) appendFileSync(process.env.KIPC_LOG, `${process.pid} ${req.type}\n`);
  const out = answer(req.type);
  rep.send(encodeResponse(schema, { ...out, token }));
  // test.Crash: answered, then the server dies (exit 1)
  if (req.type === "test.Crash") process.exit(1);
  if (req.type === "kiapi.backplane.Shutdown" && !dirty) process.exit(0);
}
