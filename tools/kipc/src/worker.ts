// One socket's thread: connects on the first request, sends, waits for the
// reply. A failed request drops the connection so the next one dials again.
import { load, Req, NngError } from "./nng.ts";

declare const self: Worker;

let req: Req | null = null;
let at = "";

function drop() {
  try {
    req?.close();
  } catch {}
  req = null;
}

self.onmessage = (ev: MessageEvent) => {
  const m = ev.data as { op: string; id?: string; sock?: string; lib?: string; bytes?: Uint8Array; timeout?: number };
  if (m.op === "close") {
    drop();
    self.postMessage({ op: "closed" });
    return;
  }
  try {
    if (!req || at !== m.sock) {
      drop();
      const r = new Req(load(m.lib));
      try {
        r.dial("ipc://" + m.sock);
      } catch (e) {
        r.close();
        throw e;
      }
      req = r;
      at = m.sock!;
    }
    const out = req.request(m.bytes!, m.timeout ?? 30000);
    self.postMessage({ op: "reply", id: m.id, bytes: out });
  } catch (e) {
    drop();
    const timeout = e instanceof NngError && e.timeout;
    self.postMessage({ op: "reply", id: m.id, error: timeout ? `KiCad did not answer in ${(m.timeout ?? 30000) / 1000} s` : String((e as Error).message ?? e) });
  }
};
