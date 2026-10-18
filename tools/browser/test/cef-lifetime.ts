// Unexpected host exit and helper EOF, without signalling any browser process.
import { existsSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { check, until, jsonProcess, privateDisplay } from "../../cef/test/harness.ts";
if (!process.env.BACKPLANE_CEF || !existsSync(process.env.BACKPLANE_CEF)) {
  console.log("SKIP CEF lifetime: set BACKPLANE_CEF"); process.exit(0);
}
const d = await privateDisplay();
const gate = join(d.root, "end-host-stdin");
const flags = ["--profile", join(d.root, "profile"), "--engine", "cef", "--parent", String(d.parent), "--display", d.env.DISPLAY, "--cef", new URL("../../cef/test/relay.ts", import.meta.url).pathname];
const h = jsonProcess(process.env.BACKPLANE_BROWSER ? [process.env.BACKPLANE_BROWSER, ...flags] : ["bun", new URL("../src/main.ts", import.meta.url).pathname, ...flags], { ...d.env, BACKPLANE_CEF_STOP_FILE: gate });
try {
  const e = await h.call("open", { page: "life", url: "data:text/html,<h1>Before exit</h1>" });
  check("--engine cef and --cef launch directly", e.engine === "cef" && e.hostPid > 0);
  await h.call("record", { page: "life", on: true });
  writeFileSync(gate, "EOF");
  await until(() => h.events.find((v) => v.event === "engine" && v.ok === false), "unexpected host exit");
  check("host exit reports engine failure and closed", h.events.some((v) => v.event === "closed"));
  check("host exit clears owner tabs", (await h.call("pages")).pages.length === 0);
  check("old owned host exited", !existsSync(`/proc/${e.hostPid}`));
  rmSync(gate);
  await h.call("open", { page: "life", url: "data:text/html,<h1>After exit</h1>" });
  const next = await h.call("status", { page: "life" });
  check("next page request relaunches host", next.hostPid !== e.hostPid && next.open);
  check("reopened owner has new document", (await h.call("snapshot", { page: "life" })).visibleText.includes("After exit"));
  h.proc.stdin.end();
  check("helper EOF exits zero", await h.proc.exited === 0);
  check("helper EOF waits for host exit", !existsSync(`/proc/${next.hostPid}`));
  console.log("all CEF lifetime checks passed");
} finally {
  if (h.proc.exitCode === null) { h.proc.stdin.end(); await h.proc.exited; }
  await d.cleanup();
}
