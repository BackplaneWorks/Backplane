import { describe, expect, test } from "bun:test";
import {
  applyFavoriteKeysToSnapshot,
  buildPickerViewModel,
  MAX_FAVORITES,
  normalizeFavKey,
  parsePickerResponse,
  toggleFavoriteKey,
} from "../src/model-picker-model";

const SAMPLE_PICKER = {
  picker: {
    chip: "claude / opus / high",
    favoriteKeys: ["claude/sonnet", "codex/gpt-6.1-sol"],
    favorites: [
      {
        key: "claude/sonnet",
        provider: "claude",
        model: "sonnet",
        label: "sonnet",
        note: "Sonnet 5 · claude-sonnet-5",
        on: false,
        ok: true,
        why: "",
      },
      {
        key: "codex/gpt-6.1-sol",
        provider: "codex",
        model: "gpt-6.1-sol",
        label: "gpt-6.1-sol",
        note: "GPT-6.1-Sol",
        on: false,
        ok: true,
        why: "",
      },
    ],
    agents: {
      claude: {
        ok: true,
        why: "",
        models: [
          { label: "default", value: "", note: "Claude Code's own default" },
          { label: "opus", value: "opus", note: "Opus 5.5 · claude-opus-5-5" },
          { label: "sonnet", value: "sonnet", note: "Sonnet 5 · claude-sonnet-5" },
        ],
      },
      codex: {
        ok: true,
        why: "",
        models: [
          { label: "default", value: "", note: "Codex's configured default" },
          { label: "gpt-6.1-sol", value: "gpt-6.1-sol", note: "GPT-6.1-Sol" },
        ],
      },
      grok: {
        ok: false,
        why: "not installed on this machine",
        models: [],
      },
    },
    sections: [
      {
        title: "Model",
        action: "model",
        choices: [
          { label: "default", value: "", on: false, ok: true, why: "" },
          { label: "Opus 5.5 · claude-opus-5-5", value: "opus", on: true, ok: true, why: "" },
          { label: "Sonnet 5 · claude-sonnet-5", value: "sonnet", on: false, ok: true, why: "" },
        ],
      },
      {
        title: "Effort",
        action: "effort",
        choices: [
          { label: "auto", value: "", on: false, ok: true, why: "" },
          { label: "high", value: "high", on: true, ok: true, why: "" },
        ],
      },
      {
        title: "Provider",
        action: "provider",
        choices: [
          { label: "claude", value: "claude", on: true, ok: true, why: "" },
          { label: "codex", value: "codex", on: false, ok: true, why: "" },
          { label: "grok", value: "grok", on: false, ok: false, why: "not installed on this machine" },
        ],
      },
    ],
  },
};

describe("Web model picker & favorites", () => {
  test("normalizeFavKey and toggleFavoriteKey normalize, deduplicate, and preserve order", () => {
    expect(normalizeFavKey(" @Claude ", " opus ")).toBe("claude/opus");
    expect(normalizeFavKey("", "opus")).toBe("");
    expect(normalizeFavKey("claude", "")).toBe("");

    const a = toggleFavoriteKey([], "@Claude", "opus");
    expect(a).toEqual({ keys: ["claude/opus"], starred: true });
    const b = toggleFavoriteKey(a.keys, "codex", "gpt-6.1-sol", true);
    expect(b).toEqual({ keys: ["claude/opus", "codex/gpt-6.1-sol"], starred: true });
    const c = toggleFavoriteKey(b.keys, "claude", "opus");
    expect(c).toEqual({ keys: ["codex/gpt-6.1-sol"], starred: false });
  });

  test("parsePickerResponse and buildPickerViewModel latch favorites at top and mark starrable rows", () => {
    const snap = parsePickerResponse(SAMPLE_PICKER);
    expect(snap.provider).toBe("claude");
    expect(snap.model).toBe("opus");
    expect(snap.effort).toBe("high");
    expect(snap.favoriteKeys).toEqual(["claude/sonnet", "codex/gpt-6.1-sol"]);

    const vm = buildPickerViewModel(snap, "");
    expect(vm.favorites.map((f) => `${f.provider}/${f.model}`)).toEqual(["claude/sonnet", "codex/gpt-6.1-sol"]);
    expect(vm.groups.map((g) => g.provider)).toEqual(["claude", "codex", "grok"]);

    const claudeRows = vm.groups[0].models;
    expect(claudeRows[0].starrable).toBe(false);
    expect(claudeRows[0].starred).toBe(false);
    expect(claudeRows[1].starrable).toBe(true);
    expect(claudeRows[1].on).toBe(true);
    expect(claudeRows[1].starred).toBe(false);
    expect(claudeRows[2].starred).toBe(true);
  });

  test("applyFavoriteKeysToSnapshot immediately updates Favorites section and search filters both sections", () => {
    const snap = parsePickerResponse(SAMPLE_PICKER);
    const updated = applyFavoriteKeysToSnapshot(snap, ["claude/opus", "codex/gpt-6.1-sol"]);
    expect(updated.favorites.map((f) => f.key)).toEqual(["claude/opus", "codex/gpt-6.1-sol"]);
    expect(updated.favorites[0].on).toBe(true);
    expect(updated.favorites[0].note).toBe("Opus 5.5 · claude-opus-5-5");

    const filtered = buildPickerViewModel(updated, "codex");
    expect(filtered.favorites.map((f) => f.key)).toEqual(["codex/gpt-6.1-sol"]);
    expect(filtered.groups.map((g) => g.provider)).toEqual(["codex"]);
  });

  test("rejects control chars, provider whitespace, overlong keys, 64-cap overflow, and unavailable models", () => {
    expect(normalizeFavKey("cl aude", "opus")).toBe("");
    expect(normalizeFavKey("claude", "opus\n")).toBe("");
    expect(normalizeFavKey("claude", "m".repeat(194))).toBe("");
    expect(normalizeFavKey("claude", "m".repeat(193)).length).toBe(200);

    const sixtyFour = Array.from({ length: MAX_FAVORITES }, (_, i) => `claude/m${i}`);
    const overflow = toggleFavoriteKey(sixtyFour, "codex", "gpt-6.1-sol", true);
    expect(overflow.starred).toBe(false);
    expect(overflow.keys).toHaveLength(MAX_FAVORITES);
    expect(overflow.keys).not.toContain("codex/gpt-6.1-sol");

    const snap = parsePickerResponse(SAMPLE_PICKER);
    const updated = applyFavoriteKeysToSnapshot(snap, ["claude/opus", "claude/retired"]);
    expect(updated.favorites[0].ok).toBe(true);
    expect(updated.favorites[1].ok).toBe(false);
    expect(updated.favorites[1].why).toBe("not available on this machine");
  });
});
