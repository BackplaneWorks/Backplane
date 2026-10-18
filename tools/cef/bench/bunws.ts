// The slice of the `ws` client Playwright's WebSocketTransport uses, over Bun's WebSocket.
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
  send(s: string) { this.w.send(s); }
  close(code?: number, reason?: string) { this.w.close(code, reason); }
}
