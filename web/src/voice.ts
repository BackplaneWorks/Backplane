// Browser microphone dictation over data/1 (`voice.start` with hub-side `src: "client"`).
// - Cancellable acquisition lifecycle: generation-guarded across double-clicks, thread switches,
//   disconnects, and upload failures; a stale start/permission reply releases only its own local
//   stream and never tears down a newer recording.
// - Graceful `stop()` flushes all queued audio chunks in order during slow uploads before sending
//   the final `X-Voice-End: 1` chunk; only `cancel()` / upload failure aborts queued chunks.
// - Uploads 16 kHz mono Int16 (s16le) ~200 ms chunks to `POST /voice/in` with same-origin cookie
//   auth and `X-Voice-Session` / `X-Voice-End` headers (never placing the capability in a URL).
import type { HubClient } from "./ws";

const TARGET_RATE = 16000;
const CHUNK_SAMPLES = 3200; // 200 ms at 16 kHz

export interface VoiceCallbacks {
  getThreadId: () => string;
  getDraft: () => string;
  getCaret: () => number;
  setDraft: (text: string) => void;
  setRecording: (active: boolean) => void;
  setLevel: (level: number) => void;
  showError: (msg: string) => void;
}

function parseSessionKey(path: string): string {
  try {
    const u = new URL(path, window.location.origin);
    return u.searchParams.get("s") || "";
  } catch {
    return "";
  }
}

function stopStreamTracks(stream: MediaStream | null): void {
  if (!stream) return;
  for (const track of stream.getTracks()) {
    try { track.stop(); } catch { /* ignore */ }
  }
}

function downsampleToInt16(input: Float32Array, inRate: number): Int16Array {
  if (inRate === TARGET_RATE) {
    const out = new Int16Array(input.length);
    for (let i = 0; i < input.length; i++) {
      const s = Math.max(-1, Math.min(1, input[i]));
      out[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
    }
    return out;
  }
  const ratio = inRate / TARGET_RATE;
  const outLen = Math.max(1, Math.floor(input.length / ratio));
  const out = new Int16Array(outLen);
  for (let i = 0; i < outLen; i++) {
    const start = Math.floor(i * ratio);
    const end = Math.min(input.length, Math.floor((i + 1) * ratio));
    let sum = 0;
    let count = 0;
    for (let j = start; j < end; j++) {
      sum += input[j];
      count++;
    }
    const s = Math.max(-1, Math.min(1, count > 0 ? sum / count : 0));
    out[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
  }
  return out;
}

export class VoiceController {
  private hub: HubClient;
  private cb: VoiceCallbacks;
  private active = false;
  private starting = false;
  private sessionGen = 0;
  private activeGen = 0;
  private abortedGen = 0;
  private threadId = "";
  private sessionKey = "";
  private stream: MediaStream | null = null;
  private audioCtx: AudioContext | null = null;
  private processor: ScriptProcessorNode | null = null;
  private pendingPcm: number[] = [];
  private postChain: Promise<void> = Promise.resolve();
  private stopDrain: Promise<void> = Promise.resolve();
  private mark = 0;

  constructor(hub: HubClient, cb: VoiceCallbacks) {
    this.hub = hub;
    this.cb = cb;
  }

  public isRecording(): boolean {
    return this.active || this.starting;
  }

  public async toggle(): Promise<void> {
    if (this.active || this.starting) {
      await this.stop();
    } else {
      await this.start();
    }
  }

  public async start(): Promise<void> {
    if (this.active || this.starting) return;
    const tid = this.cb.getThreadId();
    if (!tid) return;
    if (!navigator.mediaDevices?.getUserMedia) {
      this.cb.showError("Microphone access is not available in this browser.");
      return;
    }

    const gen = ++this.sessionGen;
    this.starting = true;
    this.threadId = tid;
    this.sessionKey = "";
    this.pendingPcm = [];
    this.mark = this.cb.getCaret();
    this.cb.setRecording(true);

    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch {
      if (gen === this.sessionGen) {
        this.starting = false;
        this.cb.setRecording(false);
        this.cb.showError("Microphone permission denied.");
      }
      return;
    }

    // If cancelled, navigated to another thread, or superseded while permission was pending,
    // immediately release ONLY this local stream and abort without touching newer state.
    if (gen !== this.sessionGen || this.cb.getThreadId() !== tid) {
      stopStreamTracks(stream);
      return;
    }

    this.stream = stream;
    // Serialize against any prior stop() drain (final chunk upload + voice.stop RPC ack)
    // so a draining recording A can never send a late voice.stop after recording B starts.
    await this.stopDrain;
    if (gen !== this.sessionGen || this.cb.getThreadId() !== tid) {
      stopStreamTracks(stream);
      if (this.stream === stream) this.stream = null;
      return;
    }

    try {
      const res = await this.hub.rpc<{ path?: string; session?: string }>("voice.start", {
        thread: tid,
        draft: this.cb.getDraft(),
        caret: this.mark,
      });
      if (gen !== this.sessionGen || this.cb.getThreadId() !== tid) {
        // Stale voice.start reply: release ONLY this generation's local stream; never touch
        // a newer recording's audio resources or stop a newer session on the hub.
        stopStreamTracks(stream);
        if (this.stream === stream) this.stream = null;
        if (!this.active && !this.starting) {
          void this.hub.rpc("voice.stop", { thread: tid }).catch(() => {});
        }
        return;
      }
      const key = res.session || (res.path ? parseSessionKey(res.path) : "");
      if (!key) throw new Error("Hub did not return a voice session.");
      this.sessionKey = key;
      this.activeGen = gen;
      this.starting = false;
      this.active = true;
      this.startCapture(stream, gen);
    } catch (e) {
      if (gen === this.sessionGen) {
        this.cleanupAudio();
        this.starting = false;
        this.active = false;
        this.cb.setRecording(false);
        this.cb.showError(e instanceof Error ? e.message : "Could not start voice dictation.");
      } else {
        stopStreamTracks(stream);
        if (this.stream === stream) this.stream = null;
      }
    }
  }

  public async stop(): Promise<void> {
    if (!this.active && !this.starting) return;
    const captureGen = this.activeGen;
    const wasActive = this.active;
    const tid = this.threadId;
    const key = this.sessionKey;
    // Advance sessionGen so any in-flight start() is invalidated, but do NOT bump abortedGen:
    // already-queued chunks for `captureGen` must flush in order before the final `end=1` chunk.
    this.sessionGen++;
    this.starting = false;
    this.active = false;
    this.sessionKey = "";
    this.cb.setRecording(false);
    this.cb.setLevel(0);
    this.cleanupAudio();

    const prevDrain = this.stopDrain;
    const drain = prevDrain.then(async () => {
      if (wasActive && key) {
        const tail = new Int16Array(this.pendingPcm);
        this.pendingPcm = [];
        this.enqueuePost(key, new Uint8Array(tail.buffer), true, captureGen);
        await this.postChain;
      } else {
        this.pendingPcm = [];
      }
      if (wasActive && tid) {
        await this.hub.rpc("voice.stop", { thread: tid }).catch(() => {});
      }
    });
    this.stopDrain = drain;
    await drain;
  }

  public cancel(): void {
    if (!this.active && !this.starting) return;
    const wasActive = this.active;
    const tid = this.threadId;
    this.abortedGen = Math.max(this.abortedGen, this.sessionGen);
    this.sessionGen++;
    this.starting = false;
    this.active = false;
    this.sessionKey = "";
    this.pendingPcm = [];
    this.cb.setRecording(false);
    this.cb.setLevel(0);
    this.cleanupAudio();
    if (wasActive && tid && this.hub.getState() === "connected") {
      void this.hub.rpc("voice.stop", { thread: tid }).catch(() => {});
    }
  }

  public handleFrame(frame: Record<string, unknown>): void {
    const ev = String(frame.ev ?? "");
    const tid = String(frame.thread ?? "");
    if (tid && this.cb.getThreadId() && tid !== this.cb.getThreadId()) return;

    if (ev === "draft" && typeof frame.text === "string") {
      this.cb.setDraft(frame.text);
      this.mark = frame.text.length;
    } else if (ev === "edit") {
      const cur = this.cb.getDraft();
      const del = Number(frame.del ?? 0);
      const ins = String(frame.ins ?? "");
      const pos = Math.max(0, Math.min(cur.length, this.mark));
      const start = Math.max(0, pos - del);
      const next = cur.slice(0, start) + ins + cur.slice(pos);
      this.mark = start + ins.length;
      this.cb.setDraft(next);
    } else if (ev === "level") {
      this.cb.setLevel(Math.max(0, Math.min(100, Number(frame.v ?? 0))));
    } else if (ev === "view") {
      const view = frame.view as { on?: boolean } | undefined;
      if (view && view.on === false && (this.active || this.starting)) {
        void this.stop();
      }
    }
  }

  private startCapture(stream: MediaStream, gen: number): void {
    const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    const ctx = new AudioCtx();
    const source = ctx.createMediaStreamSource(stream);
    const proc = ctx.createScriptProcessor(4096, 1, 1);

    proc.onaudioprocess = (ev) => {
      if (!this.active || gen !== this.sessionGen || !this.sessionKey) return;
      const pcm = downsampleToInt16(ev.inputBuffer.getChannelData(0), ctx.sampleRate);
      for (let i = 0; i < pcm.length; i++) this.pendingPcm.push(pcm[i]);
      while (this.pendingPcm.length >= CHUNK_SAMPLES) {
        const slice = new Int16Array(this.pendingPcm.splice(0, CHUNK_SAMPLES));
        this.enqueuePost(this.sessionKey, new Uint8Array(slice.buffer), false, gen);
      }
    };

    source.connect(proc);
    proc.connect(ctx.destination);
    this.audioCtx = ctx;
    this.processor = proc;
  }

  private enqueuePost(key: string, body: Uint8Array, end: boolean, gen: number): void {
    this.postChain = this.postChain.then(async () => {
      if (gen <= this.abortedGen) return;
      try {
        const headers: Record<string, string> = {
          "Content-Type": "application/octet-stream",
          "X-Voice-Session": key,
        };
        if (end) headers["X-Voice-End"] = "1";
        const res = await fetch("/voice/in", {
          method: "POST",
          credentials: "same-origin",
          headers,
          body,
        });
        if (!res.ok && !end && gen === this.sessionGen && this.active) {
          this.failUpload(`Voice upload failed (${res.status}).`);
        }
      } catch {
        if (!end && gen === this.sessionGen && this.active) {
          this.failUpload("Voice upload failed.");
        }
      }
    });
  }

  private failUpload(msg: string): void {
    this.abortedGen = Math.max(this.abortedGen, this.sessionGen);
    this.sessionGen++;
    this.active = false;
    this.starting = false;
    this.sessionKey = "";
    this.pendingPcm = [];
    this.cleanupAudio();
    this.cb.setRecording(false);
    this.cb.setLevel(0);
    this.cb.showError(msg);
  }

  private cleanupAudio(): void {
    if (this.processor) {
      this.processor.disconnect();
      this.processor.onaudioprocess = null;
      this.processor = null;
    }
    if (this.audioCtx) {
      void this.audioCtx.close().catch(() => {});
      this.audioCtx = null;
    }
    stopStreamTracks(this.stream);
    this.stream = null;
  }
}
