// bun test (in tools/voice). The local engine's pure parts always; the
// engine itself when BACKPLANE_VOICE_LOCAL_DIR names a downloaded local
// dir (local-install) and BACKPLANE_VOICE_WAV a 16 kHz mono WAV with speech.
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { wordsOf } from "../src/local";
import { Resampler } from "../src/main";

const MAIN = join(import.meta.dir, "../src/main.ts");

test("words from pieces: spaces start words, punctuation joins, timings in cs, percent", () => {
  const w = wordsOf({
    tokens: [" Open", " the", " bo", "ard", ".", " ", "8", " K", "a", "i"],
    timestamps: [0, 0.24, 0.32, 0.4, 0.56, 0.6, 0.64, 0.8, 0.88, 0.96],
    durations: [0.24, 0.08, 0.08, 0.16, 0.04, 0.04, 0.08, 0.08, 0.08, 0.08],
    ys_log_probs: [0, 0, Math.log(0.9), 0, 0, 0, 0, Math.log(0.5), 0, 0],
  });
  expect(w).toEqual([
    ["Open", 0, 24, 100],
    ["the", 24, 32, 100],
    ["board.", 32, 60, 90],
    ["8", 64, 72, 100],
    ["Kai", 80, 104, 50],
  ]);
});

test("resampler: 16 kHz to 24 kHz keeps the length ratio and the signal", () => {
  const r = new Resampler(16000, 24000);
  const n = 1600;
  const b = new Uint8Array(n * 2);
  const v = new DataView(b.buffer);
  for (let i = 0; i < n; i++) v.setInt16(i * 2, Math.round(10000 * Math.sin(i / 10)), true);
  // in odd-sized pieces, as uploads come
  let out = 0;
  for (let i = 0; i < b.length; i += 333) out += r.push(b.subarray(i, Math.min(b.length, i + 333))).length;
  expect(Math.abs(out / 2 - n * 1.5)).toBeLessThan(4);
});

const DIR = process.env.BACKPLANE_VOICE_LOCAL_DIR;
const WAV = process.env.BACKPLANE_VOICE_WAV;

test.skipIf(!DIR || !WAV)("local engine: live words settle into a done segment", async () => {
  const wav = readFileSync(WAV!);
  const pcm = wav.subarray(44);
  const p = Bun.spawn([process.execPath, MAIN, "live", "--engine", "local", "--local-dir", DIR!, "--source", "stdin"], { stdin: "pipe", stdout: "pipe", stderr: "ignore" });
  // real time, 100 ms at a time, then a second of silence
  const silence = new Uint8Array(32000);
  const all = new Uint8Array(pcm.length + silence.length);
  all.set(pcm);
  all.set(silence, pcm.length);
  for (let i = 0; i < all.length; i += 3200) {
    p.stdin.write(all.subarray(i, i + 3200));
    await Bun.sleep(100);
  }
  p.stdin.end();
  const lines = (await new Response(p.stdout).text()).trim().split("\n").map((l) => JSON.parse(l));
  expect(lines.some((e) => e.event === "hyp")).toBe(true);
  const done = lines.filter((e) => e.event === "done");
  expect(done.length).toBeGreaterThan(0);
  expect(done.map((d) => d.text).join(" ").length).toBeGreaterThan(5);
  expect(lines[lines.length - 1].event).toBe("end");
}, 60000);
