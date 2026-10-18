#!/usr/bin/env bun
// Test-only host wrapper: a gate file ends our own host's stdin, without signals.
import { existsSync } from "node:fs";
const actual = process.env.BACKPLANE_CEF;
const gate = process.env.BACKPLANE_CEF_STOP_FILE;
if (!actual || !gate) throw new Error("relay needs BACKPLANE_CEF and BACKPLANE_CEF_STOP_FILE");
const host = Bun.spawn([actual, ...process.argv.slice(2)], { stdin: "pipe", stdout: "inherit", stderr: "inherit" });
let ended = false;
function end() { if (!ended) { ended = true; host.stdin.end(); } }
process.stdin.on("data", (data) => { if (!ended) { host.stdin.write(data); host.stdin.flush(); } });
process.stdin.on("end", end);
const timer = setInterval(() => { if (existsSync(gate)) end(); }, 25);
const code = await host.exited;
clearInterval(timer);
process.exit(code);
