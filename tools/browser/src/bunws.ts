// Playwright's connectOverCDP under Bun: playwright-core's vendored `ws`
// client never completes the WebSocket upgrade there, while Bun's own
// WebSocket does. BunWs is the slice of `ws` that Playwright's
// WebSocketTransport uses, over Bun's WebSocket.
//
// playwright-core must read it: patchCore() rewrites the one line of
// lib/coreBundle.js that picks `ws` (utilsBundle.ws is a read-only getter, so
// it has to be a source transform), and useBunWs() sets the global it reads.
// build.ts applies the patch while bundling; init.ts does the same by other
// means when running from source. Unless useBunWs() ran, the patched line
// picks the stock `ws`, so chrome mode (which talks over a pipe) is unchanged.
import { EventEmitter } from "node:events";

export class BunWs extends EventEmitter {
  static CONNECTING = 0; static OPEN = 1; static CLOSING = 2; static CLOSED = 3;
  CONNECTING = 0; OPEN = 1; CLOSING = 2; CLOSED = 3;
  w: WebSocket;
  constructor(url: string, _protocols: string[], opts: any = {}) {
    super();
    this.w = new WebSocket(url, { headers: opts.headers } as any);
    this.w.onopen = () => this.emit("open");
    this.w.onmessage = (e) => { const ev = { data: typeof e.data === "string" ? e.data : Buffer.from(e.data as ArrayBuffer).toString() }; this.emit("message", ev); };
    this.w.onclose = (e) => this.emit("close", { code: e.code, reason: e.reason, wasClean: e.wasClean });
    this.w.onerror = () => this.emit("error", { type: "error", message: "websocket error" });
  }
  get readyState() { return this.w.readyState; }
  addEventListener(ev: string, f: (...a: any[]) => void) { this.on(ev, f); }
  removeEventListener(ev: string, f: (...a: any[]) => void) { this.off(ev, f); }
  send(s: string) {
    // This transport is CEF-only. Playwright's synthetic focus obtains a
    // Chromium capture handle (stay_hidden=false), which keeps minimized
    // native widgets painting. CEF uses real X focus; retain CDP's reply
    // and native input semantics without that permanent visibility lock.
    if (s.includes('"Emulation.setFocusEmulationEnabled"')) {
      const message = JSON.parse(s);
      if (message.method === "Emulation.setFocusEmulationEnabled" && message.params?.enabled) {
        message.params.enabled = false;
        s = JSON.stringify(message);
      }
    }
    this.w.send(s);
  }
  close(code?: number, reason?: string) { this.w.close(code, reason); }
}

// make the next CDP connections use BunWs (cef mode)
export function useBunWs(): void {
  (globalThis as any).__bpWs = BunWs;
}

const WS_LINE = 'ws = require("./utilsBundle").ws;';
// a constructor chosen when a transport is made, not when the module loads
const WS_PATCHED =
  'ws = function ws(...a) { const W = globalThis.__bpWs ?? require("./utilsBundle").ws; return new W(...a); };';

// fails the build when a playwright-core update moves the line
export function patchCore(text: string): string {
  if (!text.includes(WS_LINE)) throw new Error("playwright-core changed: the BunWs patch no longer matches coreBundle.js");
  return text.replace(WS_LINE, WS_PATCHED);
}

// a `ws` constructor that picks BunWs when useBunWs() ran, else `stock`
export function wsChooser(stock: any): any {
  return function ws(...a: any[]) {
    const W = (globalThis as any).__bpWs ?? stock;
    return new W(...a);
  };
}
