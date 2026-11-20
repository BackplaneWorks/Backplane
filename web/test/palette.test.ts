import { describe, expect, test } from "bun:test";
import {
  buildPaletteItems,
  filterPaletteItems,
  markRange,
  rankLabel,
  type PaletteState,
} from "../src/palette-model";

function sampleState(): PaletteState {
  return {
    selectedThreadId: "t1",
    projects: [
      { id: "p1", title: "Demo Board" },
      { id: "p2", title: "Firmware" },
    ],
    threads: [
      { id: "t1", project: "p1", title: "Board layout review", status: "active" },
      { id: "t2", project: "p1", title: "Power budget", status: "waiting", waiting: 1 },
      { id: "t3", project: "p2", title: "SPI driver", status: "finished" },
    ],
    bots: [{ id: "b1", name: "Helper", thread: "t-bot" }],
    railItems: [
      { id: "board", label: "Board", icon: "circuit-board", key: "b", state: "off" },
      { id: "schematic", label: "Schematic", icon: "cpu", key: "s", state: "disabled", why: "No schematic" },
      { id: "terminal", label: "Terminal", icon: "terminal", group: "tools", key: "t", state: "off" },
      { id: "commands", label: "Commands", icon: "search", group: "tools", state: "off" },
    ],
    commands: [
      { word: "/btw", label: "Side question", desc: "Ask without interrupting" },
      { word: "/settle", label: "Settle thread", desc: "Mark thread settled" },
    ],
  };
}

describe("Command Palette pure logic", () => {
  test("rankLabel scores prefix > word > subsequence > miss", () => {
    expect(rankLabel("Board layout", "")).toBe(1);
    expect(rankLabel("Board layout", "boa")).toBe(3);
    expect(rankLabel("Board layout", "lay")).toBe(2);
    expect(rankLabel("Board layout", "blt")).toBe(1);
    expect(rankLabel("Board layout", "xyz")).toBe(0);
  });

  test("buildPaletteItems excludes disabled rail entries and commands itself", () => {
    const items = buildPaletteItems(sampleState());
    const railIds = items
      .filter((i) => i.action.type === "rail")
      .map((i) => (i.action as { type: "rail"; id: string }).id);
    expect(railIds).toEqual(["board", "terminal"]);

    const cmds = items.filter((i) => i.action.type === "command");
    expect(cmds.length).toBe(2);
    expect(cmds.find((c) => c.label.startsWith("/settle"))?.action).toEqual({
      type: "command",
      word: "/settle",
      runDirectly: true,
    });
    expect(cmds.find((c) => c.label.startsWith("/btw"))?.action).toEqual({
      type: "command",
      word: "/btw",
      runDirectly: false,
    });
  });

  test("empty query surfaces waiting threads first", () => {
    const state = sampleState();
    const list = filterPaletteItems(buildPaletteItems(state), "", null, state.projects);
    expect(list[0].label).toBe("Power budget");
    expect(list[0].want).toBe("waiting");
  });

  test("filterPaletteItems merges hub search thread hits and message hits without duplicating local threads", () => {
    const state = sampleState();
    const all = buildPaletteItems(state);
    const list = filterPaletteItems(
      all,
      "power",
      {
        threads: [
          { id: "t2", project: "p1", title: "Power budget", status: "waiting" },
          { id: "t99", project: "p1", title: "Power rail measurements", status: "finished" },
        ],
        messages: [
          {
            thread: "t99",
            title: "Power rail measurements",
            project: "Demo Board",
            role: "assistant",
            text: "Measured 3.3V power rail ripple at 12 mV.",
          },
        ],
      },
      state.projects,
    );

    const threadIds = list
      .filter((i) => i.kind === "thread" && i.action.type === "thread")
      .map((i) => (i.action as { type: "thread"; id: string }).id);
    expect(threadIds).toEqual(["t2", "t99"]);

    const msgHit = list.find((i) => i.kind === "agent said");
    expect(msgHit?.label).toBe("Measured 3.3V power rail ripple at 12 mV.");
    expect(msgHit?.place).toBe("Power rail measurements · Demo Board");
  });

  test("markRange highlights the first query word", () => {
    expect(markRange("Measured 3.3V power rail", "power rail")).toEqual([14, 19]);
    expect(markRange("Hello world", "missing")).toBeNull();
  });
});
