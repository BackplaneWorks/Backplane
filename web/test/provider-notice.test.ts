import { describe, expect, test } from "bun:test";
import { isNoProviderError, parseNoProvider } from "../src/provider-notice";

describe("provider-notice", () => {
  test("parses hello object and info JSON string payloads", () => {
    const obj = {
      reason: "none_installed",
      head: "No AI provider connected",
      text: "Connect Claude, Codex, Antigravity, or another provider to start chatting.",
      button: "Open Settings → Accounts",
      url: "backplane://settings/accounts",
    };
    expect(parseNoProvider(obj)).toEqual(obj);
    expect(parseNoProvider(JSON.stringify(obj))).toEqual(obj);
  });

  test("returns null for empty, null, or malformed payloads", () => {
    expect(parseNoProvider(null)).toBeNull();
    expect(parseNoProvider(undefined)).toBeNull();
    expect(parseNoProvider("")).toBeNull();
    expect(parseNoProvider("not json")).toBeNull();
    expect(parseNoProvider({ head: "  " })).toBeNull();
  });

  test("detects no-provider send guard errors", () => {
    expect(isNoProviderError("No AI provider connected yet. Open Settings → Accounts to connect Claude, Codex, Antigravity, or another provider.")).toBe(true);
    expect(isNoProviderError("Network timeout")).toBe(false);
  });
});
