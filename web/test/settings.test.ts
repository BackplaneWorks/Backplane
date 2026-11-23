import { describe, expect, test } from "bun:test";
import {
  SETTINGS_GROUPS,
  SETTINGS_PAGES,
  filterSearchItems,
  filterSettingsNav,
  parseLocalePref,
  parseThemePref,
  resolveTheme,
  type SettingsSearchItem,
} from "../src/settings-model";

const SAMPLE_ITEMS: SettingsSearchItem[] = [
  {
    pageId: "appearance",
    pageTitle: "Appearance",
    groupLabel: "General",
    rowTitle: "Theme",
    rowDetail: "Auto (follow hub), Dark, or Light in this browser.",
    keywords: ["dark", "light", "auto"],
  },
  {
    pageId: "appearance",
    pageTitle: "Appearance",
    groupLabel: "General",
    rowTitle: "Language",
    rowDetail: "Interface language for this browser.",
    keywords: ["locale", "english", "espanol", "japanese"],
  },
  {
    pageId: "providers",
    pageTitle: "AI providers",
    groupLabel: "AI & tools",
    rowTitle: "Provider connections",
    rowDetail: "Connect Claude, Codex, Antigravity, or other AI providers.",
    keywords: ["accounts", "claude", "codex"],
  },
  {
    pageId: "devices",
    pageTitle: "Devices & browser",
    groupLabel: "Connect",
    rowTitle: "This browser",
    rowDetail: "Sign out and revoke this browser's session cookie.",
    keywords: ["logout", "sign out"],
  },
  {
    pageId: "devices",
    pageTitle: "Devices & browser",
    groupLabel: "Connect",
    rowTitle: "Pixel 9",
    rowDetail: "Paired device",
    keywords: ["paired", "device"],
  },
  {
    pageId: "home",
    pageTitle: "Home machine",
    groupLabel: "Connect",
    rowTitle: "Workstation",
    rowDetail: "Which machine hosts your always-on hub and bots.",
    keywords: ["boat", "move", "online"],
  },
];

describe("Web Settings model", () => {
  test("defines clean grouped pages with valid group references", () => {
    const groupIds = new Set(SETTINGS_GROUPS.map((g) => g.id));
    expect(SETTINGS_GROUPS.map((g) => g.label)).toEqual(["General", "AI & tools", "Connect"]);
    expect(SETTINGS_PAGES.map((p) => p.id)).toEqual(["appearance", "providers", "devices", "home"]);
    for (const p of SETTINGS_PAGES) {
      expect(groupIds.has(p.group)).toBe(true);
    }
  });

  test("filterSettingsNav returns all pages with no hits when query is empty", () => {
    const res = filterSettingsNav(SETTINGS_PAGES, SAMPLE_ITEMS, "   ");
    expect(res.map((m) => m.page.id)).toEqual(["appearance", "providers", "devices", "home"]);
    expect(res.every((m) => m.hits.length === 0)).toBe(true);
  });

  test("filterSettingsNav finds pages by row title, detail, group, or keyword and surfaces matching hits", () => {
    const themeMatch = filterSettingsNav(SETTINGS_PAGES, SAMPLE_ITEMS, "light");
    expect(themeMatch.map((m) => m.page.id)).toEqual(["appearance"]);
    expect(themeMatch[0].hits).toEqual(["Theme"]);

    const signOutMatch = filterSettingsNav(SETTINGS_PAGES, SAMPLE_ITEMS, "sign out");
    expect(signOutMatch.map((m) => m.page.id)).toEqual(["devices"]);
    expect(signOutMatch[0].hits).toEqual(["This browser"]);

    const deviceMatch = filterSettingsNav(SETTINGS_PAGES, SAMPLE_ITEMS, "pixel");
    expect(deviceMatch.map((m) => m.page.id)).toEqual(["devices"]);
    expect(deviceMatch[0].hits).toEqual(["Pixel 9"]);

    const providerMatch = filterSettingsNav(SETTINGS_PAGES, SAMPLE_ITEMS, "claude");
    expect(providerMatch.map((m) => m.page.id)).toEqual(["providers"]);

    const connectGroupMatch = filterSettingsNav(SETTINGS_PAGES, SAMPLE_ITEMS, "connect");
    expect(connectGroupMatch.some((m) => m.page.id === "devices")).toBe(true);
    expect(connectGroupMatch.some((m) => m.page.id === "home")).toBe(true);

    expect(filterSettingsNav(SETTINGS_PAGES, SAMPLE_ITEMS, "nonexistent-xyz")).toEqual([]);
  });

  test("filterSearchItems requires all space-separated terms", () => {
    const hits = filterSearchItems(SAMPLE_ITEMS, "browser cookie");
    expect(hits.map((h) => h.rowTitle)).toEqual(["This browser"]);
    expect(filterSearchItems(SAMPLE_ITEMS, "")).toEqual([]);
  });

  test("resolveTheme and preference parsers handle auto, explicit overrides, and invalid inputs", () => {
    expect(parseThemePref(null)).toBe("auto");
    expect(parseThemePref("bogus")).toBe("auto");
    expect(parseThemePref("light")).toBe("light");
    expect(parseThemePref("dark")).toBe("dark");

    expect(parseLocalePref(undefined)).toBe("auto");
    expect(parseLocalePref("fr")).toBe("auto");
    expect(parseLocalePref("ja")).toBe("ja");
    expect(parseLocalePref("es")).toBe("es");

    expect(resolveTheme("auto", "light")).toBe("light");
    expect(resolveTheme("auto", "dark")).toBe("dark");
    expect(resolveTheme("auto", undefined)).toBe("dark");
    expect(resolveTheme("light", "dark")).toBe("light");
    expect(resolveTheme("dark", "light")).toBe("dark");
  });
});
