// The actual phone bridge, with only generated hub frames. Exercises the
// same screen JSON consumed by iOS and Android, including saved-state load.
// No hub, installed binary, network, user data or native device is used.
//
//   bun test/tools/files_phone.ts [BRIDGE] [WIREDIR]
// WIREDIR is emitted by: bend test/wire/index.html -o build/wire
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const [bridge = "mobile/build/assets/bridge.js", wire = "build/wire"] = process.argv.slice(2);
// Independently minified bundles can reuse internal variable names.
// Keep each bundle's internals in its own scope; only exports are global.
new Function(readdirSync(wire).filter(f => f.endsWith(".js")).sort().map(file => readFileSync(join(wire, file), "utf8")).join("\n"))();
new Function(readFileSync(resolve(bridge), "utf8"))();
const W = (globalThis as any).Wire;
const B = (globalThis as any).Backplane;
if (!W || !B) throw new Error("build test/wire/index.html and the phone bridge first");
let failed = 0, checks = 0;
function check(label: string, ok: boolean, detail?: unknown) {
  checks++;
  console.log(`${ok ? "ok" : "FAIL"} ${label}${ok ? "" : `: ${JSON.stringify(detail)?.slice(0, 400)}`}`);
  if (!ok) failed++;
}
const key = "127.0.0.1:3799";
const thread = "files-thread";
const origin = "files-phone-origin";
const log = {
  t: "log", since: 0, origin, info: {}, items: [
    { $: "ProjectCreated", id: "files-project", title: "Example", root: "/tmp/files-phone-example", at: 1 },
    { $: "ThreadCreated", id: thread, project: "files-project", title: "Files", env: "local", provider: "claude", at: 2 },
  ],
};
const received = (frame: object) => JSON.parse(B.recv(key, Buffer.from(W.encode(JSON.stringify(frame))).toString("base64"), false, false));
const screen = () => JSON.parse(B.screen()).screen;
const files = () => screen()?.thread?.viewer?.files;
const values = () => (files()?.rows ?? []).map((row: any) => row.value);
function requests(step: any) {
  return (step.cmds ?? []).filter((c: any) => c.type === "send").map((c: any) => ({ hub: c.hub, request: JSON.parse(W.decode(new Uint8Array(Buffer.from(c.data, "base64")))) })).filter((c: any) => c.request.m === "files.dir");
}
function ask(action: string, path: string) {
  const sent = requests(JSON.parse(B.act(action, path)));
  if (sent.length !== 1 || sent[0].hub !== key) throw new Error(`expected one routed files.dir after ${action}: ${JSON.stringify(sent)}`);
  return sent[0].request;
}
function reply(request: any, entries: string) {
  received({ t: "reply", id: request.id, ok: true, result: { thread: request.p.thread, fmDir: request.p.path, fmRoot: "/tmp/files-phone-example", fmEntries: entries } });
}
function error(request: any, text: string) {
  received({ t: "reply", id: request.id, ok: false, result: { text } });
}

B.start("files-phone-test", "{}");
B.hubs([key]);
received(log);
B.act("select", `${key}|${thread}`);
const root = ask("fm", "");
check("phone: root request names the correct thread and empty path", root.p.thread === thread && root.p.path === "", root);
check("phone: root waits for a reply", files()?.ready === false, files());
reply(root, "");
check("phone: empty project root is a completed listing", files()?.ready === true && files()?.rows?.length === 0, files());

const a = ask("fm-cd", "a"), b = ask("fm-cd", "b");
reply(b, "f\t2\tnew.txt\n");
check("phone: newest folder reply supplies its rows", files()?.ready === true && JSON.stringify(values()) === '["b/new.txt"]', files());
reply(a, "f\t1\told.txt\n");
check("phone: delayed old folder cannot hide the current listing", files()?.ready === true && JSON.stringify(values()) === '["b/new.txt"]', files());

const firstA = ask("fm-cd", "a");
const betweenB = ask("fm-cd", "b");
const latestA = ask("fm-cd", "a");
check("phone: revisiting a folder waits for its new generation", files()?.ready === false, files());
reply(latestA, "f\t2\tfresh.txt\n");
reply(firstA, "f\t1\tstale.txt\n");
reply(betweenB, "f\t1\tother.txt\n");
check("phone: same-path old generation cannot replace current contents", files()?.ready === true && JSON.stringify(values()) === '["a/fresh.txt"]', files());
const refresh = ask("fm", "");
check("phone: refreshing a loaded folder waits for its reply", files()?.ready === false, files());
reply(refresh, "");
check("phone: an empty child folder is a completed listing", files()?.ready === true && files()?.rows?.length === 0, files());

const oldFailure = ask("fm-cd", "a"), current = ask("fm-cd", "b");
reply(current, "f\t2\tcurrent.txt\n");
error(oldFailure, "obsolete failure");
check("phone: stale failure cannot replace success with an error", files()?.ready === true && screen().error === "", screen().error);
const oldSuccess = ask("fm-cd", "a"), missing = ask("fm-cd", "missing");
error(missing, "no such folder");
check("phone: current failure remains visible and is not successful emptiness", screen().error === "no such folder" && files()?.ready === false, screen().error);
reply(oldSuccess, "f\t1\tobsolete.txt\n");
check("phone: late success cannot clear the current error", screen().error === "no such folder" && files()?.ready === false, screen().error);

const repaired = ask("fm-cd", "");
reply(repaired, "");
check("phone: retry recovers to a ready empty root", files()?.ready === true && screen().error === "", files());
const relisted = requests(received(log));
check("phone: reconnect refreshes the retained project root", relisted.length === 1 && relisted[0].hub === key && relisted[0].request.p.thread === thread && relisted[0].request.p.path === "", relisted);
check("phone: reconnect discards the previous root completion", files()?.ready === false, files());
if (relisted[0]) reply(relisted[0].request, "");
check("phone: reconnect accepts a fresh empty listing", files()?.ready === true, files());

const kept = B.save();
B.start("files-phone-test", "{}");
check("phone: saved client state loads", !!B.load(kept));
const afterLoad = requests(received(log));
check("phone: saved root is refreshed after loading and reconnecting", afterLoad.length === 1 && afterLoad[0].request.p.thread === thread && afterLoad[0].request.p.path === "", afterLoad);
check("phone: loaded stale listing waits for its new reply", files()?.ready === false, files());
if (afterLoad[0]) reply(afterLoad[0].request, "");
check("phone: saved root recovers to completed emptiness", files()?.ready === true && files()?.rows?.length === 0, files());

console.log(`${checks} assertions, ${failed} failures`);
process.exit(failed ? 1 : 0);
