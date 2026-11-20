// i18n helper backed by build-time filtered catalogs from i18n/{en,es,ja}.msg.
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore virtual module injected by web/build.ts
import { CATALOGS } from "virtual:i18n";

let currentLocale = detectLocale();
const localeListeners = new Set<(locale: string) => void>();

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

function syncDocumentLang(): void {
  if (typeof document !== "undefined" && document.documentElement) {
    document.documentElement.lang = currentLocale;
  }
}

syncDocumentLang();

export function onLocaleChange(fn: (locale: string) => void): () => void {
  localeListeners.add(fn);
  return () => { localeListeners.delete(fn); };
}

export function setLocale(locale: string | undefined | null): boolean {
  if (!locale) return false;
  const low = locale.toLowerCase();
  const next = low.startsWith("ja") ? "ja" : low.startsWith("es") ? "es" : low.startsWith("en") ? "en" : "";
  if (!next) return false;
  const changed = next !== currentLocale;
  currentLocale = next;
  syncDocumentLang();
  if (changed) {
    for (const fn of localeListeners) fn(currentLocale);
  }
  return changed;
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
