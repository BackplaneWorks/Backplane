import { describe, expect, test } from "bun:test";
import { interpolate } from "../src/i18n-format";

describe("i18n interpolate", () => {
  test("inserts $& and other regex replacement tokens literally", () => {
    expect(interpolate("Answered: {answer}", { answer: "$&" })).toBe("Answered: $&");
    expect(interpolate("Answered: {answer}", { answer: "$$ $` $' $1" })).toBe("Answered: $$ $` $' $1");
  });

  test("does not re-interpolate placeholder tokens inside variable values", () => {
    expect(interpolate("Finished {time} · {dur}", { time: "{dur}", dur: "3s" })).toBe("Finished {dur} · 3s");
  });

  test("leaves unknown placeholders untouched and handles missing vars", () => {
    expect(interpolate("Hello {missing}", { answer: "ok" })).toBe("Hello {missing}");
    expect(interpolate("Plain text")).toBe("Plain text");
  });
});
