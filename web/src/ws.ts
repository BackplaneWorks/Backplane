// data/1 JSON + binary CBOR WebSocket client for Backplane web UI.
// Authenticates strictly via the HttpOnly __Host-bp_web cookie (never ?token=).
import { RawInflater } from "./inflate";

export interface HelloFrame {
  t: "hello";
  proto: string;
  hub: string;
  name: string;
  version: string;
  epoch: string;
  now: number;
  z?: string;
  features?: string[];
  commands?: Array<{ word: string; label: string; desc: string }>;
  theme?: string;
  locale?: string;
  notice?: unknown;
}

export interface DataPush {
  t: "d";
  sub?: string;
  k: string;
  [key: string]: unknown;
}

export type JsonFrame = HelloFrame | DataPush | { t: string; [key: string]: unknown };
export type ConnState = "connecting" | "connected" | "reconnecting" | "revoked";

interface PendingCall {
  resolve: (v: unknown) => void;
  reject: (e: Error) => void;
  timer: number;
}

export class HubClient {
  private ws: WebSocket | null = null;
  private nextId = 1;
  private pending = new Map<number, PendingCall>();
  private state: ConnState = "connecting";
  private reconnectDelay = 500;
  private reconnectTimer: number | null = null;
  private closed = false;
  private inflater: RawInflater | null = null;
  private textDecoder = new TextDecoder();

  public hello: HelloFrame | null = null;
  private features = new Set<string>();

  private stateListeners = new Set<(s: ConnState, hello: HelloFrame | null) => void>();
  private frameListeners = new Set<(f: JsonFrame) => void>();
  private binaryListeners = new Set<(buf: ArrayBuffer) => void>();

  public connect(): void {
    this.closed = false;
    this.openSocket();
  }

  public disconnect(): void {
    this.closed = true;
    if (this.reconnectTimer !== null) {
      window.clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.ws?.close();
    this.ws = null;
  }

  public getState(): ConnState {
    return this.state;
  }

  public hasFeature(name: string): boolean {
    return this.features.has(name);
  }

  public onState(fn: (s: ConnState, hello: HelloFrame | null) => void): () => void {
    this.stateListeners.add(fn);
    return () => { this.stateListeners.delete(fn); };
  }

  public onFrame(fn: (f: JsonFrame) => void): () => void {
    this.frameListeners.add(fn);
    return () => { this.frameListeners.delete(fn); };
  }

  public onBinary(fn: (buf: ArrayBuffer) => void): () => void {
    this.binaryListeners.add(fn);
    return () => { this.binaryListeners.delete(fn); };
  }

  public rpc<T = Record<string, unknown>>(method: string, params: Record<string, unknown> = {}, timeoutMs = 20_000): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
        reject(new Error("Not connected to hub"));
        return;
      }
      const id = this.nextId++;
      const timer = window.setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`RPC timeout: ${method}`));
      }, timeoutMs);
      this.pending.set(id, {
        resolve: resolve as (v: unknown) => void,
        reject,
        timer,
      });
      this.ws.send(JSON.stringify({ id, m: method, p: params }));
    });
  }

  private setState(next: ConnState): void {
    this.state = next;
    for (const fn of this.stateListeners) fn(next, this.hello);
  }

  private rejectAllPending(reason: string): void {
    for (const [, p] of this.pending) {
      window.clearTimeout(p.timer);
      p.reject(new Error(reason));
    }
    this.pending.clear();
  }

  private openSocket(): void {
    if (this.closed) return;
    this.inflater = null;
    const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${proto}//${window.location.host}/ws?data=1&enc=json&z=deflate`;
    const ws = new WebSocket(url);
    ws.binaryType = "arraybuffer";
    this.ws = ws;

    const handleJson = (text: string): void => {
      let frame: JsonFrame;
      try {
        frame = JSON.parse(text) as JsonFrame;
      } catch {
        return;
      }
      if (frame.t === "hello") {
        this.hello = frame as HelloFrame;
        this.features = new Set(this.hello.features ?? []);
        this.inflater = this.hello.z === "deflate" ? new RawInflater() : null;
        this.reconnectDelay = 500;
        this.setState("connected");
      } else if (frame.t === "reply" && typeof frame.id === "number") {
        const call = this.pending.get(frame.id);
        if (call) {
          this.pending.delete(frame.id);
          window.clearTimeout(call.timer);
          if (frame.ok) {
            call.resolve(frame.result ?? {});
          } else {
            const errObj = frame.error;
            const msg =
              typeof errObj === "string"
                ? errObj
                : (errObj && typeof errObj === "object" && "message" in errObj)
                  ? String((errObj as { message: unknown }).message)
                  : "Request failed";
            call.reject(new Error(msg));
          }
        }
        return;
      }
      for (const fn of this.frameListeners) fn(frame);
    };

    ws.addEventListener("message", (ev) => {
      if (ev.data instanceof ArrayBuffer) {
        if (this.inflater) {
          let raw: Uint8Array;
          try {
            raw = this.inflater.feed(new Uint8Array(ev.data));
          } catch {
            // Broken deflate stream: close and reconnect.
            ws.close();
            return;
          }
          // Binary CBOR plot frame (header 0xA0..0xB7, 0x01, 0x64 "plot") vs JSON text frame.
          const isPlot =
            raw.length > 7 &&
            raw[0] >= 0xa0 &&
            raw[0] <= 0xb7 &&
            raw[1] === 1 &&
            raw[2] === 0x64 &&
            raw[3] === 0x70 &&
            raw[4] === 0x6c &&
            raw[5] === 0x6f &&
            raw[6] === 0x74;
          if (isPlot) {
            const copy = raw.buffer.slice(raw.byteOffset, raw.byteOffset + raw.byteLength);
            for (const fn of this.binaryListeners) fn(copy);
          } else {
            handleJson(this.textDecoder.decode(raw));
          }
          return;
        }
        for (const fn of this.binaryListeners) fn(ev.data);
        return;
      }
      if (typeof ev.data !== "string") return;
      handleJson(ev.data);
    });

    ws.addEventListener("close", (ev) => {
      if (this.ws === ws) this.ws = null;
      this.rejectAllPending("Connection closed");
      if (this.closed) return;

      // Explicit unauthorized/revoked close codes return immediately to pairing page.
      if (ev.code === 4001 || ev.code === 4401 || ev.code === 1008) {
        this.handleRevoked();
        return;
      }
      void this.probeSessionOrReconnect();
    });

    ws.addEventListener("error", () => {
      // close event follows and handles reconnect/revocation
    });
  }

  private async probeSessionOrReconnect(): Promise<void> {
    try {
      // Probe root with no-store; if revoked/expired, hub returns 401 or serves pair.html.
      const res = await fetch("/", {
        method: "GET",
        credentials: "same-origin",
        cache: "no-store",
      });
      if (res.status === 401 || res.status === 403) {
        this.handleRevoked();
        return;
      }
      if (res.ok) {
        const html = await res.text();
        if (html.includes('id="pair-root"')) {
          this.handleRevoked();
          return;
        }
      }
    } catch {
      // Network hiccup: fall through to reconnect
    }
    this.setState("reconnecting");
    const delay = this.reconnectDelay;
    this.reconnectDelay = Math.min(8000, Math.round(this.reconnectDelay * 1.6));
    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null;
      this.openSocket();
    }, delay);
  }

  private handleRevoked(): void {
    this.closed = true;
    this.setState("revoked");
    window.location.replace("/");
  }
}
