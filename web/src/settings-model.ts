// Pure Web Settings model: taxonomy, search/filter, and browser preference resolution.
// Zero DOM imports so it can be unit tested directly with bun test.

export type WebThemePref = "auto" | "dark" | "light";
export type WebLocalePref = "auto" | "en" | "es" | "ja";
export type SettingsPageId = "appearance" | "providers" | "devices" | "home";
export type SettingsGroupId = "general" | "ai" | "connect";

export interface SettingsGroupMeta {
  id: SettingsGroupId;
  label: string;
}

export interface SettingsPageMeta {
  id: SettingsPageId;
  group: SettingsGroupId;
  title: string;
  subtitle: string;
  icon: "sun" | "cpu" | "link" | "globe";
  keywords: string[];
}

export interface SettingsSearchItem {
  pageId: SettingsPageId;
  pageTitle: string;
  groupLabel: string;
  rowTitle: string;
  rowDetail: string;
  keywords?: string[];
}

export interface SettingsNavMatch {
  page: SettingsPageMeta;
  hits: string[];
}

export const SETTINGS_GROUPS: readonly SettingsGroupMeta[] = [
  { id: "general", label: "General" },
  { id: "ai", label: "AI & tools" },
  { id: "connect", label: "Connect" },
];

export const SETTINGS_PAGES: readonly SettingsPageMeta[] = [
  {
    id: "appearance",
    group: "general",
    title: "Appearance",
    subtitle: "Kept in this browser only. Never sent to the hub.",
    icon: "sun",
    keywords: ["theme", "dark", "light", "auto", "color", "language", "locale", "english", "espanol", "japanese"],
  },
  {
    id: "providers",
    group: "ai",
    title: "AI providers",
    subtitle: "Account sign-ins live in the desktop or mobile app.",
    icon: "cpu",
    keywords: ["accounts", "claude", "codex", "antigravity", "grok", "openai", "anthropic", "signin", "login", "model"],
  },
  {
    id: "devices",
    group: "connect",
    title: "Devices & browser",
    subtitle: "This browser session and paired devices on this hub.",
    icon: "link",
    keywords: ["sign out", "logout", "session", "cookie", "revoke", "paired", "phone", "browser", "pair"],
  },
  {
    id: "home",
    group: "connect",
    title: "Home machine",
    subtitle: "Which machine hosts your always-on hub and bots.",
    icon: "globe",
    keywords: ["host", "boat", "move", "online", "away", "queued", "linked"],
  },
];

export function parseThemePref(raw: string | null | undefined): WebThemePref {
  if (raw === "dark" || raw === "light" || raw === "auto") return raw;
  return "auto";
}

export function parseLocalePref(raw: string | null | undefined): WebLocalePref {
  if (raw === "en" || raw === "es" || raw === "ja" || raw === "auto") return raw;
  return "auto";
}

export function resolveTheme(pref: WebThemePref, hubTheme?: string | null): "dark" | "light" {
  if (pref === "dark" || pref === "light") return pref;
  return hubTheme === "light" ? "light" : "dark";
}

function splitTerms(query: string): string[] {
  return query.trim().toLowerCase().split(/\s+/).filter(Boolean);
}

function matchesAllTerms(haystack: string, terms: string[]): boolean {
  if (terms.length === 0) return true;
  const low = haystack.toLowerCase();
  return terms.every((term) => low.includes(term));
}

export function filterSearchItems(
  items: readonly SettingsSearchItem[],
  query: string,
): SettingsSearchItem[] {
  const terms = splitTerms(query);
  if (terms.length === 0) return [];
  return items.filter((item) => {
    const haystack = [
      item.groupLabel,
      item.pageTitle,
      item.rowTitle,
      item.rowDetail,
      ...(item.keywords ?? []),
    ].join(" ");
    return matchesAllTerms(haystack, terms);
  });
}

export function filterSettingsNav(
  pages: readonly SettingsPageMeta[],
  items: readonly SettingsSearchItem[],
  query: string,
): SettingsNavMatch[] {
  const terms = splitTerms(query);
  if (terms.length === 0) {
    return pages.map((page) => ({ page, hits: [] }));
  }

  const groupLabelById = new Map(SETTINGS_GROUPS.map((g) => [g.id, g.label]));
  const out: SettingsNavMatch[] = [];

  for (const page of pages) {
    const pageItems = items.filter((it) => it.pageId === page.id);
    const matchedRows = pageItems.filter((it) =>
      matchesAllTerms(
        [it.rowTitle, it.rowDetail, it.pageTitle, it.groupLabel, ...(it.keywords ?? [])].join(" "),
        terms,
      ),
    );
    const pageMatches = matchesAllTerms(
      [groupLabelById.get(page.group) ?? "", page.title, page.subtitle, ...page.keywords].join(" "),
      terms,
    );

    if (pageMatches || matchedRows.length > 0) {
      const hits: string[] = [];
      for (const r of matchedRows) {
        if (!hits.includes(r.rowTitle)) hits.push(r.rowTitle);
        if (hits.length >= 2) break;
      }
      out.push({ page, hits });
    }
  }
  return out;
}
