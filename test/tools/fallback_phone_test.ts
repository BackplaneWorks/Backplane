// Exercise the actual bundled iOS/Android bridge, without a hub or providers.
import { readFileSync, readdirSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createContext, runInContext } from "node:vm";

const [bridge = "mobile/build/assets/bridge.js", wire = "build/fallback-wire", out = "build/fallback-phone"] = process.argv.slice(2);
// Each bundle owns its generated short names, as in separate native engines.
const wireContext = createContext({});
for (const f of readdirSync(wire).filter(f => f.endsWith(".js"))) runInContext(readFileSync(join(wire, f), "utf8"), wireContext);
const W = wireContext.Wire;
(0, eval)(readFileSync(bridge, "utf8"));
const B = (globalThis as any).Backplane;
const key = "fixture:3999";
const check = (yes: unknown, name: string) => { if (!yes) throw new Error(name); console.log(`ok ${name}`); };
const recv = (o: object) => JSON.parse(B.recv(key, Buffer.from(W.encode(JSON.stringify(o))).toString("base64"), false, false));
const events = readFileSync("test/fixtures/fallback-settings/dark.jsonl", "utf8").trim().split("\n").map(l => JSON.parse(l));
B.start("fallback-phone", "{}"); B.hubs([key]); B.online(key, true);
recv({ t: "log", version: "test", seq: events.length, items: events });
recv(JSON.parse(readFileSync("test/fixtures/fallback-settings/info.jsonl", "utf8")));
B.act("select", `${key}|worker`);
const settings = JSON.parse(B.act("flag", "settings"));
const rows = JSON.stringify(settings.screen?.settings);
check(rows?.includes("Fallback hierarchy"), "bundled phone bridge renders fallback Settings");
check(rows.includes("fallback.on=off") && rows.includes("Move up") && rows.includes("Remove"), "phone Settings carries toggle reorder and remove controls");
check(rows.includes("codex/gpt-6.1-sol"), "phone Settings keeps exact configured Codex model");

const body = "Example thread: claude/sonnet → codex/gpt-6.1-sol. Weekly usage limit. 3 active threads affected. Reset supplied.";
const alert = { $: "SettingSet", key: "alert.worker", value: `fallback\toutage-test\t\t20.1\t${body}`, at: 20 };
const notified = recv({ t: "changes", items: [alert] });
const notifications = notified.cmds.filter((c: any) => c.type === "notify");
check(notifications.length === 1, "phone receives one persistent fallback notification");
check(notifications[0].body === body && notifications[0].kind === "fallback", "phone notification preserves full fallback reason and affected count");
check(notifications[0].thread === `${key}|worker`, "phone notification has qualified thread deeplink");
const replayed = recv({ t: "changes", items: [alert] });
check(!replayed.cmds.some((c: any) => c.type === "notify"), "phone replay deduplicates the same outage notification");

mkdirSync(out, { recursive: true });
writeFileSync(join(out, "settings.json"), JSON.stringify(settings.screen.settings, null, 2));
writeFileSync(join(out, "notification.json"), JSON.stringify(notifications[0], null, 2));
