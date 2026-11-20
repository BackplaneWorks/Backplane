// i18n helper backed by build-time filtered catalogs from i18n/{en,es,ja}.msg.
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore virtual module injected by web/build.ts
import { CATALOGS } from "virtual:i18n";

let currentLocale = detectLocale();

// Reverse lookup: English string -> catalog key (mirrors GPUI's trs() for hub-supplied labels & whys).
const EN_TO_KEY = new Map<string, string>();
for (const [k, v] of Object.entries(CATALOGS.en || {})) {
  if (typeof v === "string" && !EN_TO_KEY.has(v)) {
    EN_TO_KEY.set(v, k);
  }
}

function detectLocale(): string {
  const nav = (typeof navigator !== "undefined" && navigator.language) ? navigator.language.toLowerCase() : "en";
  if (nav.startsWith("ja")) return "ja";
  if (nav.startsWith("es")) return "es";
  return "en";
}

export function setLocale(locale: string | undefined | null): void {
  if (!locale) return;
  const low = locale.toLowerCase();
  if (low.startsWith("ja")) currentLocale = "ja";
  else if (low.startsWith("es")) currentLocale = "es";
  else if (low.startsWith("en")) currentLocale = "en";
}

export function getLocale(): string {
  return currentLocale;
}

export function t(key: string, fallback?: string, vars?: Record<string, string | number>): string {
  const cat = CATALOGS[currentLocale] || CATALOGS.en || {};
  let text = cat[key] ?? CATALOGS.en?.[key] ?? fallback ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
  }
  return text;
}

export function trs(english: string): string {
  if (!english) return "";
  const key = EN_TO_KEY.get(english);
  if (!key) return english;
  return t(key, english);
}
