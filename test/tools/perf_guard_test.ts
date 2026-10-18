import { strict as assert } from "node:assert";
import { completeOpens, FixtureBots, streamWindow } from "./perf_guard";

const bots = new FixtureBots();
bots.apply({ $: "BotSet", id: "bot", thread: "thread" });
bots.apply({ $: "BotSet", id: "bot", thread: "" });
assert.equal(bots.threads.get("bot"), "thread");
bots.apply({ $: "UberSet", bot: "bot" });
assert.equal(bots.uber, "bot");
bots.apply({ $: "SettingSet", key: "uber.bot", value: "legacy" });
assert.equal(bots.uber, "legacy");
bots.apply({ $: "UberSet", bot: "" });
assert.equal(bots.uber, "");
bots.apply({ $: "BotDeleted", id: "bot" });
assert.equal(bots.threads.has("bot"), false);

// A burst followed by a dead stream must not satisfy a whole phase.
assert.equal(streamWindow(Array.from({ length: 100 }, (_, i) => i * 20), 0, 26000).sustained, false);
assert.equal(streamWindow(Array.from({ length: 260 }, (_, i) => i * 100), 0, 26000).sustained, true);
assert.equal(streamWindow([], 0, 100).sustained, false);
assert.equal(streamWindow([0, 100, 5000], 0, 5100).sustained, false);
assert.equal(streamWindow([0], 0, 26000).longest_gap_ms, 26000);
// Events just outside a phase cannot fill its gaps.
assert.equal(streamWindow([-1, 26001], 0, 26000).sustained, false);
assert.equal(completeOpens([1], 10), false);
assert.equal(completeOpens([1, NaN], 2), false);
assert.equal(completeOpens([], 0), false);
assert.equal(completeOpens([1, 2], 2), true);
console.log("PASS: fixture bot updates/promotions, dead streams, boundary gaps and partial/missing opens");
