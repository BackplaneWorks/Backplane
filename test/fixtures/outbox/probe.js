// Test exports from the real shared client; no replacement sorting here.
import P from "./probe.bend";
import "../../wire/wire.js";

globalThis.OutboxFixture = {
  scenario: (start, thread) => JSON.parse(P.scenario(BigInt(start), thread)),
};
