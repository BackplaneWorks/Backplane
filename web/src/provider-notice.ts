export interface NoProviderNotice {
  reason?: string;
  head: string;
  text: string;
  button: string;
  url: string;
}

export function parseNoProvider(raw: unknown): NoProviderNotice | null {
  let obj: Record<string, unknown> | null = null;
  if (typeof raw === "string") {
    const trimmed = raw.trim();
    if (!trimmed) return null;
    try {
      const parsed: unknown = JSON.parse(trimmed);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        obj = parsed as Record<string, unknown>;
      }
    } catch {
      return null;
    }
  } else if (raw && typeof raw === "object" && !Array.isArray(raw)) {
    obj = raw as Record<string, unknown>;
  }
  if (!obj) return null;
  const head = typeof obj.head === "string" ? obj.head.trim() : "";
  if (!head) return null;
  return {
    reason: typeof obj.reason === "string" ? obj.reason : "",
    head,
    text: typeof obj.text === "string" ? obj.text : "",
    button: typeof obj.button === "string" && obj.button.trim() ? obj.button : "Open Settings → Accounts",
    url: typeof obj.url === "string" && obj.url.trim() ? obj.url : "backplane://settings/accounts",
  };
}

export function isNoProviderError(msg: string): boolean {
  return msg.startsWith("No AI provider connected yet.");
}

export function isLocalNoProviderError(msg: string, code?: string): boolean {
  if (code === "no_provider") return true;
  if (code) return false;
  return isNoProviderError(msg);
}
