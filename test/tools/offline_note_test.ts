// A Send pressed while the link is down goes nowhere ("Not sent"); once the
// link is back the bar must still say so (it used to clear, leaving the text
// in the composer looking sent) until the next Send or a while passes.
// Lifts host.js's own upStatus/holdViewNote/dropUnsentBack; no DOM or hub.
import { readFileSync } from "node:fs";
import { strict as assert } from "node:assert";
const source = readFileSync("src/web/host.js", "utf8");
const from = source.indexOf("let unsentBack = \"\";");
const to = source.indexOf("function thinReconnect()");
assert.ok(from > 0 && to > from, "host.js keeps the reconnect note functions together");
const h = new Function(`
  let viewNote = "", viewNoteTimer = 0, remoteReady = true, ui = {};
  let downSince = 5, downTimer = 0, offlineSend = false, offlineAttach = false;
  const bar = [];
  const timers = [];
  const setTimeout = (f) => { timers.push(f); return timers.length; };
  const clearTimeout = () => {};
  const showViewStatus = (t) => bar.push(t);
  const reconnectNote = () => {}, connDot = () => {};
  const thinBackoff = { reset() {} }, thinRetry = { cancel() {} };
  const App = { host_message: (u, k) => k === "offline-send-back" ? "BACK" : "?" };
  ${source.slice(from, to)}
  return { upStatus, dropUnsentBack, bar, timers,
    down(sent) { offlineSend = sent; }, note: () => viewNote };
`)();
h.down(true);
h.upStatus();
assert.equal(h.note(), "BACK", "a Send made while down is still told once connected");
assert.equal(h.bar.at(-1), "BACK");
h.dropUnsentBack();
assert.equal(h.note(), "", "the next Send clears it");
assert.equal(h.bar.at(-1), "");
h.down(true); h.upStatus(); h.timers.at(-1)();
assert.equal(h.note(), "", "and it goes by itself after a while");
h.down(false); h.upStatus();
assert.equal(h.note(), "", "a reconnect with nothing unsent says nothing");
console.log("PASS: an unsent message is still said once the link is back");
