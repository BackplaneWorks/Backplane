# Click-to-render results: the 2026-11-05 snappiness branch

Before = main fc31e97f9. After = the tip of `backplane/snappy` named in each table. Method: README.md (input via CDP,
the recorder in the page, median / p95 of warm reps after one cold rep).

Machine: AMD Ryzen 7 5800 (8 cores / 16 threads), 31 GB, Linux 7.2; headless Chromium 152 (web) and the desktop app
(Electron 44, `--ozone-platform=headless`). The machine was shared with other agents' builds and hubs (load 7-13);
summaries record the load average at start and end.

States:
- R: a copy of a real history (44 MB log, 16 projects, 146 threads, the longest 11k events), stand-in agents.
- X: R x3 by `mkx.py` (48 projects, 434 threads; two threads with 3x their events, one with 3,327 messages) and 20
  stand-in bots streaming a delta a second while clicking.
- S: the e2e skill's synthetic trial workspace (5 projects, ~20 threads).

Machines: fast (no throttle), slow (Chromium CPU 4x, hub cgroup CPUQuota=100 %), very slow (6x, 50 %). Network: RTT
added by relay.mjs between the browser and the hub.

Columns: p50 / p95 ms from the input event to the frame that shows the result ("shown"); for threads also
"confirmed by hub" (the hub's view replaced the cached picture); "within 16 / 50 / 100" = share of interactions whose
p50 is under that.

## Final: tip dd000ff75 (the code at the 09:27 code freeze) vs base fc31e97f9
Both with an 80 ms press hold (a person's click; the tip acts at the press, base at the release), metric px (the end of
the frame that holds the change), warm p50 / p95 ms, reps 3-5 (final.sh). R = real-data copy, X = R x3 + 20 streaming
bots, fast / slow (4x CPU + hub 1 core) / worst (6x + hub 50 % + RTT 100 + bots); RTT via relay.mjs.

## Summary
Click-to-render, main fc31e97f9 -> dd000ff75: input event -> the frame that shows the result, warm p50, an 80 ms press
hold (a person's click). Real-data copy (R), the same x3 with 20 bots streaming (X), web unless noted; slow = 4x CPU
+ hub on one core; worst = 6x CPU + hub at half a core + RTT 100 ms + bots.

| cell | n | median p50 before -> after (ms) | slowest 80 %: mean p50 before -> after | geo-mean speed-up | within 50 ms | within 100 ms | CLS-rule shifting interactions before -> after | late items/rep before -> after (late in both + NEW) |
|---|---:|---|---|---:|---|---|---|---|
| R-fast-web | 30 | 160 -> 32 | 372 -> 47 (7.9x) | 4.6x | 3/30 -> 24/30 | 6/30 -> 26/30 | 1 -> 0 | 27.6 -> 22.2 + 41.8 new |
| R-fast-web-rtt100 | 20 | 255 -> 28 | 569 -> 70 (8.1x) | 5.8x | 2/20 -> 16/20 | 2/20 -> 16/20 | 1 -> 0 | 23.7 -> 20.3 + 26.3 new |
| R-slow-web | 15 | 338 -> 131 | 787 -> 148 (5.3x) | 3.5x | 0/15 -> 1/15 | 0/15 -> 5/15 | 2 -> 0 | 13.7 -> 12.3 + 29.0 new |
| X-fast-web | 20 | 315 -> 46 | 707 -> 62 (11.3x) | 5.6x | 2/20 -> 14/20 | 3/20 -> 17/20 | 2 -> 0 | 15.8 -> 15.0 + 36.3 new |
| X-vslow-web-rtt100 | 15 | 1294 -> 240 | 1550 -> 316 (4.9x) | 4.3x | 1/15 -> 2/15 | 1/15 -> 2/15 | — -> 0 | 8.3 -> 0.0 + 13.7 new |
| R-fast-web-rtt30 (97abcb9ea) | 20 | 192 -> 34 | 503 -> 57 (8.9x) | 4.8x | 2/20 -> 14/20 | 2/20 -> 17/20 | — -> — | 26.5 -> 12.5 + 18.5 new |
| R-fast-web-rtt250 (97abcb9ea) | 20 | 416 -> 37 | 733 -> 115 (6.4x) | 6.1x | 2/20 -> 16/20 | 2/20 -> 16/20 | — -> — | 14.2 -> 7.8 + 28.5 new |
| R-vslow-web (97abcb9ea) | 10 | 596 -> 219 | 657 -> 230 (2.9x) | 2.9x | 0/10 -> 0/10 | 0/10 -> 1/10 | — -> — | 3.2 -> 1.0 + 5.2 new |
| R-fast-desk (97abcb9ea) | 20 | 160 -> 37 | 475 -> 55 (8.6x) | 4.0x | 2/20 -> 14/20 | 3/20 -> 17/20 | — -> — | 23.2 -> 9.8 + 8.2 new |
| R-fast-phone (97abcb9ea) | 9 | 150 -> 135 | 206 -> 148 (1.4x) | 1.2x | 1/9 -> 1/9 | 1/9 -> 1/9 | — -> — | 14.2 -> 3.8 + 11.0 new |
| S-fast-web (97abcb9ea) | 19 | 36 -> 36 | 62 -> 62 (1.0x) | 1.0x | 14/19 -> 13/19 | 17/19 -> 17/19 | — -> — | 6.0 -> 4.8 + 1.5 new |

Late content: "NEW" = late only on the tip because the result now shows earlier. On R-fast ~30 of the ~42 NEW items per
rep are viewer content after the now-instant frame (Diff's file list 18, 3D's toolbar and status 12); the rest: every
viewer's Close button still added ~100-370 ms late, Board/Schematic's history bar (~175 ms), a sidebar shelf on the
far-thread jump, fold's "Read full activity text", the token popover body re-added. Shift: by Chromium's CLS rule (no
recent input) 0 interactions shift on the tip in every cell (base: the Diff viewer's late shift and 1-2 others); all
remaining shifts are within 500 ms of the click (unchanged count, e.g. every viewer open 0.144, thread switch 0.046).

Notes: rows marked (97abcb9ea) come from the near-final round (07:36, code before C's last fixes); R-slow has 15 of 20
comparable rows (a base re-run was cut short); the phone's taps act on the touch's release on both builds, so the 80 ms
hold costs it what desktop no longer pays; S (synthetic) is not used for claims (its base already acts at the press).
16 ms: the vsync floor here is ~17 ms, so "within 16" is not a meaningful target; the floor is the next frame.

Worst 10 remaining (p50): 
1. palette-type — X-vslow-web-rtt100: 680 ms
2. esc-back — X-vslow-web-rtt100: 523 ms
3. project-home — X-vslow-web-rtt100: 485 ms
4. palette-type — R-fast-web-rtt250: 406 ms
5. project-home — R-fast-web-rtt250: 377 ms
6. scroll-up — R-fast-web-rtt250: 376 ms
7. viewer-board — X-vslow-web-rtt100: 367 ms
8. project-home — R-vslow-web: 344 ms
9. thread-near — R-vslow-web: 339 ms
10. palette-type — R-vslow-web: 335 ms

## Speed, final tip (cells measured 09:28-09:54, load 2-3; base final-phase cells 05:05-09:27, load 3-5)
| cell | n | median p50 before -> after | slowest 80 %: mean p50 before -> after | geo-mean speed-up | within 16 ms | within 50 ms | within 100 ms | shift>0.01 (all) | shift>0.01 (CLS: no recent input) | late>0 |
|---|---:|---|---|---:|---|---|---|---|---|---|
| R-fast-web | 30 | 160 -> 32 | 372 -> 47 (7.9x) | 4.6x | 1/30 -> 1/30 | 3/30 -> 24/30 | 6/30 -> 26/30 | 16 -> 16 | 1 -> 0 | 6 -> 13 |
| R-fast-web-rtt100 | 20 | 255 -> 28 | 569 -> 70 (8.1x) | 5.8x | 1/20 -> 1/20 | 2/20 -> 16/20 | 2/20 -> 16/20 | 9 -> 9 | 1 -> 0 | 4 -> 7 |
| R-slow-web | 15 | 338 -> 131 | 787 -> 148 (5.3x) | 3.5x | 0/15 -> 0/15 | 0/15 -> 1/15 | 0/15 -> 5/15 | 9 -> 10 | 1 -> 0 | 2 -> 7 |
| X-fast-web | 20 | 315 -> 46 | 707 -> 62 (11.3x) | 5.6x | 0/20 -> 0/20 | 2/20 -> 14/20 | 3/20 -> 17/20 | 7 -> 8 | 1 -> 0 | 3 -> 6 |
| X-vslow-web-rtt100 | 15 | 1294 -> 240 | 1550 -> 316 (4.9x) | 4.3x | 0/15 -> 0/15 | 1/15 -> 2/15 | 1/15 -> 2/15 | 5 -> 6 | — -> 0 | 1 -> 2 |

## Speed, near-final tip 97abcb9ea (07:36; cells the final window had no time for)
| cell | n | median p50 before -> after | slowest 80 %: mean p50 before -> after | geo-mean speed-up | within 16 ms | within 50 ms | within 100 ms | shift>0.01 (all) | shift>0.01 (CLS: no recent input) | late>0 |
|---|---:|---|---|---:|---|---|---|---|---|---|
| R-fast-web-rtt30 | 20 | 192 -> 34 | 503 -> 57 (8.9x) | 4.8x | 1/20 -> 1/20 | 2/20 -> 14/20 | 2/20 -> 17/20 | 10 -> 10 | — -> — | 6 -> 10 |
| R-fast-web-rtt250 | 20 | 416 -> 37 | 733 -> 115 (6.4x) | 6.1x | 1/20 -> 1/20 | 2/20 -> 16/20 | 2/20 -> 16/20 | 10 -> 10 | — -> — | 4 -> 10 |
| R-vslow-web | 10 | 596 -> 219 | 657 -> 230 (2.9x) | 2.9x | 0/10 -> 0/10 | 0/10 -> 0/10 | 0/10 -> 1/10 | 5 -> 4 | — -> — | 2 -> 4 |
| R-fast-desk | 20 | 160 -> 37 | 475 -> 55 (8.6x) | 4.0x | 1/20 -> 1/20 | 2/20 -> 14/20 | 3/20 -> 17/20 | 9 -> 9 | — -> — | 6 -> 9 |
| R-fast-phone | 9 | 150 -> 135 | 206 -> 148 (1.4x) | 1.2x | 1/9 -> 1/9 | 1/9 -> 1/9 | 1/9 -> 1/9 | 1 -> 1 | — -> — | 3 -> 3 |
| S-fast-web | 19 | 36 -> 36 | 62 -> 62 (1.0x) | 1.0x | 1/19 -> 1/19 | 14/19 -> 13/19 | 17/19 -> 17/19 | 4 -> 4 | — -> — | 1 -> 1 |

## Layout shift and late content, final tip
All shifts (every layout-shift entry during settle) and, per Chromium's CLS rule, only those without recent input:
| cell | interactions | shifting (>0.01) before -> after | mean CLS / interaction before -> after | late items / rep, all interactions: before | after, late in both | after, NEW (shown earlier) |
|---|---:|---|---|---:|---:|---:|
| R-fast-web | 30 | 16 -> 16 | 0.057 -> 0.051 | 27.6 | 22.2 | 41.8 |
| R-fast-web-rtt100 | 20 | 8 -> 9 | 0.040 -> 0.037 | 23.7 | 20.3 | 26.3 |
| R-slow-web | 20 | 8 -> 10 | 0.045 -> 0.053 | 13.7 | 12.3 | 29.0 |
| X-fast-web | 20 | 7 -> 8 | 0.034 -> 0.036 | 15.8 | 15.0 | 36.3 |
| X-vslow-web-rtt100 | 15 | 5 -> 6 | 0.025 -> 0.034 | 8.3 | 0.0 | 13.7 |

CLS rule (hadRecentInput=false only; base runs before 09:00 do not record it: "—"):
| cell | interactions | shifting (>0.01) before -> after | mean CLS / interaction before -> after | late items / rep, all interactions: before | after, late in both | after, NEW (shown earlier) |
|---|---:|---|---|---:|---:|---:|
| R-fast-web | 30 | 1 -> 0 | 0.005 -> 0.000 | 27.6 | 22.2 | 41.8 |
| R-fast-web-rtt100 | 20 | 1 -> 0 | 0.007 -> 0.000 | 23.7 | 20.3 | 26.3 |
| R-slow-web | 20 | 2 -> 0 | 0.010 -> 0.000 | 13.7 | 12.3 | 29.0 |
| X-fast-web | 20 | 2 -> 0 | 0.009 -> 0.000 | 15.8 | 15.0 | 36.3 |
| X-vslow-web-rtt100 | 15 | — -> 0 | — -> 0.000 | 8.3 | 0.0 | 13.7 |
"NEW" late items = late only on the tip because it now shows the result earlier (the rest follows: e.g. the Diff's
file list after its instant frame, a viewer's Close button); "late in both" = also late on base.

## Per-cell detail (final tip)
| R-slow-web | 15 | 338 -> 131 | 787 -> 148 (5.3x) | 3.5x | 0/15 -> 0/15 | 0/15 -> 1/15 | 0/15 -> 5/15 | 9 -> 10 | 1 -> 0 | 2 -> 7 |
| X-fast-web | 20 | 315 -> 46 | 707 -> 62 (11.3x) | 5.6x | 0/20 -> 0/20 | 2/20 -> 14/20 | 3/20 -> 17/20 | 7 -> 8 | 1 -> 0 | 3 -> 6 |
| X-vslow-web-rtt100 | 15 | 1294 -> 240 | 1550 -> 316 (4.9x) | 4.3x | 0/15 -> 0/15 | 1/15 -> 2/15 | 1/15 -> 2/15 | 5 -> 6 | — -> 0 | 1 -> 2 |

## R-fast-web

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-diff | 4568 / 4636 | 26 / 27 | 175.0x | — -> — | 4569 -> 241 | 0.144 -> 0.144 |
| viewer-board | 277 / 299 | 37 / 47 | 7.6x | — -> — | 308 -> 143 | 0.144 -> 0.144 |
| viewer-fm | 277 / 321 | 27 / 28 | 10.3x | — -> — | 567 -> 154 | 0.144 -> 0.144 |
| viewer-3d | 263 / 275 | 32 / 188 | 8.3x | — -> — | 1038 -> 567 | 0.144 -> 0.144 |
| viewer-schematic | 236 / 245 | 32 / 37 | 7.5x | — -> — | 258 -> 117 | 0.144 -> 0.144 |
| search-type | 234 / 234 | 50 / 50 | 4.7x | — -> — | 234 -> 50 | 0.013 -> 0.006 |
| tool-fold | 226 / 259 | 19 / 20 | 11.7x | — -> — | 228 -> 58 | 0.000 -> 0.000 |
| token-popover | 222 / 232 | 18 / 18 | 12.6x | — -> — | 223 -> 95 | 0.000 -> 0.000 |
| viewer-browser | 222 / 244 | 25 / 27 | 8.9x | — -> — | 362 -> 116 | 0.144 -> 0.144 |
| account-picker | 217 / 241 | 17 / 18 | 12.5x | — -> — | 346 -> 94 | 0.000 -> 0.000 |
| project-look | 205 / 215 | 157 / 172 | 1.3x | — -> — | 206 -> 167 | 0.025 -> 0.025 |
| palette-type | 204 / 206 | 142 / 142 | 1.4x | — -> — | 205 -> 143 | 0.002 -> 0.046 |
| settings-open | 178 / 179 | 18 / 21 | 9.9x | — -> — | 256 -> 38 | 0.000 -> 0.000 |
| settings-tab | 169 / 182 | 42 / 46 | 4.1x | — -> — | 170 -> 100 | 0.116 -> 0.117 |
| scroll-up | 160 / 163 | 108 / 118 | 1.5x | — -> — | 292 -> 172 | 0.000 -> 0.000 |
| esc-back | 155 / 164 | 48 / 50 | 3.2x | 142 -> 42 | 294 -> 73 | 0.000 -> 0.000 |
| project-home | 154 / 165 | 126 / 137 | 1.2x | — -> — | 202 -> 133 | 0.039 -> 0.039 |
| thread-near | 146 / 152 | 36 / 39 | 4.0x | 240 -> 70 | 370 -> 117 | 0.046 -> 0.046 |
| thread-short | 146 / 155 | 34 / 34 | 4.3x | 178 -> 62 | 247 -> 116 | 0.076 -> 0.076 |
| thread-project | 143 / 154 | 34 / 34 | 4.3x | 205 -> 68 | 302 -> 117 | 0.004 -> 0.004 |
| thread-long | 143 / 144 | 33 / 35 | 4.3x | 237 -> 64 | 372 -> 116 | 0.046 -> 0.046 |
| bot-page | 136 / 145 | 26 / 30 | 5.1x | — -> — | 219 -> 100 | 0.028 -> 0.028 |
| palette-open | 127 / 148 | 33 / 33 | 3.9x | — -> — | 129 -> 68 | 0.000 -> 0.000 |
| model-picker | 122 / 124 | 18 / 20 | 6.8x | — -> — | 162 -> 41 | 0.000 -> 0.000 |
| thread-far | 99 / 111 | 57 / 57 | 1.7x | 246 -> 96 | 246 -> 101 | 0.248 -> 0.060 |
| search-open | 94 / 94 | 17 / 18 | 5.4x | — -> — | 94 -> 17 | 0.000 -> 0.000 |
| thread-menu | 89 / 100 | 17 / 17 | 5.2x | — -> — | 89 -> 17 | 0.000 -> 0.000 |
| send | 37 / 40 | 31 / 31 | 1.2x | — -> — | 396 -> 346 | 0.000 -> 0.000 |
| resize | 18 / 19 | 18 / 18 | 1.0x | — -> — | 18 -> 32 | 0.163 -> 0.186 |
| type-key | 16 / 16 | 16 / 16 | 1.0x | — -> — | 193 -> 32 | 0.000 -> 0.000 |

## R-fast-web-rtt100

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-diff | 5263 / 5634 | 22 / 24 | 236.0x | — -> — | 5266 -> 524 | 0.144 -> 0.144 |
| viewer-board | 374 / 413 | 35 / 37 | 10.7x | — -> — | 468 -> 296 | 0.144 -> 0.144 |
| palette-type | 350 / 350 | 244 / 244 | 1.4x | — -> — | 352 -> 250 | 0.018 -> 0.046 |
| tool-fold | 329 / 336 | 18 / 18 | 18.5x | — -> — | 332 -> 162 | 0.000 -> 0.000 |
| viewer-fm | 321 / 322 | 23 / 23 | 14.1x | — -> — | 687 -> 431 | 0.144 -> 0.144 |
| account-picker | 320 / 323 | 17 / 18 | 18.4x | — -> — | 656 -> 158 | 0.000 -> 0.000 |
| palette-open | 319 / 370 | 16 / 32 | 19.6x | — -> — | 323 -> 158 | 0.000 -> 0.000 |
| settings-open | 285 / 286 | 17 / 19 | 16.6x | — -> — | 566 -> 137 | 0.000 -> 0.000 |
| scroll-up | 261 / 277 | 206 / 207 | 1.3x | — -> — | 523 -> 372 | 0.000 -> 0.000 |
| project-home | 255 / 260 | 223 / 224 | 1.1x | — -> — | 503 -> 233 | 0.039 -> 0.039 |
| esc-back | 254 / 259 | 48 / 49 | 5.3x | 247 -> 41 | 544 -> 176 | 0.000 -> 0.000 |
| thread-far | 212 / 233 | 137 / 138 | 1.5x | 488 -> 286 | 488 -> 290 | 0.111 -> 0.066 |
| thread-project | 144 / 147 | 28 / 32 | 5.2x | 305 -> 155 | 567 -> 160 | 0.004 -> 0.004 |
| thread-short | 137 / 142 | 28 / 28 | 5.0x | 268 -> 160 | 525 -> 160 | 0.076 -> 0.076 |
| thread-long | 136 / 142 | 26 / 27 | 5.2x | 335 -> 161 | 647 -> 161 | 0.046 -> 0.046 |
| thread-near | 136 / 142 | 30 / 34 | 4.5x | 337 -> 160 | 645 -> 160 | 0.046 -> 0.046 |
| model-picker | 118 / 119 | 17 / 18 | 6.8x | — -> — | 263 -> 124 | 0.000 -> 0.000 |
| thread-menu | 101 / 101 | 17 / 17 | 6.0x | — -> — | 101 -> 17 | 0.000 -> 0.000 |
| send | 35 / 48 | 29 / 29 | 1.2x | — -> — | 853 -> 634 | 0.000 -> 0.000 |
| type-key | 15 / 16 | 16 / 16 | 1.0x | — -> — | 300 -> 32 | 0.000 -> 0.000 |

## R-slow-web

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-diff | 5258 / 5299 | 117 / 118 | 45.0x | — -> — | 5266 -> 524 | 0.144 -> 0.144 |
| viewer-board | 710 / 1025 | 141 / 142 | 5.0x | — -> — | 820 -> 434 | 0.144 -> 0.144 |
| thread-near | 419 / 468 | 247 / 263 | 1.7x | 634 -> 238 | 851 -> 247 | 0.046 -> 0.046 |
| project-home | 414 / 442 | 249 / 256 | 1.7x | — -> — | 519 -> 254 | 0.039 -> 0.039 |
| thread-far | 394 / 448 | 147 / 152 | 2.7x | 607 -> 324 | 646 -> 339 | 0.251 -> 0.330 |
| viewer-fm | 364 / 368 | 109 / 110 | 3.3x | — -> — | 710 -> 461 | 0.144 -> 0.144 |
| account-picker | 339 / 364 | 55 / 68 | 6.1x | — -> — | 644 -> 178 | 0.000 -> 0.000 |
| thread-project | 338 / 367 | 143 / 244 | 2.4x | 521 -> 257 | 727 -> 279 | 0.004 -> 0.034 |
| thread-short | 322 / 335 | 146 / 252 | 2.2x | 491 -> 248 | 615 -> 265 | 0.076 -> 0.109 |
| palette-type | 314 / 314 | 217 / 217 | 1.4x | — -> — | 319 -> 222 | 0.018 -> 0.046 |
| thread-long | 304 / 305 | 131 / 261 | 2.3x | 442 -> 253 | 628 -> 261 | 0.046 -> 0.046 |
| settings-open | 262 / 264 | 70 / 71 | 3.7x | — -> — | 376 -> 119 | 0.000 -> 0.000 |
| palette-open | 229 / 240 | 67 / 82 | 3.4x | — -> — | 234 -> 144 | 0.000 -> 0.000 |
| model-picker | 198 / 199 | 57 / 58 | 3.5x | — -> — | 251 -> 97 | 0.000 -> 0.000 |
| thread-menu | 117 / 131 | 19 / 19 | 6.2x | — -> — | 118 -> 19 | 0.000 -> 0.000 |
| tool-fold | — / — | 60 / 65 | unmeasured | | |
| type-key | — / — | 18 / 19 | unmeasured | | |
| scroll-up | — / — | 148 / 166 | unmeasured | | |
| send | — / — | 68 / 70 | unmeasured | | |
| esc-back | — / — | 117 / 120 | unmeasured | | |

## X-fast-web

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-diff | 5548 / 5610 | 48 / 53 | 115.4x | — -> — | 5553 -> 349 | 0.144 -> 0.144 |
| account-picker | 730 / 734 | 19 / 22 | 38.2x | — -> — | 1100 -> 127 | 0.000 -> 0.000 |
| viewer-board | 658 / 820 | 71 / 79 | 9.2x | — -> — | 706 -> 253 | 0.144 -> 0.144 |
| tool-fold | 657 / 688 | 24 / 24 | 27.7x | — -> — | 1058 -> 108 | 0.000 -> 0.000 |
| viewer-fm | 504 / 522 | 49 / 51 | 10.2x | — -> — | 548 -> 304 | 0.144 -> 0.144 |
| esc-back | 502 / 609 | 95 / 98 | 5.3x | 477 -> 80 | 518 -> 165 | 0.000 -> 0.000 |
| settings-open | 496 / 593 | 32 / 45 | 15.3x | — -> — | 926 -> 78 | 0.000 -> 0.000 |
| palette-type | 477 / 477 | 202 / 202 | 2.4x | — -> — | 478 -> 203 | 0.007 -> 0.070 |
| palette-open | 406 / 438 | 33 / 49 | 12.3x | — -> — | 440 -> 131 | 0.000 -> 0.000 |
| project-home | 315 / 356 | 159 / 162 | 2.0x | — -> — | 463 -> 160 | 0.003 -> 0.003 |
| thread-short | 202 / 214 | 46 / 48 | 4.3x | 320 -> 88 | 1022 -> 116 | 0.082 -> 0.076 |
| thread-project | 190 / 192 | 47 / 54 | 4.1x | 441 -> 108 | 692 -> 133 | 0.005 -> 0.007 |
| thread-near | 184 / 204 | 44 / 47 | 4.2x | 495 -> 136 | 524 -> 139 | 0.046 -> 0.046 |
| thread-long | 174 / 187 | 44 / 44 | 4.0x | 530 -> 122 | 530 -> 126 | 0.046 -> 0.046 |
| model-picker | 138 / 149 | 22 / 23 | 6.4x | — -> — | 306 -> 46 | 0.000 -> 0.000 |
| thread-far | 126 / 128 | 63 / 64 | 2.0x | 642 -> 170 | 660 -> 176 | 0.056 -> 0.056 |
| thread-menu | 109 / 111 | 18 / 18 | 6.0x | — -> — | 234 -> 18 | 0.000 -> 0.000 |
| scroll-up **REGRESSION** | 68 / 314 | 144 / 167 | 0.5x | — -> — | 602 -> 239 | 0.000 -> 0.000 |
| send | 49 / 56 | 37 / 40 | 1.3x | — -> — | 1150 -> 631 | 0.000 -> 0.000 |
| type-key | 18 / 21 | 18 / 18 | 1.0x | — -> — | 342 -> 32 | 0.000 -> 0.000 |

## X-vslow-web-rtt100

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-board | 2540 / 2540 | 367 / 376 | 6.9x | — -> — | 2716 -> 367 | 0.144 -> 0.144 |
| viewer-fm | 2400 / 2490 | 297 / 318 | 8.1x | — -> — | 2679 -> 796 | 0.144 -> 0.144 |
| palette-type | 2361 / 2361 | 680 / 680 | 3.5x | — -> — | 2364 -> 684 | 0.007 -> 0.070 |
| esc-back | 2158 / 2336 | 523 / 530 | 4.1x | 2052 -> 436 | 2419 -> 975 | 0.000 -> 0.000 |
| tool-fold | 1948 / 1964 | 131 / 143 | 14.8x | — -> — | 1972 -> 132 | 0.000 -> 0.000 |
| thread-far | 1509 / 1998 | 331 / 540 | 4.6x | 3159 -> 806 | 3159 -> 846 | 0.056 -> 0.056 |
| palette-open | 1366 / 2041 | 168 / 182 | 8.1x | — -> — | 1382 -> 584 | 0.000 -> 0.000 |
| settings-open | 1294 / 1379 | 159 / 164 | 8.1x | — -> — | 1400 -> 290 | 0.000 -> 0.000 |
| project-home | 1188 / 1350 | 485 / 584 | 2.4x | — -> — | 1195 -> 489 | 0.003 -> 0.003 |
| thread-near | 741 / 820 | 284 / 309 | 2.6x | 1490 -> 646 | 1490 -> 646 | 0.046 -> 0.046 |
| thread-long | 650 / 658 | 240 / 245 | 2.7x | 1504 -> 623 | 1504 -> 622 | 0.046 -> 0.046 |
| model-picker | 440 / 441 | 129 / 148 | 3.4x | — -> — | 1253 -> 231 | 0.000 -> 0.000 |
| thread-menu | 282 / 299 | 32 / 42 | 8.7x | — -> — | 895 -> 32 | 0.000 -> 0.000 |
| send | 221 / 264 | 155 / 170 | 1.4x | — -> — | 1615 -> 354 | 0.000 -> 0.000 |
| type-key | 40 / 44 | 32 / 32 | 1.2x | — -> — | 50 -> 34 | 0.000 -> 0.000 |

## Worst 10 after

| interaction | cell | p50 |
|---|---|---:|
| palette-type | X-vslow-web-rtt100 | 680 |
| esc-back | X-vslow-web-rtt100 | 523 |
| project-home | X-vslow-web-rtt100 | 485 |
| viewer-board | X-vslow-web-rtt100 | 367 |
| thread-far | X-vslow-web-rtt100 | 331 |
| viewer-fm | X-vslow-web-rtt100 | 297 |
| thread-near | X-vslow-web-rtt100 | 284 |
| project-home | R-slow-web | 249 |
| thread-near | R-slow-web | 247 |
| palette-type | R-fast-web-rtt100 | 244 |

## Regressions (p50 > +20 % and > +10 ms)

- X-fast-web scroll-up: 68 -> 144 ms

