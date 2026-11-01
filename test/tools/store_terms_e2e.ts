// A store whose search terms are behind (meta "terms" another version) gets
// its term table made again from the entry table, without a rebuild, equal
// to a fresh build's (server/storeterms.bend).
//
//   bun test/tools/store_terms_e2e.ts
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Database } from "bun:sqlite";

const root = mkdtempSync(join(process.env.STORE_UP_DIR ?? tmpdir(), "bp-terms-"));
let fail = 0;
const check = (ok: unknown, what: string, got?: unknown) => { console.log(`${ok ? "ok" : "FAIL"} ${what}${ok || got === undefined ? "" : ": " + JSON.stringify(got).slice(0, 300)}`); if (!ok) fail++; };
const run = async (file: string, home: string) => {
  const p = Bun.spawn(["sh", "-c", 'ulimit -s 1048576 2>/dev/null; exec bend "$@"', "sh", resolve(file), home], { env: { PATH: `${process.env.HOME}/.bend/bin:/usr/bin:/bin`, HOME: process.env.HOME ?? root, BEND_NO_TELEMETRY: "1", BUN_JSC_maxPerThreadStackUsage: "4000000000" }, stdout: "pipe", stderr: "pipe" });
  const [o, e] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text()]);
  await p.exited;
  return o + e;
};
const ev = ($: string, f: object) => JSON.stringify({ $, ...f, at: 1 });
const words = ["board", "Schematic", "résumé", "BoardView", "ÉCOLE", "naïve", "router", "KiCad"];
const lines = [ev("ProjectCreated", { id: "p", title: "P", root: "/p" }), ev("ThreadCreated", { id: "t1", project: "p", title: "a", env: "local", branch: "", worktree: "", provider: "claude", model: "" })];
for (let i = 0; i < 300; i++) lines.push(ev(i % 2 ? "MessagePosted" : "ActivityLogged", i % 2 ? { thread: "t1", id: `m${i}`, role: "user", text: `${words[i % 8]} ${words[(i * 3) % 8]} number ${i}` } : { thread: "t1", id: `a${i}`, tone: "tool", kind: "Bash", text: `${words[(i * 5) % 8]} tool ${i}` }));
const terms = (file: string) => { const d = new Database(file, { readonly: true }); try { return JSON.stringify(d.query("SELECT word, ord FROM term ORDER BY word, ord").values()); } finally { d.close(); } };
const meta = (file: string, k: string) => { const d = new Database(file, { readonly: true }); try { return (d.query("SELECT v FROM meta WHERE k = ?").values(k)[0] ?? [""])[0]; } finally { d.close(); } };
const tables = (file: string) => { const d = new Database(file, { readonly: true }); try { return d.query("SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'term%'").values().map((r) => String(r[0])).sort(); } finally { d.close(); } };
try {
  const fresh = join(root, "fresh"), old = join(root, "old");
  for (const d of [fresh, old]) { mkdirSync(d); writeFileSync(join(d, "events.jsonl"), lines.join("\n") + "\n"); }
  await run("test/native/store_up_probe.bend", fresh);
  cpSync(join(fresh, "store.db"), join(old, "store.db"));
  check(meta(join(fresh, "store.db"), "terms") === "1", "a fresh build records its terms version");
  // an older version's terms: other words, and the version behind
  const d = new Database(join(old, "store.db"));
  d.exec("DELETE FROM term; INSERT INTO term(word, ord) VALUES ('stale', 1), ('words', 2); UPDATE meta SET v = '0' WHERE k = 'terms'");
  d.close();
  check(terms(join(old, "store.db")) !== terms(join(fresh, "store.db")), "the old store's terms differ");
  const out = await run("test/native/store_terms_probe.bend", old);
  console.log(`  ${out.trim().split("\n").pop()}`);
  check(terms(join(old, "store.db")) === terms(join(fresh, "store.db")), "made again, the terms equal a fresh build's, row for row");
  check(meta(join(old, "store.db"), "terms") === "1" && meta(join(old, "store.db"), "terms.next") === "", "meta says the current version, and no build is pending");
  check(JSON.stringify(tables(join(old, "store.db"))) === JSON.stringify(["term"]), "term_next and term_old are gone", tables(join(old, "store.db")));
  const again = await run("test/native/store_terms_probe.bend", old);
  check(again.includes("current"), "a current store is left alone", again.trim().split("\n").pop());
} finally {
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
