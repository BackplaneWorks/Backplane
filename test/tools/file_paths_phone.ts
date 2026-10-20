import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

// Actual shared bridge and CBOR codec, using public phone APIs only.
const [bridge = "mobile/build/assets/bridge.js", wire = "build/wire"] = process.argv.slice(2);
new Function(readdirSync(wire).filter(f => f.endsWith(".js")).sort().map(file => readFileSync(join(wire, file), "utf8")).join("\n"))();
new Function(readFileSync(resolve(bridge), "utf8"))();
const W = (globalThis as any).Wire;
const B = (globalThis as any).Backplane;
if (!W || !B) throw new Error("build test/wire/index.html and the phone bridge first");
let checks = 0;
function check(ok: boolean, name: string) {
  if (!ok) throw new Error(name);
  checks++;
  console.log(`ok ${name}`);
}
const key = "127.0.0.1:3799";
const thread = "paths-thread";
const frame = (data: object) => JSON.parse(B.recv(key, Buffer.from(W.encode(JSON.stringify(data))).toString("base64"), false, false));
const screen = () => JSON.parse(B.screen()).screen;
const find = () => screen().find;
const rows = () => find()?.rows ?? [];
const names = ["café.step", "雪.kicad_pcb", 'quote".step', "back\\slash.step", "with spaces.step", "tab\there.step", "line\nbreak.step"];
const log = { t: "log", since: 0, origin: "paths-origin", info: {}, items: [
  { $: "ProjectCreated", id: "paths-project", title: "Example", root: "/tmp/file-path-example", at: 1 },
  { $: "ThreadCreated", id: thread, project: "paths-project", title: "Files", env: "local", provider: "claude", at: 2 },
] };
function receiveFiles(paths: string[]) {
  frame({ t: "reply", id: 999, ok: true, result: { thread, files: paths } });
}
B.start("file-path-phone-test", "{}");
B.hubs([key]);
frame(log);
B.act("select", `${key}|${thread}`);
B.act("find-open", "files");
receiveFiles(names);
check(find()?.mode === "files", "phone opens the file palette");
check(rows().length === names.length, "phone receives one row per complete path");
for (let i = 0; i < names.length; i++) {
  check(rows().some((r: any) => r.label === names[i] && r.action === "mention" && r.value === names[i]), `phone preserves filename ${i + 1} in its label and action payload`);
}
B.act("find-q", "break");
check(rows().length === 1 && rows()[0].value === "line\nbreak.step", "phone query matches a complete newline filename");
B.act(rows()[0].action, rows()[0].value);
check(screen().thread?.draft === "@line\nbreak.step ", "phone mention inserts the complete path");
B.act("find-open", "files");
receiveFiles(names);
const saved = B.save();
B.start("file-path-phone-test", "{}");
check(!!B.load(saved), "phone saved state loads with the unchanged state key");
B.act("find-open", "files");
B.act("find-q", "break");
check(rows().length === 1 && rows()[0].value === "line\nbreak.step", "phone keeps record framing across save and load");
const many = Array.from({ length: 2999 }, (_, i) => `component-${i}.step`);
many.push("sentinel\nlast.step");
receiveFiles(many);
B.act("find-q", "sentinel");
check(rows().length === 1 && rows()[0].value === "sentinel\nlast.step", "phone parses all 3000 paths at the phone stack limit");
receiveFiles([]);
check(rows().length === 0, "phone empty listing clears previous paths");
console.log(`${checks} assertions, 0 failures`);
