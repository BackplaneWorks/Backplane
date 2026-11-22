import { describe, expect, test } from "bun:test";
import { isLocalNoProviderError, isNoProviderError, parseNoProvider } from "../src/provider-notice";

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

  test("detects local vs remote no-provider send guard errors", () => {
    const localMsg = "No AI provider connected yet. Open Settings → Accounts to connect Claude, Codex, Antigravity, or another provider.";
    const remoteMsg = "No AI provider connected on boat-hub. Open Settings → Accounts on boat-hub to connect Claude, Codex, Antigravity, or another provider.";
    expect(isNoProviderError(localMsg)).toBe(true);
    expect(isNoProviderError(remoteMsg)).toBe(false);
    expect(isNoProviderError("Network timeout")).toBe(false);

    expect(isLocalNoProviderError(localMsg, "no_provider")).toBe(true);
    expect(isLocalNoProviderError(remoteMsg, "no_provider_remote")).toBe(false);
    expect(isLocalNoProviderError(localMsg, "no_provider_remote")).toBe(false);
    expect(isLocalNoProviderError(localMsg)).toBe(true);
    expect(isLocalNoProviderError(remoteMsg)).toBe(false);
  });
});
