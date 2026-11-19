// i18n helper backed by build-time filtered catalogs from i18n/{en,es,ja}.msg.
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore virtual module injected by web/build.ts
import { CATALOGS } from "virtual:i18n";

let currentLocale = detectLocale();

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
