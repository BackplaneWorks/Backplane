# Web plot motion and rested-frame validation

This integrates Cameron Basham's [PR #62](https://github.com/i2cjak/Backplane/pull/62)
with the current web viewer. During camera motion, the viewer reuses a layer
bitmap; after 120 ms without movement it redraws vector paths directly to the
visible canvas. Direct rested drawing matters because fractional bitmap offsets,
and even the offscreen margin's clipping, change Canvas2D antialiasing. Cache
warming runs separately and leaves those rested pixels in place.

Geometry identity, masks, focused layer, fade setting, background, size and DPR
invalidate the bitmap. A changed fade attribute also requests a frame when
mounting the canvas. Hidden viewers cancel their own frames, timers and observers.
A completed fade and a resting view stop asking for frames.

From the repository root, with Playwright and Chrome available:

```sh
git show 30884966ef59c383581bcd1b7653f8f739cf009e:src/web/plot2d.js > /tmp/plot2d-main.js
PLAYWRIGHT_MODULE=/absolute/path/to/playwright-core/index.mjs \
  CHROME=/absolute/path/to/chrome \
  node test/tools/plot2d_cache.mjs /tmp/plot2d-main.js
PLAYWRIGHT_MODULE=/absolute/path/to/playwright-core/index.mjs \
  CHROME=/absolute/path/to/chrome \
  node test/tools/plot2d_mount.mjs /tmp/plot2d-main.js
```

The first runner uses the production wire-chunk decoder, Path2D and Canvas2D
with generated asymmetric copper polygons and stroke text. It checks 14 pixel,
coordinate and idle conditions against the unchanged main renderer. It forces
raster completion with a pixel read before recording draw timings. Its dense
fixture contains 30,000 polygons; the cache and vector paths receive the same
camera sequence. The runner accepts a third source path for a negative control
or an identical-source timing control.

The mount runner uses production `mount`/`got`, animation frames, idle warming,
fade completion, version changes, ResizeObserver and disposal. Its ten assertions
check actual visible pixels without forcing the candidate to draw. Chrome's CDP
DPR override changes the resolution without delivering a media event in this
headless environment; the test delivers that change notification on the real
MediaQueryList event target. This checks the listener and redraw path and does
not certify an operating-system monitor transition. Arrival telemetry is omitted
from pixel comparisons because its elapsed timings vary between draws.

On an Intel Core i7-8650U Linux host, headless Chrome 155.0.8059.84,
Node 22.23.3 and Playwright 1.62.1, two
same-fixture runs measured the following milliseconds. These are software Canvas
draw/raster timings, not physical display or pointer-to-pixel latency:

| Measurement | Run 1 baseline / candidate | Run 2 baseline / candidate |
| --- | ---: | ---: |
| Pan p95 | 136.3 / 2.3 | 123.9 / 1.7 |
| Zoom p95 | 163.2 / 2.1 | 135.5 / 1.8 |
| First paint | 132.7 / 132.2 | 125.5 / 92.2 |
| Layer toggle | 4.7 / 4.3 | 1.5 / 3.8 |

All 14 dense checks and ten mount checks pass. Before the rested-drawing fix,
the rebased original cache failed initial, fractional-pan, zoom and fit pixel
comparisons. Before the fade scheduling fix, a warmed canvas did not repaint
when only `data-fade` changed; the other nine mount assertions passed.

Physical browser/device visual acceptance and the full affected bundle builds
remain separate validation lanes. These results do not establish those lanes.
