// Seeds the iOS terminal rig (scripts/mobile/ios-term-rig.sh) over data/1: a project, a thread whose stand-in agent opens a chat terminal (terminal_open
// as that thread), and prints {project, thread}. usage: bun rig-seed.ts HOST PORT TOKEN_FILE PROJECT_DIR
import { readFileSync } from "fs";
const [host, port, tokf, dir] = process.argv.slice(2);
const tok = readFileSync(tokf, "utf8").trim();
const ws = new WebSocket(`ws://${host}:${port}/ws?data=1&enc=json&token=${tok}`);
let id = 0; const wait = new Map<number, (r: any) => void>();
ws.onmessage = (e) => { const m = JSON.parse(String(e.data)); if (m.t === "reply") wait.get(m.id)?.(m); };
const call = (m: string, p: any) => new Promise<any>((res) => { const i = ++id; wait.set(i, res); ws.send(JSON.stringify({ id: i, m, p })); });
await new Promise((r) => (ws.onopen = r));
const open = JSON.stringify({ name: "terminal_open", arguments: { command: "for i in 1 2 3; do echo card line $i; done", title: "Card demo", cols: 60, rows: 12 } });
const p = await call("project.add", { path: dir, title: "Rig" });
const project = p.result?.project?.id ?? (await call("projects.list", {})).result.projects[0].id;
const t = await call("thread.create", { project, title: "Terminal chat", text: `mode:stream BP_STREAM_N=1 mcpcall:${open}` });
console.log(JSON.stringify({ project, thread: t.result.thread?.id ?? t.result.thread }));
ws.close();
