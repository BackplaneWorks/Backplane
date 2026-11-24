import { describe, expect, test } from "bun:test";
import { splitUserModelRefs, type MessageRef } from "../src/user-refs";

describe("splitUserModelRefs", () => {
  const models: MessageRef[] = [
    { word: "&antigravity/gemini-4-neon", kind: "model", id: "", label: "Gemini 4 Neon" },
    { word: "&claude/claude-fable-5-2[1m]", kind: "model", id: "", label: "Fable 5.2 (1M)" },
    { word: "&claude/opus", kind: "model", id: "", label: "Opus" },
    { word: "&claude/opus[1m]", kind: "model", id: "", label: "Opus (1M)" },
  ];

  test("returns plain text when no model refs exist", () => {
    expect(splitUserModelRefs("hello &provider/model", [])).toEqual([
      { kind: "text", text: "hello &provider/model" },
    ]);
    expect(splitUserModelRefs("ping @bot", [{ word: "@bot", kind: "bot", label: "Bot" }])).toEqual([
      { kind: "text", text: "ping @bot" },
    ]);
  });

  test("embeds single, multiple, repeated, and [1m] model refs inline", () => {
    const input =
      "swarm with &antigravity/gemini-4-neon and &claude/claude-fable-5-2[1m], then &antigravity/gemini-4-neon.";
    expect(splitUserModelRefs(input, models)).toEqual([
      { kind: "text", text: "swarm with " },
      { kind: "model", word: "&antigravity/gemini-4-neon", label: "Gemini 4 Neon" },
      { kind: "text", text: " and " },
      { kind: "model", word: "&claude/claude-fable-5-2[1m]", label: "Fable 5.2 (1M)" },
      { kind: "text", text: ", then " },
      { kind: "model", word: "&antigravity/gemini-4-neon", label: "Gemini 4 Neon" },
      { kind: "text", text: "." },
    ]);
  });

  test("respects word boundaries and prefers longer [1m] suffix over bare prefix", () => {
    const input = "(&claude/opus[1m]) [&claude/opus] foo&claude/opus https://x.io?a=1&claude/opus &provider/model";
    expect(splitUserModelRefs(input, models)).toEqual([
      { kind: "text", text: "(" },
      { kind: "model", word: "&claude/opus[1m]", label: "Opus (1M)" },
      { kind: "text", text: ") [" },
      { kind: "model", word: "&claude/opus", label: "Opus" },
      { kind: "text", text: "] foo&claude/opus https://x.io?a=1&claude/opus &provider/model" },
    ]);
  });

  test("preserves multiline text verbatim around chips", () => {
    const input = "line 1 &antigravity/gemini-4-neon\n\nline 2";
    const segs = splitUserModelRefs(input, models);
    expect(segs.map((s) => (s.kind === "text" ? s.text : s.word)).join("")).toBe(input);
    expect(segs.filter((s) => s.kind === "model")).toHaveLength(1);
  });

  test("splits model ref before trailing colon while keeping colon tags non-chips", () => {
    expect(splitUserModelRefs("Use &claude/opus: it's fast", models)).toEqual([
      { kind: "text", text: "Use " },
      { kind: "model", word: "&claude/opus", label: "Opus" },
      { kind: "text", text: ": it's fast" },
    ]);
    const tagModels: MessageRef[] = [{ word: "&opencode/llama3", kind: "model", id: "", label: "Llama 3" }];
    expect(splitUserModelRefs("try &opencode/llama3:8b", tagModels)).toEqual([
      { kind: "text", text: "try &opencode/llama3:8b" },
    ]);
  });
});
