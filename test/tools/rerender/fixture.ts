// The event log a re-render run's hub starts from (see README.md).
//
// synthetic: projects with a thread each, a long thread (messages and tool
//   activity), a tool-heavy thread, and a background thread. Small, made
//   here, safe to commit results from.
// real: a slice of a real log (--log), kept on this machine only: every
//   project, bot, room and setting the copy needs, every thread's creation,
//   renames, archive/delete and turn state (so the sidebar is the real
//   one), its last message, and *everything* of the threads named. Devices,
//   push, hooks, peers, wakes, routines, locks and non-background work are
//   dropped, as perf_load.ts does; paths under the person's home move into
//   the run's temp folder. The named threads are given unique titles (the
//   palette finds them) and the stand-in Claude as their agent.
import { mkdirSync, openSync, readSync, closeSync, writeFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

export type Target = { role: "long" | "tools" | "bg"; id: string; title: string };
export type Fixture = { lines: number; bytes: number; targets: Target[]; source: string; cut?: number; sha256?: string };

const words = (n: number, seed: number) => Array.from({ length: n }, (_, i) => `word${(i * 7 + seed) % 97}`).join(" ");

export function synthetic(home: string, root: string, o: { projects: number; long: number }): Fixture {
  const at = Math.floor(Date.now() / 1000) - 7200;
  const lines: any[] = [
    { $: "SettingSet", key: "restart.continue", value: "off", at },
    { $: "SettingSet", key: "onboard.done", value: "1", at },
    { $: "SettingSet", key: "onboard.uber", value: "1", at },
  ];
  const thread = (id: string, project: string, title: string, t: number) =>
    lines.push({ $: "ThreadCreated", id, project, title, env: "local", branch: "", worktree: "", provider: "claude", model: "", at: t });
  for (let p = 0; p < o.projects; p++) {
    const dir = join(root, "proj", `p${p}`); mkdirSync(dir, { recursive: true });
    lines.push({ $: "ProjectCreated", id: `p${p}`, title: `Project ${p} ${words(3, p)}`, root: dir, at: at + p });
    thread(`t${p}`, `p${p}`, `Thread ${p}: ${words(6, p)}`, at + p);
    lines.push({ $: "MessagePosted", thread: `t${p}`, msg: `m${p}`, role: "user", text: words(20, p), at: at + p });
  }
  // the long thread: messages with tool activity between, some code and lists
  thread("zqlong", "p0", "zqlong thread", at + 1000);
  for (let i = 0; i < o.long; i++) {
    const text = i % 5 === 3 ? `${i}: here is the change\n\n\`\`\`ts\nconst a${i} = ${i};\nexport function f${i}() { return a${i} * 2; }\n\`\`\`\n\n- ${words(8, i)}\n- ${words(9, i + 1)}` : `${i}: ${words(60 + (i % 7) * 20, i)}`;
    lines.push({ $: "MessagePosted", thread: "zqlong", msg: `l${i}`, role: i % 2 ? "assistant" : "user", text, at: at + 1000 + i });
    if (i % 2 === 1) for (let k = 0; k < 1 + (i % 4); k++)
      lines.push({ $: "ActivityLogged", thread: "zqlong", act: `a${i}-${k}`, tone: "tool", kind: ["Bash", "Read", "Edit", "Grep"][k % 4], summary: words(12, i + k), at: at + 1000 + i });
  }
  // tool-heavy: long runs of calls between short messages
  thread("zqtools", "p1", "zqtools thread", at + 3000);
  for (let i = 0; i < 40; i++) {
    lines.push({ $: "MessagePosted", thread: "zqtools", msg: `x${i}`, role: i % 2 ? "assistant" : "user", text: `${i}: ${words(25, i)}`, at: at + 3000 + i });
    if (i % 2 === 1) for (let k = 0; k < 14; k++)
      lines.push({ $: "ActivityLogged", thread: "zqtools", act: `xa${i}-${k}`, tone: "tool", kind: ["Bash", "Read", "Edit", "Write"][k % 4], summary: `${["grep", "sed", "cat", "ls"][k % 4]} ${words(6, i * k)}`, at: at + 3000 + i });
  }
  thread("zqbg", "p2", "zqbg thread", at + 4000);
  lines.push({ $: "MessagePosted", thread: "zqbg", msg: "bg0", role: "user", text: "the background thread", at: at + 4000 });
  const text = lines.map((l) => JSON.stringify(l)).join("\n") + "\n";
  writeFileSync(join(home, "events.jsonl"), text);
  // (its times are now-relative: hashed as offsets from its base, so the
  // same fixture hashes the same on any day)
  const sha256 = new Bun.CryptoHasher("sha256").update(text.split(root).join("<root>").replace(/"at":(\d+)/g, (_, n) => `"at":+${Number(n) - at}`)).digest("hex");
  return { lines: lines.length, bytes: text.length, source: "synthetic", sha256, targets: [
    { role: "long", id: "zqlong", title: "zqlong thread" }, { role: "tools", id: "zqtools", title: "zqtools thread" }, { role: "bg", id: "zqbg", title: "zqbg thread" }] };
}

const DROP_TYPES = new Set(["DeviceRegistered", "DeviceDropped", "HookSet", "PeerSet", "WakeSet", "RoutineSet", "RoutinePlan", "LockChanged", "LaunchSent", "UberOp"]);
const DROP_SETTING = /^(push|voice|deploy|google|hook|apns|token|secret|sync\.|uber\.|launch\.far)/i;
// what of another thread the sidebar needs
const SIDEBAR_TYPES = new Set(["ThreadCreated", "ThreadRenamed", "ThreadArchived", "ThreadUnarchived", "ThreadDeleted", "ThreadPinned", "ThreadUnpinned", "ThreadSettled", "ThreadSnoozed",
  "TurnChanged", "ProviderSet", "ThreadParentSet", "ThreadLaunched", "ThreadWoke"]);

// lines of a big file, without holding it as one string
function* fileLines(path: string) {
  const fd = openSync(path, "r"), buf = Buffer.alloc(1 << 22);
  let rest = "";
  try {
    for (;;) {
      const n = readSync(fd, buf, 0, buf.length, null);
      if (n <= 0) break;
      const s = rest + buf.toString("utf8", 0, n);
      const parts = s.split("\n");
      rest = parts.pop() ?? "";
      yield* parts;
    }
    if (rest) yield rest;
  } finally { closeSync(fd); }
}

// The same input on every run: the slice ends at `until` (unix seconds;
// default the log's newest event), every turn still running there is
// closed as interrupted and every delegated task still open is cancelled
// and delivered (a copied log otherwise resumes them: the stand-in agent
// then streams during open and idle), and all the slice adds is stamped at
// the cut. report.json carries the cut and the slice's sha256.
export function real(home: string, root: string, o: { log: string; long: string; tools: string; bg: string; until?: number }): Fixture {
  const liveHome = join(homedir(), ".backplane-bend");
  const elsewhere = join(root, "elsewhere");
  const keep = new Set([o.long, o.tools, o.bg].filter(Boolean));
  const out: string[] = [];
  // last-wins facts keep only their last line: a setting by its key; of
  // another thread, its last message, turn state, settle and wake
  const last = new Map<string, number>();
  const keepLast = (key: string, line: string) => { const i = last.get(key); if (i !== undefined) out[i] = ""; last.set(key, out.length); out.push(line); };
  const LAST_OF_OTHERS = new Set(["MessagePosted", "TurnChanged", "ThreadSettled", "ThreadUnsettled", "ThreadWoke", "ProviderSet", "SessionSet", "ModesSet", "WorktreeSet"]);
  const titles = new Map<string, string>();
  const rx = /^\{"\$":"([A-Za-z]+)"/;
  const threadOf = (line: string) => /"thread":"([^"]+)"/.exec(line)?.[1] ?? "";
  const turns = new Map<string, string>(), tasks = new Set<string>();
  const keptTasks = new Set<string>();
  let cut = 0;
  for (const raw of fileLines(o.log)) {
    if (!raw) continue;
    const at = Number(/"at":(\d+)/.exec(raw)?.[1] ?? 0);
    if (o.until && at > o.until) continue;
    if (at > cut) cut = at;
    const ty = rx.exec(raw)?.[1] ?? "";
    if (ty === "TurnChanged") turns.set(/"thread":"([^"]+)"/.exec(raw)?.[1] ?? "", /"state":"([a-z]+)"/.exec(raw)?.[1] ?? "");
    if (ty === "TaskDelegated") tasks.add(/"task":"([^"]+)"/.exec(raw)?.[1] ?? "");
    if (ty === "TaskDelivered") tasks.delete(/"task":"([^"]+)"/.exec(raw)?.[1] ?? "");
    if (DROP_TYPES.has(ty) || /devicetoken|webhook|apns/i.test(raw)) continue;
    if (ty === "WorkSet" && !raw.includes('"kind":"bg"')) continue;
    const setting = ty === "SettingSet" ? /"key":"((?:[^"\\]|\\.)*)"/.exec(raw)?.[1] ?? "" : "";
    if (ty === "SettingSet" && DROP_SETTING.test(setting)) continue;
    // per-thread bookkeeping of other threads (launch.<by>, fallback.<thread>...: 300 MB live)
    // another thread's own state kept as a setting (fallback.*, launch.*,
    // ride.*: its messages waiting to ride a new turn, which the hub then
    // delivers at start and so wakes the thread, ...): dropped whole
    if (ty === "SettingSet" && /t1\d{9}-\d+/.test(setting) && ![...keep].some((id) => setting.includes(id))) continue;
    if (ty === "SettingSet" && /^(fallback|launch|triage|run|wake|tok|turn|restart|alert|told|ride)[.:]/.test(setting) && ![...keep].some((id) => setting.includes(id))) continue;
    const line = raw.split(liveHome).join(home).split(homedir() + "/").join(elsewhere + "/");
    if (ty === "SettingSet") { keepLast("s\u0000" + setting, line); continue; }
    if (ty.startsWith("Thread") || ty === "ProviderSet") {
      const id = /"id":"([^"]+)"/.exec(raw)?.[1] ?? threadOf(raw);
      if (ty === "ThreadCreated") titles.set(id, /"title":"((?:[^"\\]|\\.)*)"/.exec(raw)?.[1] ?? "");
      if (keep.has(id)) out.push(line);
      else if (LAST_OF_OTHERS.has(ty)) keepLast(ty + "\u0000" + id, line);
      else if (SIDEBAR_TYPES.has(ty)) out.push(line);
      continue;
    }
    const t = threadOf(raw);
    // a delegated task is kept only with its parent or its child: the
    // slice keeps another thread's last message only, so the hub, finding a
    // finished task's delivery missing from its parent, delivered it again
    // at start and woke the parent (and through it its launcher)
    if (ty.startsWith("Task")) {
      const id = /"task":"([^"]+)"/.exec(raw)?.[1] ?? "";
      if (ty === "TaskDelegated") {
        const parent = /"parent":"([^"]+)"/.exec(raw)?.[1] ?? "", child = /"child":"([^"]+)"/.exec(raw)?.[1] ?? "";
        if (keep.has(parent) || keep.has(child)) keptTasks.add(id);
      }
      if (keptTasks.has(id)) out.push(line);
      continue;
    }
    if (!t || keep.has(t)) { out.push(line); continue; }
    if (LAST_OF_OTHERS.has(ty)) { keepLast(ty + "\u0000" + t, line); continue; }
    if (SIDEBAR_TYPES.has(ty)) out.push(line);
  }
  for (const id of keep) if (!titles.has(id)) throw new Error(`no thread ${id} in ${o.log}`);
  const now = o.until ?? cut;
  for (const [t, state] of turns) if (t && state === "running") out.push(JSON.stringify({ $: "TurnChanged", thread: t, state: "interrupted", at: now }));
  // a delegated task still open at the cut is taken out whole (its lines
  // dropped): closing it delivered "Delegated task … finished" to its
  // parent, which started a turn, whose end then reported to its launcher
  const taskOf = (l: string) => /^\{"\$":"Task[A-Za-z]+"/.test(l) ? /"task":"([^"]+)"/.exec(l)?.[1] ?? "" : "";
  if (tasks.size) for (let i = 0; i < out.length; i++) if (tasks.has(taskOf(out[i]))) out[i] = "";
  const targets: Target[] = [];
  for (const [role, id] of [["long", o.long], ["tools", o.tools], ["bg", o.bg]] as const) {
    if (!id) continue;
    const title = `zq${role} thread`;
    targets.push({ role, id, title });
    out.push(JSON.stringify({ $: "ThreadRenamed", id, title, at: now }));
    out.push(JSON.stringify({ $: "ProviderSet", id, provider: "claude", model: "", at: now }));
    // nothing left waiting to ride its next turn (it would start one at once)
    out.push(JSON.stringify({ $: "SettingSet", key: `ride.${id}`, value: "", at: now }));
  }
  out.push(JSON.stringify({ $: "SettingSet", key: "restart.continue", value: "off", at: now }));
  out.push(JSON.stringify({ $: "SettingSet", key: "onboard.done", value: "1", at: now }));
  const kept = out.filter(Boolean);
  // the folders projects and threads name (all inside the temp folder now)
  for (const l of kept) for (const m of l.matchAll(/"(?:root|worktree|folder|cwd)":"([^"]+)"/g)) if (m[1].startsWith(root)) try { mkdirSync(m[1], { recursive: true }); } catch {}
  for (const l of kept) if (l.startsWith('{"$":"BotSet"')) try { mkdirSync(join(home, "bots", JSON.parse(l).id), { recursive: true }); } catch {}
  const text = kept.join("\n") + "\n";
  writeFileSync(join(home, "events.jsonl"), text);
  // (each run's temp folder replaced, so equal input hashes equal)
  const sha256 = new Bun.CryptoHasher("sha256").update(text.split(root).join("<root>")).digest("hex");
  return { lines: kept.length, bytes: text.length, source: `real slice of ${o.log} (${(statSync(o.log).size / 1e6).toFixed(0)} MB) cut at ${now}`, targets, cut: now, sha256 };
}
