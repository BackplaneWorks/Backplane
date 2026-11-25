import { describe, expect, mock, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

mock.module("virtual:i18n", () => ({ CATALOGS: {} }));
const { statusPillOf, isSwarmVisibleUnderProject, moveTargets, homeBannerState, CHATS, SIDEBAR_CACHE_KEY, loadSidebarCache, saveSidebarCache, clearSidebarCache } = await import("../src/sidebar");

const root = join(import.meta.dir, "..");
const sidebarSrc = readFileSync(join(root, "src/sidebar.ts"), "utf8");
const appSrc = readFileSync(join(root, "src/app.ts"), "utf8");
const shellCss = readFileSync(join(root, "src/shell.css"), "utf8");

describe("SidebarView Crew / Inbox / Bench + Status Pills + Snooze", () => {
  test("statusPillOf produces unmistakable icon+word pills from hub label/tone and word/want", () => {
    expect(statusPillOf({ label: "Approve", tone: "bad" })).toEqual({ icon: "!", label: "Approve", tone: "bad" });
    expect(statusPillOf({ label: "Input", tone: "bad" })).toEqual({ icon: "?", label: "Input", tone: "bad" });
    expect(statusPillOf({ label: "Failed", tone: "bad" })).toEqual({ icon: "×", label: "Failed", tone: "bad-out" });
    expect(statusPillOf({ label: "Working", tone: "run" })).toEqual({ icon: "▸", label: "Working", tone: "run" });
    expect(statusPillOf({ label: "Waiting", tone: "wait" })).toEqual({ icon: "◌", label: "Waiting", tone: "wait" });
    expect(statusPillOf({ label: "Done", tone: "ok" })).toEqual({ icon: "✓", label: "Done", tone: "ok" });
    expect(statusPillOf({ wake: "t1|off" })).toEqual({ icon: "◷", label: "Snoozed", tone: "dim" });
    // Fallback without hub label/tone
    expect(statusPillOf({ word: "needs", want: "question" })).toEqual({ icon: "?", label: "Input", tone: "bad" });
    expect(statusPillOf({ word: "running" })).toEqual({ icon: "▸", label: "Working", tone: "run" });
    expect(statusPillOf({ word: "done" })).toEqual({ icon: "✓", label: "Done", tone: "ok" });
    expect(statusPillOf({ word: "idle" })).toBeNull();
  });

  test("Quick chats: New chat in the header and the Chats head, Move to project on a chat row, own projects only", () => {
    expect(CHATS).toBe("chats");
    expect(sidebarSrc).toContain('"sidebar-new-chat"');
    expect(sidebarSrc).toContain('"section-new-chat"');
    expect(sidebarSrc).toContain("`move-row-${row.id}`");
    const got = moveTargets([
      { id: "p2", title: "psu" }, { id: "chats", title: "Chats" }, { id: "scratch", title: "Scratch" }, { id: "L~p9", title: "far" }, { id: "p1", title: "Amp" },
    ]);
    expect(got.map((p) => p.id)).toEqual(["p1", "p2"]);
  });

  test("Crew band is mounted outside scrollable list and never appended after projects", () => {
    expect(sidebarSrc).toContain('this.crewEl.className = "bp-crew-band bp-hidden"');
    expect(sidebarSrc).toContain("this.el.append(hostRow, head, this.homeBanner, this.crewEl, this.listEl, footer)");
    expect(sidebarSrc).not.toContain("bp-bots-group");
  });

  test("Supports Running + foldable Snoozed sections, hover Snooze/Wake/Settle, and hardware icons", () => {
    expect(sidebarSrc).toContain('sec.id === "running"');
    expect(sidebarSrc).toContain("row.snooze");
    expect(sidebarSrc).toContain("row.wake");
    expect(sidebarSrc).toContain("this.cb.onSnoozeRow?.(opt.value)");
    expect(sidebarSrc).toContain("const hasSettle = Boolean(row.settle) && (!isSwarm || Boolean(this.cb.onSettleSwarm));");
    expect(sidebarSrc).toContain("isSwarmVisibleUnderProject");
    expect(sidebarSrc).toContain("proj.hasBoard || proj.hasSchematic");
    expect(sidebarSrc).toContain('data-testid", "all-projects-filter"');
  });

  test("isSwarmVisibleUnderProject hides idle/settled/finished swarms under collapsed projects", () => {
    expect(isSwarmVisibleUnderProject({ id: "r1", title: "Web UI", state: "active", working: 0 }, true)).toBe(false);
    expect(isSwarmVisibleUnderProject({ id: "r1", title: "Web UI", state: "active", working: 0 }, false)).toBe(true);
    expect(isSwarmVisibleUnderProject({ id: "r2", title: "Active", state: "active", working: 2 }, true)).toBe(true);
    expect(isSwarmVisibleUnderProject({ id: "r3", title: "Mon", state: "active", working: 0, monitoring: true }, true)).toBe(true);
    expect(isSwarmVisibleUnderProject({ id: "r4", title: "Paused", state: "paused", working: 1 }, true)).toBe(false);
    expect(isSwarmVisibleUnderProject({ id: "r5", title: "Needs", state: "paused", working: 0, need: { line: "Paused" } }, true)).toBe(true);
    expect(isSwarmVisibleUnderProject({ id: "r6", title: "WordNeeds", state: "active", word: "needs", working: 0 }, true)).toBe(true);
    expect(isSwarmVisibleUnderProject({ id: "r7", title: "Settled", state: "active", working: 0, settled: true }, false)).toBe(false);
    expect(isSwarmVisibleUnderProject({ id: "r8", title: "Done", state: "done", working: 0 }, false)).toBe(false);
    expect(isSwarmVisibleUnderProject({ id: "r9", title: "Stopped", state: "stopped", working: 0 }, false)).toBe(false);
  });

  test("Quick-open hardware buttons stay hidden at rest and reveal on hover", () => {
    expect(shellCss).toContain("button.bp-proj-quick {\n  display: none;");
    expect(shellCss).toContain(".bp-proj-head:hover button.bp-proj-quick");
    expect(sidebarSrc).not.toContain('className = "icon bp-proj-quick"');
  });

  test("homeBannerState marks only failed home moves as dismissible and wires dismiss RPC", () => {
    expect(homeBannerState(null)).toBeNull();
    expect(homeBannerState({ kind: "local", id: "m1", name: "desk", online: true, queuedCount: 0, canMoveHere: false })).toBeNull();
    expect(homeBannerState({
      kind: "peer", id: "m2", name: "boat", online: true, queuedCount: 0, canMoveHere: false,
      moving: { to: "boat", state: "moving", step: "copying" },
    })).toEqual({ text: "Moving home to boat: copying", isError: false, dismissible: false });
    expect(homeBannerState({
      kind: "peer", id: "m2", name: "boat", online: true, queuedCount: 0, canMoveHere: true,
      moving: { to: "boat", state: "failed", note: "that machine is away" },
    })).toEqual({ text: "Home move failed: that machine is away", isError: true, dismissible: true });
    expect(homeBannerState({
      kind: "peer", id: "m2", name: "boat", online: true, queuedCount: 0, canMoveHere: true,
      moving: { to: "boat", state: "failed" },
    })).toEqual({ text: "Home move failed", isError: true, dismissible: true });
    expect(homeBannerState({
      kind: "peer", id: "m2", name: "boat", online: false, queuedCount: 2, canMoveHere: false,
    })).toEqual({ text: "Home (boat) is away · 2 queued", isError: false, dismissible: false });
    expect(sidebarSrc).toContain('"home-move-dismiss"');
    expect(sidebarSrc).toContain("this.cb.onDismissHomeMove?.()");
    expect(appSrc).toContain('"home.move.dismiss"');
  });

  test("Sidebar cache round-trips, rejects empty/invalid entries, and clears on logout", () => {
    const store = new Map<string, string>();
    const storage = {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => { store.set(k, v); },
      removeItem: (k: string) => { store.delete(k); },
    };
    expect(loadSidebarCache(storage)).toBeNull();
    store.set(SIDEBAR_CACHE_KEY, "{bad json");
    expect(loadSidebarCache(storage)).toBeNull();
    store.set(SIDEBAR_CACHE_KEY, JSON.stringify({ v: 1, projects: [], threads: [], sections: [] }));
    expect(loadSidebarCache(storage)).toBeNull();

    saveSidebarCache(storage, {
      hostName: "desk",
      sections: [{ id: "pinned", kind: "projects", title: "Pinned", rows: [] }],
      projects: [{ id: "p1", title: "Amp" }],
      threads: [{ id: "t1", project: "p1", title: "Bringup", state: "idle", updatedAt: 100 }],
      bots: [{ id: "b1", name: "Bub", thread: "tb1", uber: true }],
      rooms: [{ id: "r1", name: "lab" }],
      shellHashes: { "p:p1": "abc", "@sections": "def" },
      openSectionFolds: ["sec:quiet"],
    });
    const loaded = loadSidebarCache(storage);
    expect(loaded).not.toBeNull();
    expect(loaded?.hostName).toBe("desk");
    expect(loaded?.shellHashes).toEqual({ "p:p1": "abc", "@sections": "def" });
    expect(loaded?.openSectionFolds).toEqual(["sec:quiet"]);
    clearSidebarCache(storage);
    expect(loadSidebarCache(storage)).toBeNull();
    expect(appSrc).toContain("loadSidebarCache(");
    expect(appSrc).toContain("clearSidebarCache(");
  });
});
