import { describe, expect, test } from "bun:test";
import { parseRingList, ringAudioPath, ringCountText, ringHeardText, ringStateText } from "../src/ring-model";
import { interpolate } from "../src/i18n-format";

describe("Ring notes model", () => {
  test("parseRingList keeps notes that have an id and reads the rest defensively", () => {
    const l = parseRingList({
      total: 3,
      notes: [
        { id: "n0123456789abcdef", said: 1_790_000_000, gesture: "Hold & talk", words: "Call Sam", model: "parakeet-v2", conf: 91, unsure: "", state: "sent", error: "", thread: "t9", play: "https://hub.example/ring/audio/n0123456789abcdef?k=ab12", heard: 1 },
        { words: "no id" },
        null,
        { id: "n1111111111111111" },
      ],
    });
    expect(l.total).toBe(3);
    expect(l.notes.map((n) => n.id)).toEqual(["n0123456789abcdef", "n1111111111111111"]);
    expect(l.notes[0].conf).toBe(91);
    expect(l.notes[1]).toEqual({
      id: "n1111111111111111", said: 0, gesture: "", words: "", model: "", conf: null, unsure: "", state: "", error: "", thread: "", play: "", heard: 0,
    });
    expect(parseRingList(null)).toEqual({ total: 0, notes: [], busy: "" });
    expect(parseRingList({ busy: "n2222222222222222", notes: [] }).busy).toBe("n2222222222222222");
    expect(l.busy).toBe("");
    expect(parseRingList({ notes: [{ id: "n2222222222222222" }] }).total).toBe(1);
  });

  test("a recording plays from this page's own origin, by its id and key only", () => {
    expect(ringAudioPath("https://hub.example:8451/ring/audio/n0123456789abcdef?k=ab12CD")).toBe("/ring/audio/n0123456789abcdef?k=ab12CD");
    expect(ringAudioPath("http://127.0.0.1:3797/ring/audio/n0123456789abcdef.m4a?k=00")).toBe("/ring/audio/n0123456789abcdef?k=00");
    for (const bad of [
      "",
      "https://elsewhere.example/audio/n0123456789abcdef?k=ab",
      "https://hub.example/ring/audio/n0123456789abcdef",
      "https://hub.example/ring/audio/../../file?k=ab",
      "https://hub.example/ring/audio/n0123456789abcdef?k=ab&next=//evil.example",
      "javascript:alert(1)//ring/audio/n0123456789abcdef?k=zz",
    ]) {
      expect(ringAudioPath(bad)).toBe("");
    }
  });

  test("who heard a note, and where it is", () => {
    expect(ringHeardText({ model: "", conf: null }).fallback).toBe("Not heard");
    expect(ringHeardText({ model: "ring", conf: 80 }).fallback).toBe("The Pebble app's words");
    const sure = ringHeardText({ model: "parakeet-v2", conf: 90.6 });
    expect(interpolate(sure.fallback, sure.vars)).toBe("Heard by parakeet-v2, 91% sure");
    const plain = ringHeardText({ model: "gpt-4o-transcribe", conf: null });
    expect(interpolate(plain.fallback, plain.vars)).toBe("Heard by gpt-4o-transcribe");

    expect(ringStateText("sent", false).fallback).toBe("given to Pebble");
    expect(ringStateText("new", false).fallback).toBe("waiting to be heard");
    expect(ringStateText("heard", false).fallback).toBe("waiting to go to Pebble");
    expect(ringStateText("failed", false).fallback).toBe("not given to Pebble");
    expect(ringStateText("sent", true).fallback).toBe("being heard now");
    expect(ringStateText("odd", false)).toEqual({ key: "", fallback: "odd" });
  });

  test("the count line", () => {
    expect(ringCountText(0, 0, "").key).toBe("ring.no.recordings.yet");
    expect(ringCountText(1, 1, "").fallback).toBe("1 recording");
    const many = ringCountText(12, 12, "");
    expect(interpolate(many.fallback, many.vars)).toBe("12 recordings");
    const found = ringCountText(12, 2, " sam ");
    expect(interpolate(found.fallback, found.vars)).toBe("2 of 12 have “sam”.");
  });
});
