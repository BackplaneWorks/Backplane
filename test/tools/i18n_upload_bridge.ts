// Actual bundled phone bridge, inert synthetic hub data; no network/provider IO.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { createContext, runInContext } from "node:vm";
const [bridge = "mobile/build/assets/bridge.js", wire = "build/fallback-wire"] = process.argv.slice(2);
const wc = createContext({});
for (const f of readdirSync(wire).filter(f => f.endsWith(".js"))) runInContext(readFileSync(join(wire, f), "utf8"), wc);
(0, eval)(readFileSync(bridge, "utf8"));
const B = (globalThis as any).Backplane, key = "fixture:3999";
const recv = (o: object) => B.recv(key, Buffer.from(wc.Wire.encode(JSON.stringify(o))).toString("base64"), false, false);
const events = readFileSync("test/fixtures/fallback-settings/dark.jsonl", "utf8").trim().split("\n").map(l => JSON.parse(l));
B.start("upload-language-fixture", "{}"); B.hubs([key]); B.online(key, true);
recv({ t: "log", version: "test", seq: events.length, items: events });
recv(JSON.parse(readFileSync("test/fixtures/fallback-settings/info.jsonl", "utf8")));
B.act("select", `${key}|worker`); B.local("lang-preview", "1"); B.local("lang", "en");
function check(reply: string, want: string, label: string) {
  const got = JSON.parse(reply).screen?.error;
  if (got !== want) throw new Error(`${label}: ${JSON.stringify({ got, want })}`);
  console.log(`ok phone upload ${label}`);
}
B.act("attach-up", JSON.stringify({ name: "raw {name}.png", size: 3 }));
check(B.act("attach-end", JSON.stringify({ uiError: "upload.network", upload: true, thread: "worker" })), "the upload did not reach the hub", "error en");
check(B.local("lang", "ja"), "アップロードがハブに届きませんでした", "held en to ja");
check(B.local("lang", "en"), "the upload did not reach the hub", "held ja to en");
B.local("lang", "ja");
check(B.act("attach-end", JSON.stringify({ uiError: "unknown", text: "the upload failed", upload: true, thread: "worker" })), "the upload failed", "unknown raw text unchanged");
check(B.act("attach-up", JSON.stringify({ name: "raw {name}.png", size: 99999999 })), "添付ファイルは10 MBまでです", "oversized file ja");

check(B.act("attach-end", "null"), "アップロードに失敗しました", "nonobject reply ja");
