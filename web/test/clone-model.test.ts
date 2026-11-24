import { describe, expect, test } from "bun:test";
import { goesAtOnce, isNewer, newJobId, type CloneJob } from "../src/clone-model";

const job = (state: string, ended = 0, extra: Partial<CloneJob> = {}): CloneJob => ({ id: "w1-a", state, ended, ...extra });

describe("newJobId", () => {
  test("only [A-Za-z0-9_-], at most 64, distinct", () => {
    const a = newJobId();
    const b = newJobId();
    expect(a).toMatch(/^[A-Za-z0-9_-]{1,64}$/);
    expect(a).not.toBe(b);
    expect(newJobId(1700000000000, () => 0.5)).toMatch(/^w1700000000000-[a-z0-9]+$/);
  });
});

describe("isNewer", () => {
  test("nothing held, or another job: taken", () => {
    expect(isNewer(null, job("running"))).toBe(true);
    expect(isNewer({ id: "old", state: "failed", ended: 9 }, job("running"))).toBe(true);
  });
  test("a terminal state beats running; running never beats it", () => {
    for (const t of ["done", "failed", "cancelled"]) {
      expect(isNewer(job("running"), job(t, 5))).toBe(true);
      expect(isNewer(job(t, 5), job("running"))).toBe(false);
    }
  });
  test("the late start reply (running) after the done push is dropped", () => {
    const pushed = job("done", 10, { thread: "t9" });
    expect(isNewer(pushed, job("running", 0, { phase: "Starting" }))).toBe(false);
  });
  test("same state: later ended wins, a tie takes the later frame", () => {
    expect(isNewer(job("running", 0, { pct: 10 }), job("running", 0, { pct: 20 }))).toBe(true);
    expect(isNewer(job("done", 10), job("done", 10))).toBe(true);
    expect(isNewer(job("done", 10), job("done", 9))).toBe(false);
    expect(isNewer(job("failed", 4), job("cancelled", 6))).toBe(true);
  });
});

describe("goesAtOnce", () => {
  test("done with no notes goes; notes hold; the hub's hold decides when it is sent", () => {
    expect(goesAtOnce(job("done"))).toBe(true);
    expect(goesAtOnce(job("done", 1, { notes: ["x"] }))).toBe(false);
    expect(goesAtOnce(job("done", 1, { notes: ["x"], hold: false }))).toBe(true);
    expect(goesAtOnce(job("done", 1, { hold: true }))).toBe(false);
    expect(goesAtOnce(job("failed"))).toBe(false);
  });
});
