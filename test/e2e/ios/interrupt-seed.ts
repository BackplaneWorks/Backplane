// Seeds the interrupt journey's rig over data/1: a project and an empty thread; prints {project, thread}. usage: bun interrupt-seed.ts HOST PORT TOKEN_FILE PROJECT_DIR
import { readFileSync } from "fs";
const [host, port, tokf, dir] = process.argv.slice(2);
const tok = readFileSync(tokf, "utf8").trim();
const ws = new WebSocket(`ws://${host}:${port}/ws?data=1&enc=json&token=${tok}`);
let id = 0; const wait = new Map<number, (r: any) => void>();
ws.onmessage = (e) => { const m = JSON.parse(String(e.data)); if (m.t === "reply") wait.get(m.id)?.(m); };
const call = (m: string, p: any) => new Promise<any>((res) => { const i = ++id; wait.set(i, res); ws.send(JSON.stringify({ id: i, m, p })); });
await new Promise((r) => (ws.onopen = r));
const p = await call("project.add", { path: dir, title: "Rig" });
const project = p.result?.project?.id ?? (await call("projects.list", {})).result.projects[0].id;
const t = await call("thread.create", { project, title: "Interrupt" });
console.log(JSON.stringify({ project, thread: t.result.thread?.id ?? t.result.thread }));
ws.close();
