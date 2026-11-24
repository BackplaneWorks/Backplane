import { describe, expect, test } from "bun:test";
import { needAfter, needPress, parseNeed } from "../src/swarm-need";

// A swarm row's need as native/host/src/present/swarm_need.rs writes it (`Need::json`), and what each press does here.
const row = {
  id: "h|r1", kind: "swarm", nav: "@room:h|r1", say: "Member gemini failed: Gemini sign-in expired",
  need: { line: "Member gemini failed: Gemini sign-in expired", why: "sign-in", actions: [
    { label: "Sign in to Gemini", act: "signin", room: "h|r1", thread: "h|b" },
    { label: "Move to Claude", act: "move", room: "h|r1", thread: "h|b" },
  ] },
};

describe("swarm need", () => {
  test("parses the hub's need; null and an older hub's row have none", () => {
    const n = parseNeed(row.need)!;
    expect(n.line).toBe("Member gemini failed: Gemini sign-in expired");
    expect(n.why).toBe("sign-in");
    expect(n.actions.map((a) => [a.act, a.room, a.thread])).toEqual([["signin", "h|r1", "h|b"], ["move", "h|r1", "h|b"]]);
    expect(parseNeed(null)).toBeNull();
    expect(parseNeed(undefined)).toBeNull();
  });
  test("a hub action is swarm.act, the thread only when there is one", () => {
    expect(needPress(row.need.actions[0], false)).toEqual({ kind: "call", method: "swarm.act", params: { room: "h|r1", action: "signin", thread: "h|b" } });
    for (const act of ["resume", "raise", "retry", "move"]) {
      expect(needPress({ label: act, act, room: "r1", thread: "" }, false)).toEqual({ kind: "call", method: "swarm.act", params: { room: "r1", action: act } });
    }
  });
  test("open opens the member; board needs a room view; an unknown act does nothing", () => {
    expect(needPress({ label: "Answer", act: "open", room: "r1", thread: "h|a" }, false)).toEqual({ kind: "thread", id: "h|a" });
    expect(needPress({ label: "Answer", act: "open", room: "r1", thread: "" }, false)).toBeNull();
    expect(needPress({ label: "Read board", act: "board", room: "r1", thread: "" }, false)).toBeNull();
    expect(needPress({ label: "Read board", act: "board", room: "r1", thread: "" }, true)).toEqual({ kind: "board", room: "r1" });
    expect(needPress({ label: "?", act: "later", room: "r1", thread: "" }, true)).toBeNull();
  });
  test("the reply opens the sign-in it names", () => {
    expect(needAfter({ open: "account:p7" })).toEqual({ kind: "account", id: "p7" });
    expect(needAfter({ open: "thread:t9" })).toEqual({ kind: "thread", id: "t9" });
    expect(needAfter({ ok: true })).toBeNull();
    expect(needAfter({ open: "account:" })).toBeNull();
    expect(needAfter(null)).toBeNull();
  });
});
