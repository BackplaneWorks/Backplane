import { describe, expect, mock, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

mock.module("virtual:i18n", () => ({ CATALOGS: {} }));
const { statusPillOf } = await import("../src/sidebar");

const root = join(import.meta.dir, "..");
const sidebarSrc = readFileSync(join(root, "src/sidebar.ts"), "utf8");
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
    expect(sidebarSrc).toContain("const hasSettle = Boolean(row.settle);");
    expect(sidebarSrc).toContain("proj.hasBoard || proj.hasSchematic");
    expect(sidebarSrc).toContain('data-testid", "all-projects-filter"');
  });

  test("Quick-open hardware buttons stay hidden at rest and reveal on hover", () => {
    expect(shellCss).toContain("button.bp-proj-quick {\n  display: none;");
    expect(shellCss).toContain(".bp-proj-head:hover button.bp-proj-quick");
    expect(sidebarSrc).not.toContain('className = "icon bp-proj-quick"');
  });
});
