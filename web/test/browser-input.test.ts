import { describe, expect, test } from "bun:test";
import { mapClickToPageCoords, normalizeBrowserUrl, SerialQueue } from "../src/viewers/browser-input";

describe("browser-input", () => {
  test("maps click coordinates inside object-fit:contain top-aligned image to 0..1000", () => {
    // Container 800x600, image 1280x720 -> scale = 800/1280 = 0.625 -> rendered 800x450 at (0,0)
    const box = { left: 100, top: 50, width: 800, height: 600, naturalWidth: 1280, naturalHeight: 720 };
    expect(mapClickToPageCoords(100, 50, box)).toEqual({ x: 0, y: 0 });
    expect(mapClickToPageCoords(500, 275, box)).toEqual({ x: 500, y: 500 });
    expect(mapClickToPageCoords(900, 500, box)).toEqual({ x: 1000, y: 1000 });
    // Below rendered height (y=550 > 50+450) -> null (outside image)
    expect(mapClickToPageCoords(500, 550, box)).toBeNull();
  });

  test("maps click coordinates with horizontal letterboxing", () => {
    // Container 1000x400, image 800x800 -> scale = 0.5 -> rendered 400x400 centered at x0 = 300
    const box = { left: 0, top: 0, width: 1000, height: 400, naturalWidth: 800, naturalHeight: 800 };
    expect(mapClickToPageCoords(250, 200, box)).toBeNull();
    expect(mapClickToPageCoords(300, 0, box)).toEqual({ x: 0, y: 0 });
    expect(mapClickToPageCoords(500, 200, box)).toEqual({ x: 500, y: 500 });
    expect(mapClickToPageCoords(700, 400, box)).toEqual({ x: 1000, y: 1000 });
    expect(mapClickToPageCoords(750, 200, box)).toBeNull();
  });

  test("normalizes URLs", () => {
    expect(normalizeBrowserUrl("")).toBe("");
    expect(normalizeBrowserUrl("  example.com/docs ")).toBe("https://example.com/docs");
    expect(normalizeBrowserUrl("http://127.0.0.1:8080/a")).toBe("http://127.0.0.1:8080/a");
    expect(normalizeBrowserUrl("https://kicad.org")).toBe("https://kicad.org");
  });

  test("SerialQueue executes operations strictly in FIFO order", async () => {
    const q = new SerialQueue();
    const order: number[] = [];
    const p1 = q.enqueue(async () => {
      await new Promise((r) => setTimeout(r, 25));
      order.push(1);
    });
    const p2 = q.enqueue(async () => {
      order.push(2);
    });
    const p3 = q.enqueue(async () => {
      order.push(3);
    });
    await Promise.all([p1, p2, p3]);
    expect(order).toEqual([1, 2, 3]);
  });
});
