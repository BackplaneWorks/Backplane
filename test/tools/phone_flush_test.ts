// Test the bridge's actual flush/out helpers without a hub, state or Bend build.
import { readFileSync } from "node:fs";
import { strict as assert } from "node:assert";
const source = readFileSync("src/mobile/bridge.js", "utf8");
const out = source.slice(source.indexOf("function out("), source.indexOf("// a step's answer:"));
// direct() builds every answer now (thin drain, kept drafts, deadline)
const direct = source.slice(source.indexOf("function direct("), source.indexOf("function expired()"));
const start = source.indexOf("  flush(owe) {");
assert(start > 0, "bridge exposes an owed-screen flush");
const flush = source.slice(start, source.indexOf("\n  },", start) + 5);
const harness = new Function(`
  let owed = false, renders = 0, hubs = {};
  const each = function* () {};
  const is = () => false, plain = x => x;
  const App = {screenj: () => ({version: ++renders})};
  const screenText = () => JSON.stringify(plain(App.screenj(hubs)));
  // what flush/out/direct read from the rest of bridge.js: no thin hub, no
  // expired calls or sync updates, nothing waiting for the foreground
  const thin = { drain: () => ({ cmds: [], sync: [] }), thin: () => false, views: () => null };
  const kept = {}, syncUpdates = [], recent = { links: new Map() };
  const expired = () => [], currentDeadline = () => 0, commandList = cmds => cmds ?? [];
  const foregroundHub = () => "", waitingForForeground = () => false, foregroundPresentation = () => false;
  ${direct}
  ${out}
  return {${flush}
    withhold() { owed = true; },
    renders() { return renders; }
  };
`)();
assert.equal(JSON.parse(harness.flush(false)).screen, null);
assert.equal(harness.renders(), 0);
harness.withhold();
assert.deepEqual(JSON.parse(harness.flush(false)).screen, {version: 1});
assert.equal(JSON.parse(harness.flush(false)).screen, null);
assert.equal(harness.renders(), 1);
assert.deepEqual(JSON.parse(harness.flush(true)).screen, {version: 2});
assert.equal(JSON.parse(harness.flush(false)).screen, null);
assert.equal(harness.renders(), 2);
console.log("PASS: actual bridge flush draws an owed screen once, forces a dropped screen, and otherwise builds nothing");
