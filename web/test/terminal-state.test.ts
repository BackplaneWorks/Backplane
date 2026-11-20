import { describe, expect, test } from "bun:test";
import { TerminalSwitchState } from "../src/viewers/terminal-state";

describe("TerminalSwitchState", () => {
  test("gates input while switching until selected shell snapshot resolves", () => {
    const st = new TerminalSwitchState();
    const g1 = st.beginSelect("t1");
    expect(st.canSendInput()).toBe(false);
    expect(st.resolveWatch(g1, "t1")).toBe(true);
    expect(st.canSendInput()).toBe(true);

    // Switching to s2 immediately gates input before s2's snapshot arrives.
    const g2 = st.beginSelect("s2");
    expect(st.activeKey).toBe("s2");
    expect(st.canSendInput()).toBe(false);

    // A stale watch reply for t1 is rejected and does not unlock input.
    expect(st.resolveWatch(g1, "t1")).toBe(false);
    expect(st.canSendInput()).toBe(false);

    // Once s2's snapshot resolves, input unlocks for s2.
    expect(st.resolveWatch(g2, "s2")).toBe(true);
    expect(st.canSendInput()).toBe(true);
  });

  test("delayed + New reply does not override a subsequent explicit shell selection", () => {
    const st = new TerminalSwitchState();
    const g1 = st.beginSelect("t1");
    st.resolveWatch(g1, "t1");
    const g2 = st.beginSelect("s2");
    st.resolveWatch(g2, "s2");

    // User clicks + New (in-flight term.new)
    const gNew = st.beginSelect();
    expect(st.canSendInput()).toBe(false);

    // Before + New resolves, user explicitly selects t1 from the dropdown.
    const gPick = st.beginSelect("t1");
    expect(st.resolveWatch(gPick, "t1")).toBe(true);
    expect(st.activeKey).toBe("t1");
    expect(st.canSendInput()).toBe(true);

    // Delayed + New reply arrives with s3: must be remembered in picker,
    // must NOT override activeKey ("t1"), and must NOT disrupt inputReady.
    expect(st.resolveNew(gNew, "s3")).toBe(false);
    expect(st.activeKey).toBe("t1");
    expect(st.canSendInput()).toBe(true);
    expect(st.knownKeys).toContain("s3");
  });
});
