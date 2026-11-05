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

Both runs: an 80 ms press hold, metric px (the end of the frame that holds the change), warm p50 / p95 in ms.
Before: `final-base-fc31e97f9`; after: `tip3-0736-97abcb9ea` (tip 97abcb9ea, 07:36; the final tip at 10:00 is in the PR text).

Notes: with an 80 ms press hold the base waits for the release on almost every click, the tip acts at the press (most
of the gain on menus and pickers). Phone taps act on the touch's release on both. S (synthetic) behaves differently
from R and X on base (most clicks already at the press there) and is not used for claims. Shift columns count every
layout shift during settle, including those right after the input (Chromium's CLS leaves those out); "CLS" columns
appear for runs that record hadRecentInput.

## Speed

| cell | n | median p50 before -> after | slowest 80 %: mean p50 before -> after | geo-mean speed-up | within 16 ms | within 50 ms | within 100 ms | shift>0.01 (all) | shift>0.01 (CLS: no recent input) | late>0 |
|---|---:|---|---|---:|---|---|---|---|---|---|
| R-fast-web | 30 | 166 -> 36 | 392 -> 54 (7.3x) | 4.0x | 1/30 -> 1/30 | 3/30 -> 19/30 | 4/30 -> 25/30 | 16 -> 17 | — -> — | 6 -> 7 |
| R-fast-web-rtt30 | 20 | 192 -> 34 | 503 -> 57 (8.9x) | 4.8x | 1/20 -> 1/20 | 2/20 -> 14/20 | 2/20 -> 17/20 | 10 -> 10 | — -> — | 6 -> 10 |
| R-fast-web-rtt100 | 20 | 260 -> 32 | 585 -> 77 (7.6x) | 5.1x | 1/20 -> 1/20 | 2/20 -> 14/20 | 2/20 -> 16/20 | 9 -> 9 | — -> — | 6 -> 10 |
| R-fast-web-rtt250 | 20 | 416 -> 37 | 733 -> 115 (6.4x) | 6.1x | 1/20 -> 1/20 | 2/20 -> 16/20 | 2/20 -> 16/20 | 10 -> 10 | — -> — | 4 -> 10 |
| R-slow-web | 20 | 374 -> 138 | 708 -> 199 (3.6x) | 2.3x | 0/20 -> 0/20 | 1/20 -> 2/20 | 2/20 -> 6/20 | 10 -> 10 | — -> — | 5 -> 10 |
| R-vslow-web | 10 | 596 -> 219 | 657 -> 230 (2.9x) | 2.9x | 0/10 -> 0/10 | 0/10 -> 0/10 | 0/10 -> 1/10 | 5 -> 4 | — -> — | 2 -> 4 |
| R-fast-desk | 20 | 160 -> 37 | 475 -> 55 (8.6x) | 4.0x | 1/20 -> 1/20 | 2/20 -> 14/20 | 3/20 -> 17/20 | 9 -> 9 | — -> — | 6 -> 9 |
| X-fast-web | 20 | 398 -> 51 | 731 -> 73 (10.0x) | 4.9x | 0/20 -> 0/20 | 2/20 -> 10/20 | 3/20 -> 16/20 | 7 -> 8 | — -> — | 3 -> 7 |
| X-vslow-web-rtt100 | 15 | 1294 -> 310 | 1550 -> 482 (3.2x) | 2.9x | 0/15 -> 0/15 | 1/15 -> 1/15 | 1/15 -> 2/15 | 5 -> 6 | — -> — | 1 -> 3 |
| R-fast-phone | 9 | 150 -> 135 | 206 -> 148 (1.4x) | 1.2x | 1/9 -> 1/9 | 1/9 -> 1/9 | 1/9 -> 1/9 | 1 -> 1 | — -> — | 3 -> 3 |
| S-fast-web | 19 | 36 -> 36 | 62 -> 62 (1.0x) | 1.0x | 1/19 -> 1/19 | 14/19 -> 13/19 | 17/19 -> 17/19 | 4 -> 4 | — -> — | 1 -> 1 |

### R-fast-web

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-diff | 4796 / 4853 | 54 / 70 | 89.6x | — -> — | 4818 -> 435 | 0.144 -> 0.558 |
| viewer-3d | 437 / 669 | 31 / 161 | 14.0x | — -> — | 1535 -> 901 | 0.144 -> 0.144 |
| viewer-board | 286 / 336 | 59 / 76 | 4.8x | — -> — | 319 -> 257 | 0.144 -> 0.414 |
| viewer-schematic | 260 / 273 | 52 / 70 | 5.0x | — -> — | 283 -> 210 | 0.144 -> 0.414 |
| viewer-browser | 247 / 317 | 34 / 53 | 7.2x | — -> — | 401 -> 168 | 0.144 -> 0.414 |
| tool-fold | 246 / 266 | 18 / 22 | 13.6x | — -> — | 249 -> 63 | 0.000 -> 0.000 |
| search-type | 233 / 234 | 50 / 67 | 4.7x | — -> — | 233 -> 50 | 0.013 -> 0.013 |
| viewer-fm | 230 / 269 | 47 / 55 | 4.9x | — -> — | 443 -> 217 | 0.144 -> 0.414 |
| token-popover | 222 / 267 | 18 / 23 | 12.1x | — -> — | 223 -> 98 | 0.000 -> 0.000 |
| account-picker | 219 / 225 | 18 / 20 | 12.5x | — -> — | 344 -> 98 | 0.000 -> 0.000 |
| palette-type | 214 / 231 | 146 / 162 | 1.5x | — -> — | 214 -> 148 | 0.002 -> 0.013 |
| project-look | 214 / 240 | 156 / 157 | 1.4x | — -> — | 214 -> 166 | 0.025 -> 0.025 |
| settings-tab | 182 / 188 | 51 / 57 | 3.6x | — -> — | 184 -> 100 | 0.116 -> 0.116 |
| settings-open | 182 / 187 | 20 / 37 | 9.2x | — -> — | 273 -> 56 | 0.000 -> 0.000 |
| project-home | 166 / 171 | 146 / 161 | 1.1x | — -> — | 223 -> 148 | 0.039 -> 0.039 |
| esc-back | 160 / 173 | 53 / 56 | 3.0x | 151 -> 43 | 295 -> 86 | 0.000 -> 0.000 |
| scroll-up | 159 / 183 | 108 / 136 | 1.5x | — -> — | 292 -> 188 | 0.000 -> 0.000 |
| thread-project | 151 / 154 | 35 / 42 | 4.3x | 204 -> 75 | 311 -> 116 | 0.004 -> 0.006 |
| thread-short | 147 / 153 | 36 / 39 | 4.1x | 187 -> 70 | 262 -> 116 | 0.076 -> 0.076 |
| thread-near | 141 / 146 | 30 / 41 | 4.7x | 247 -> 72 | 395 -> 116 | 0.046 -> 0.046 |
| thread-long | 140 / 143 | 31 / 43 | 4.5x | 229 -> 69 | 368 -> 116 | 0.046 -> 0.046 |
| bot-page | 139 / 140 | 40 / 44 | 3.5x | — -> — | 220 -> 116 | 0.028 -> 0.028 |
| palette-open | 125 / 136 | 32 / 39 | 3.8x | — -> — | 126 -> 67 | 0.000 -> 0.000 |
| model-picker | 120 / 122 | 23 / 30 | 5.2x | — -> — | 178 -> 44 | 0.000 -> 0.000 |
| search-open | 101 / 102 | 101 / 102 | 1.0x | — -> — | 101 -> 101 | 0.000 -> 0.000 |
| thread-menu | 100 / 101 | 17 / 17 | 5.9x | — -> — | 100 -> 17 | 0.000 -> 0.000 |
| thread-far | 98 / 99 | 59 / 109 | 1.7x | 245 -> 107 | 245 -> 112 | 0.248 -> 0.248 |
| send | 38 / 52 | 33 / 36 | 1.1x | — -> — | 411 -> 351 | 0.000 -> 0.000 |
| resize | 19 / 19 | 18 / 19 | 1.0x | — -> — | 19 -> 32 | 0.163 -> 0.163 |
| type-key | 15 / 16 | 16 / 16 | 1.0x | — -> — | 195 -> 32 | 0.000 -> 0.000 |

### R-fast-web-rtt30

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-diff | 4933 / 4985 | 27 / 28 | 182.0x | — -> — | 4935 -> 393 | 0.144 -> 0.558 |
| viewer-board | 286 / 291 | 44 / 48 | 6.5x | — -> — | 319 -> 186 | 0.144 -> 0.144 |
| account-picker | 275 / 296 | 17 / 17 | 15.9x | — -> — | 468 -> 124 | 0.000 -> 0.000 |
| palette-type | 270 / 270 | 179 / 179 | 1.5x | — -> — | 270 -> 181 | 0.018 -> 0.046 |
| viewer-fm | 265 / 280 | 26 / 29 | 10.3x | — -> — | 498 -> 202 | 0.144 -> 0.414 |
| tool-fold | 255 / 256 | 18 / 18 | 14.5x | — -> — | 256 -> 97 | 0.000 -> 0.000 |
| esc-back | 220 / 288 | 52 / 55 | 4.2x | 211 -> 44 | 426 -> 139 | 0.000 -> 0.000 |
| settings-open | 220 / 220 | 18 / 19 | 12.5x | — -> — | 348 -> 76 | 0.000 -> 0.000 |
| project-home | 195 / 200 | 163 / 172 | 1.2x | — -> — | 296 -> 167 | 0.039 -> 0.039 |
| palette-open | 192 / 194 | 17 / 33 | 11.4x | — -> — | 194 -> 94 | 0.000 -> 0.000 |
| scroll-up | 192 / 195 | 144 / 155 | 1.3x | — -> — | 346 -> 258 | 0.000 -> 0.000 |
| thread-project | 174 / 176 | 38 / 40 | 4.6x | 270 -> 108 | 439 -> 120 | 0.034 -> 0.034 |
| thread-short | 154 / 155 | 75 / 89 | 2.1x | 214 -> 143 | 328 -> 149 | 0.237 -> 0.109 |
| thread-near | 149 / 150 | 38 / 42 | 3.9x | 294 -> 103 | 473 -> 106 | 0.046 -> 0.046 |
| thread-long | 144 / 152 | 33 / 44 | 4.3x | 278 -> 114 | 461 -> 119 | 0.046 -> 0.046 |
| model-picker | 124 / 124 | 17 / 22 | 7.1x | — -> — | 213 -> 58 | 0.000 -> 0.000 |
| thread-far | 114 / 130 | 94 / 99 | 1.2x | 295 -> 202 | 297 -> 207 | 0.248 -> 0.314 |
| thread-menu | 101 / 102 | 17 / 17 | 6.0x | — -> — | 101 -> 17 | 0.000 -> 0.000 |
| send | 45 / 51 | 34 / 38 | 1.3x | — -> — | 540 -> 485 | 0.000 -> 0.000 |
| type-key | 16 / 16 | 16 / 16 | 1.0x | — -> — | 250 -> 31 | 0.000 -> 0.000 |

### R-fast-web-rtt100

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-diff | 5491 / 6116 | 24 / 26 | 229.8x | — -> — | 5718 -> 544 | 0.144 -> 0.144 |
| account-picker | 427 / 462 | 17 / 17 | 25.4x | — -> — | 827 -> 176 | 0.000 -> 0.000 |
| viewer-board | 373 / 455 | 32 / 40 | 11.7x | — -> — | 468 -> 294 | 0.144 -> 0.144 |
| viewer-fm | 341 / 439 | 24 / 25 | 14.4x | — -> — | 710 -> 454 | 0.144 -> 0.144 |
| tool-fold | 326 / 330 | 18 / 18 | 17.9x | — -> — | 328 -> 160 | 0.000 -> 0.000 |
| palette-type | 323 / 323 | 258 / 258 | 1.2x | — -> — | 323 -> 263 | 0.018 -> 0.046 |
| settings-open | 309 / 352 | 17 / 21 | 18.0x | — -> — | 606 -> 138 | 0.000 -> 0.000 |
| project-home | 266 / 266 | 227 / 230 | 1.2x | — -> — | 521 -> 233 | 0.039 -> 0.039 |
| scroll-up | 260 / 261 | 208 / 209 | 1.2x | — -> — | 516 -> 373 | 0.000 -> 0.000 |
| esc-back | 260 / 268 | 50 / 52 | 5.2x | 253 -> 43 | 566 -> 186 | 0.000 -> 0.000 |
| palette-open | 252 / 324 | 32 / 32 | 7.9x | — -> — | 254 -> 182 | 0.000 -> 0.000 |
| thread-far | 187 / 189 | 160 / 167 | 1.2x | 438 -> 324 | 438 -> 331 | 0.254 -> 0.112 |
| thread-project | 140 / 146 | 41 / 53 | 3.5x | 318 -> 179 | 626 -> 182 | 0.004 -> 0.004 |
| thread-near | 140 / 140 | 36 / 40 | 3.9x | 337 -> 184 | 638 -> 184 | 0.046 -> 0.046 |
| thread-short | 136 / 138 | 53 / 56 | 2.6x | 268 -> 194 | 517 -> 194 | 0.076 -> 0.076 |
| thread-long | 136 / 137 | 36 / 38 | 3.8x | 322 -> 179 | 632 -> 179 | 0.046 -> 0.046 |
| model-picker | 131 / 132 | 20 / 23 | 6.7x | — -> — | 302 -> 143 | 0.000 -> 0.000 |
| thread-menu | 101 / 101 | 18 / 18 | 5.6x | — -> — | 101 -> 18 | 0.000 -> 0.000 |
| send | 34 / 38 | 29 / 30 | 1.2x | — -> — | 727 -> 665 | 0.000 -> 0.000 |
| type-key | 16 / 16 | 16 / 16 | 1.0x | — -> — | 292 -> 32 | 0.000 -> 0.000 |

### R-fast-web-rtt250

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-diff | 6022 / 6334 | 26 / 28 | 235.2x | — -> — | 6023 -> 326 | 0.144 -> 0.144 |
| viewer-fm | 567 / 587 | 24 / 26 | 24.1x | — -> — | 842 -> 888 | 0.144 -> 0.144 |
| tool-fold | 545 / 562 | 18 / 33 | 29.9x | — -> — | 547 -> 326 | 0.000 -> 0.000 |
| viewer-board | 536 / 536 | 37 / 41 | 14.3x | — -> — | 772 -> 599 | 0.144 -> 0.144 |
| settings-open | 497 / 510 | 20 / 24 | 25.0x | — -> — | 744 -> 289 | 0.000 -> 0.000 |
| account-picker | 491 / 546 | 18 / 18 | 28.1x | — -> — | 731 -> 308 | 0.000 -> 0.000 |
| palette-type | 482 / 482 | 406 / 406 | 1.2x | — -> — | 483 -> 407 | 0.018 -> 0.046 |
| esc-back | 429 / 430 | 39 / 51 | 10.9x | 419 -> 36 | 641 -> 322 | 0.000 -> 0.000 |
| scroll-up | 419 / 511 | 376 / 380 | 1.1x | — -> — | 420 -> 387 | 0.000 -> 0.000 |
| project-home | 416 / 427 | 377 / 389 | 1.1x | — -> — | 428 -> 384 | 0.039 -> 0.039 |
| palette-open | 401 / 434 | 16 / 17 | 24.3x | — -> — | 403 -> 316 | 0.000 -> 0.000 |
| thread-far | 339 / 345 | 311 / 312 | 1.1x | 729 -> 623 | 729 -> 628 | 0.254 -> 0.314 |
| thread-short | 158 / 158 | 39 / 39 | 4.1x | 426 -> 321 | 426 -> 325 | 0.237 -> 0.109 |
| thread-project | 149 / 160 | 45 / 55 | 3.3x | 495 -> 339 | 495 -> 346 | 0.034 -> 0.034 |
| thread-near | 140 / 145 | 46 / 52 | 3.0x | 481 -> 341 | 481 -> 341 | 0.046 -> 0.046 |
| thread-long | 140 / 142 | 43 / 55 | 3.2x | 493 -> 338 | 493 -> 338 | 0.046 -> 0.046 |
| model-picker | 124 / 131 | 18 / 21 | 6.8x | — -> — | 429 -> 275 | 0.000 -> 0.000 |
| thread-menu | 101 / 102 | 17 / 17 | 6.0x | — -> — | 101 -> 17 | 0.000 -> 0.000 |
| send | 39 / 43 | 31 / 38 | 1.3x | — -> — | 1282 -> 929 | 0.000 -> 0.000 |
| type-key | 15 / 16 | 16 / 16 | 1.0x | — -> — | 166 -> 31 | 0.000 -> 0.000 |

### R-slow-web

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-diff | 5774 / 5810 | 127 / 220 | 45.5x | — -> — | 5818 -> 648 | 0.144 -> 0.144 |
| viewer-board | 471 / 539 | 223 / 232 | 2.1x | — -> — | 595 -> 576 | 0.144 -> 0.144 |
| esc-back | 455 / 473 | 204 / 243 | 2.2x | 384 -> 170 | 728 -> 331 | 0.000 -> 0.000 |
| viewer-fm | 440 / 622 | 138 / 140 | 3.2x | — -> — | 810 -> 590 | 0.144 -> 0.144 |
| thread-short | 396 / 418 | 311 / 344 | 1.3x | 570 -> 299 | 733 -> 313 | 0.237 -> 0.109 |
| account-picker | 391 / 425 | 61 / 81 | 6.4x | — -> — | 705 -> 198 | 0.000 -> 0.000 |
| tool-fold | 387 / 389 | 115 / 127 | 3.4x | — -> — | 396 -> 288 | 0.000 -> 0.000 |
| thread-project | 380 / 422 | 247 / 252 | 1.5x | 552 -> 263 | 702 -> 286 | 0.036 -> 0.034 |
| thread-far | 380 / 462 | 234 / 253 | 1.6x | 592 -> 503 | 597 -> 524 | 0.251 -> 0.330 |
| project-home | 374 / 412 | 333 / 541 | 1.1x | — -> — | 446 -> 341 | 0.039 -> 0.039 |
| thread-near | 372 / 382 | 292 / 302 | 1.3x | 562 -> 294 | 731 -> 304 | 0.046 -> 0.046 |
| thread-long | 358 / 381 | 122 / 140 | 2.9x | 535 -> 246 | 705 -> 256 | 0.046 -> 0.046 |
| palette-type **REGRESSION** | 332 / 332 | 417 / 417 | 0.8x | — -> — | 336 -> 425 | 0.018 -> 0.046 |
| settings-open | 305 / 313 | 97 / 99 | 3.1x | — -> — | 446 -> 153 | 0.000 -> 0.000 |
| palette-open | 270 / 300 | 91 / 136 | 3.0x | — -> — | 274 -> 199 | 0.000 -> 0.000 |
| scroll-up | 233 / 250 | 176 / 271 | 1.3x | — -> — | 431 -> 328 | 0.000 -> 0.000 |
| model-picker | 217 / 218 | 110 / 125 | 2.0x | — -> — | 288 -> 172 | 0.000 -> 0.000 |
| thread-menu | 129 / 131 | 32 / 37 | 4.1x | — -> — | 129 -> 32 | 0.000 -> 0.000 |
| send | 88 / 96 | 72 / 80 | 1.2x | — -> — | 786 -> 516 | 0.000 -> 0.000 |
| type-key **REGRESSION** | 18 / 30 | 30 / 39 | 0.6x | — -> — | 281 -> 36 | 0.000 -> 0.000 |

### R-vslow-web

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| thread-far | 841 / 972 | 219 / 222 | 3.8x | 1326 -> 494 | 1349 -> 507 | 0.063 -> 0.076 |
| project-home | 824 / 938 | 344 / 353 | 2.4x | — -> — | 959 -> 351 | 0.037 -> 0.003 |
| viewer-board | 818 / 1589 | 236 / 240 | 3.5x | — -> — | 967 -> 678 | 0.144 -> 0.144 |
| tool-fold | 609 / 650 | 110 / 113 | 5.5x | — -> — | 620 -> 244 | 0.000 -> 0.000 |
| palette-type | 596 / 596 | 335 / 335 | 1.8x | — -> — | 602 -> 340 | 0.018 -> 0.046 |
| palette-open | 588 / 680 | 110 / 136 | 5.3x | — -> — | 601 -> 254 | 0.000 -> 0.000 |
| thread-near | 544 / 547 | 339 / 428 | 1.6x | 882 -> 657 | 1118 -> 657 | 0.046 -> 0.046 |
| settings-open | 434 / 437 | 145 / 153 | 3.0x | — -> — | 687 -> 257 | 0.000 -> 0.000 |
| model-picker | 322 / 337 | 94 / 95 | 3.4x | — -> — | 446 -> 151 | 0.000 -> 0.000 |
| send | 229 / 262 | 162 / 167 | 1.4x | — -> — | 1843 -> 866 | 0.000 -> 0.000 |

### R-fast-desk

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-diff | 4857 / 4888 | 37 / 37 | 132.4x | — -> — | 4859 -> 511 | 0.142 -> 0.554 |
| palette-type | 290 / 290 | 155 / 155 | 1.9x | — -> — | 292 -> 156 | 0.019 -> 0.047 |
| viewer-board | 262 / 263 | 43 / 77 | 6.2x | — -> — | 289 -> 212 | 0.142 -> 0.413 |
| account-picker | 243 / 279 | 16 / 18 | 14.7x | — -> — | 394 -> 96 | 0.000 -> 0.000 |
| tool-fold | 228 / 241 | 20 / 20 | 11.5x | — -> — | 230 -> 77 | 0.000 -> 0.000 |
| viewer-fm | 213 / 220 | 50 / 64 | 4.2x | — -> — | 385 -> 251 | 0.142 -> 0.413 |
| settings-open | 184 / 243 | 28 / 62 | 6.6x | — -> — | 296 -> 75 | 0.000 -> 0.000 |
| scroll-up | 176 / 177 | 111 / 140 | 1.6x | — -> — | 309 -> 190 | 0.000 -> 0.000 |
| esc-back | 166 / 170 | 45 / 46 | 3.7x | 155 -> 38 | 332 -> 75 | 0.000 -> 0.000 |
| project-home | 160 / 161 | 167 / 210 | 1.0x | — -> — | 200 -> 168 | 0.037 -> 0.037 |
| palette-open | 151 / 260 | 34 / 38 | 4.5x | — -> — | 152 -> 90 | 0.000 -> 0.000 |
| thread-project | 143 / 145 | 33 / 52 | 4.3x | 228 -> 75 | 357 -> 117 | 0.004 -> 0.006 |
| thread-near | 140 / 146 | 50 / 68 | 2.8x | 230 -> 113 | 363 -> 132 | 0.047 -> 0.047 |
| thread-short | 135 / 139 | 35 / 37 | 3.9x | 176 -> 68 | 248 -> 116 | 0.077 -> 0.077 |
| thread-long | 130 / 138 | 45 / 88 | 2.9x | 260 -> 123 | 424 -> 129 | 0.047 -> 0.047 |
| model-picker | 125 / 137 | 17 / 18 | 7.3x | — -> — | 195 -> 42 | 0.000 -> 0.000 |
| thread-menu | 101 / 101 | 18 / 18 | 5.5x | — -> — | 101 -> 18 | 0.000 -> 0.000 |
| thread-far | 96 / 104 | 70 / 96 | 1.4x | 298 -> 168 | 298 -> 173 | 0.256 -> 0.250 |
| send | 35 / 44 | 34 / 43 | 1.0x | — -> — | 448 -> 424 | 0.000 -> 0.000 |
| type-key | 16 / 16 | 16 / 16 | 1.0x | — -> — | 208 -> 31 | 0.000 -> 0.000 |

### X-fast-web

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-diff | 5622 / 5671 | 47 / 61 | 120.7x | — -> — | 5624 -> 460 | 0.144 -> 0.144 |
| account-picker | 781 / 927 | 32 / 34 | 24.1x | — -> — | 956 -> 202 | 0.000 -> 0.000 |
| tool-fold | 718 / 785 | 25 / 27 | 29.1x | — -> — | 719 -> 233 | 0.000 -> 0.000 |
| viewer-board | 710 / 822 | 72 / 89 | 9.9x | — -> — | 763 -> 279 | 0.144 -> 0.144 |
| palette-type | 561 / 561 | 238 / 238 | 2.4x | — -> — | 562 -> 238 | 0.007 -> 0.070 |
| viewer-fm | 520 / 551 | 62 / 72 | 8.4x | — -> — | 925 -> 370 | 0.144 -> 0.144 |
| project-home | 451 / 598 | 162 / 162 | 2.8x | — -> — | 655 -> 162 | 0.003 -> 0.003 |
| esc-back | 430 / 593 | 102 / 102 | 4.2x | 410 -> 86 | 720 -> 178 | 0.000 -> 0.000 |
| settings-open | 408 / 575 | 43 / 52 | 9.4x | — -> — | 715 -> 127 | 0.000 -> 0.000 |
| palette-open | 398 / 421 | 45 / 47 | 8.8x | — -> — | 401 -> 210 | 0.000 -> 0.000 |
| thread-project | 216 / 224 | 51 / 52 | 4.2x | 550 -> 137 | 690 -> 142 | 0.005 -> 0.007 |
| thread-near | 184 / 184 | 74 / 96 | 2.5x | 466 -> 194 | 492 -> 200 | 0.046 -> 0.046 |
| thread-short | 182 / 184 | 47 / 62 | 3.9x | 285 -> 105 | 501 -> 116 | 0.076 -> 0.076 |
| thread-long | 180 / 183 | 75 / 75 | 2.4x | 542 -> 182 | 542 -> 186 | 0.046 -> 0.046 |
| model-picker | 175 / 177 | 24 / 29 | 7.1x | — -> — | 410 -> 50 | 0.000 -> 0.000 |
| thread-far | 163 / 245 | 68 / 68 | 2.4x | 733 -> 230 | 733 -> 237 | 0.056 -> 0.056 |
| thread-menu | 125 / 126 | 19 / 20 | 6.4x | — -> — | 125 -> 19 | 0.000 -> 0.000 |
| scroll-up **REGRESSION** | 60 / 597 | 159 / 193 | 0.4x | — -> — | 655 -> 273 | 0.000 -> 0.000 |
| send | 48 / 53 | 40 / 45 | 1.2x | — -> — | 1156 -> 709 | 0.000 -> 0.000 |
| type-key | 18 / 18 | 18 / 18 | 1.0x | — -> — | 379 -> 32 | 0.000 -> 0.000 |

### X-vslow-web-rtt100

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| viewer-board | 2540 / 2540 | 968 / 968 | 2.6x | — -> — | 2716 -> 2047 | 0.144 -> 0.144 |
| viewer-fm | 2400 / 2490 | 498 / 556 | 4.8x | — -> — | 2679 -> 1444 | 0.144 -> 0.144 |
| palette-type | 2361 / 2361 | 1079 / 1079 | 2.2x | — -> — | 2364 -> 1085 | 0.007 -> 0.070 |
| esc-back | 2158 / 2336 | 634 / 642 | 3.4x | 2052 -> 515 | 2419 -> 1286 | 0.000 -> 0.000 |
| tool-fold | 1948 / 1964 | 219 / 286 | 8.9x | — -> — | 1972 -> 1537 | 0.000 -> 0.000 |
| thread-far | 1509 / 1998 | 428 / 432 | 3.5x | 3159 -> 1004 | 3159 -> 1062 | 0.056 -> 0.056 |
| palette-open | 1366 / 2041 | 213 / 241 | 6.4x | — -> — | 1382 -> 764 | 0.000 -> 0.000 |
| settings-open | 1294 / 1379 | 310 / 347 | 4.2x | — -> — | 1400 -> 576 | 0.000 -> 0.000 |
| project-home | 1188 / 1350 | 574 / 588 | 2.1x | — -> — | 1195 -> 577 | 0.003 -> 0.003 |
| thread-near | 741 / 820 | 305 / 595 | 2.4x | 1490 -> 800 | 1490 -> 800 | 0.046 -> 0.046 |
| thread-long | 650 / 658 | 386 / 397 | 1.7x | 1504 -> 1008 | 1504 -> 1008 | 0.046 -> 0.046 |
| model-picker | 440 / 441 | 174 / 228 | 2.5x | — -> — | 1253 -> 265 | 0.000 -> 0.000 |
| thread-menu | 282 / 299 | 37 / 50 | 7.6x | — -> — | 895 -> 37 | 0.000 -> 0.000 |
| send | 221 / 264 | 231 / 235 | 1.0x | — -> — | 1615 -> 313 | 0.000 -> 0.000 |
| type-key **REGRESSION** | 40 / 44 | 59 / 70 | 0.7x | — -> — | 50 -> 60 | 0.000 -> 0.000 |

### R-fast-phone

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| ph-viewer-board | 297 / 335 | 144 / 146 | 2.1x | — -> — | 539 -> 398 | 0.000 -> 0.005 |
| ph-palette | 292 / 342 | 134 / 156 | 2.2x | — -> — | 294 -> 201 | 0.000 -> 0.000 |
| ph-tool-fold | 283 / 399 | 135 / 138 | 2.1x | — -> — | 284 -> 211 | 0.000 -> 0.000 |
| ph-back | 156 / 156 | 158 / 175 | 1.0x | — -> — | 329 -> 329 | 0.004 -> 0.004 |
| ph-thread-open | 150 / 166 | 179 / 213 | 0.8x | 253 -> 208 | 394 -> 356 | 0.004 -> 0.004 |
| ph-send **REGRESSION** | 134 / 136 | 161 / 175 | 0.8x | — -> — | 475 -> 568 | 0.123 -> 0.123 |
| ph-model-picker | 131 / 161 | 129 / 134 | 1.0x | — -> — | 189 -> 156 | 0.000 -> 0.000 |
| ph-views-menu | 116 / 117 | 114 / 115 | 1.0x | — -> — | 116 -> 114 | 0.000 -> 0.000 |
| ph-type-key | 16 / 16 | 16 / 16 | 1.0x | — -> — | 315 -> 283 | 0.000 -> 0.000 |

### S-fast-web

| interaction | before p50 / p95 | after p50 / p95 | speed-up p50 | confirmed by hub before -> after | settle before -> after | shift before -> after |
|---|---:|---:|---:|---|---|---|
| project-home | 274 / 303 | 223 / 242 | 1.2x | — -> — | 275 -> 224 | 0.001 -> 0.001 |
| palette-type **REGRESSION** | 128 / 133 | 158 / 165 | 0.8x | — -> — | 133 -> 163 | 0.020 -> 0.020 |
| esc-back | 69 / 81 | 46 / 47 | 1.5x | 60 -> 43 | 103 -> 81 | 0.000 -> 0.000 |
| thread-far | 59 / 61 | 52 / 57 | 1.1x | 103 -> 93 | 103 -> 93 | 0.000 -> 0.000 |
| viewer-diff | 52 / 57 | 27 / 28 | 1.9x | — -> — | 218 -> 184 | 0.414 -> 0.414 |
| thread-short | 48 / 69 | 36 / 36 | 1.3x | 97 -> 67 | 116 -> 115 | 0.000 -> 0.000 |
| thread-project | 47 / 62 | 53 / 157 | 0.9x | 114 -> 96 | 119 -> 130 | 0.000 -> 0.000 |
| scroll-up | 36 / 37 | 37 / 37 | 1.0x | — -> — | 36 -> 37 | 0.000 -> 0.000 |
| send | 36 / 45 | 31 / 31 | 1.2x | — -> — | 164 -> 147 | 0.000 -> 0.000 |
| thread-long **REGRESSION** | 36 / 42 | 87 / 96 | 0.4x | 68 -> 156 | 116 -> 160 | 0.000 -> 0.000 |
| viewer-board | 34 / 36 | 32 / 34 | 1.1x | — -> — | 189 -> 178 | 0.414 -> 0.414 |
| thread-near **REGRESSION** | 33 / 44 | 64 / 78 | 0.5x | 67 -> 76 | 115 -> 116 | 0.000 -> 0.000 |
| palette-open | 32 / 32 | 37 / 42 | 0.9x | — -> — | 60 -> 76 | 0.000 -> 0.000 |
| settings-open | 28 / 30 | 27 / 29 | 1.0x | — -> — | 40 -> 38 | 0.000 -> 0.000 |
| viewer-fm | 26 / 44 | 28 / 30 | 0.9x | — -> — | 188 -> 183 | 0.414 -> 0.414 |
| model-picker | 18 / 18 | 18 / 29 | 1.0x | — -> — | 41 -> 44 | 0.000 -> 0.000 |
| account-picker **REGRESSION** | 17 / 18 | 36 / 44 | 0.5x | — -> — | 95 -> 116 | 0.000 -> 0.000 |
| thread-menu | 17 / 17 | 17 / 17 | 1.0x | — -> — | 17 -> 17 | 0.000 -> 0.000 |
| type-key | 16 / 17 | 16 / 16 | 1.0x | — -> — | 31 -> 31 | 0.000 -> 0.000 |
| tool-fold | — / — | — / — | unmeasured | | |

### Worst 10 after

| interaction | cell | p50 |
|---|---|---:|
| palette-type | X-vslow-web-rtt100 | 1079 |
| viewer-board | X-vslow-web-rtt100 | 968 |
| esc-back | X-vslow-web-rtt100 | 634 |
| project-home | X-vslow-web-rtt100 | 574 |
| viewer-fm | X-vslow-web-rtt100 | 498 |
| thread-far | X-vslow-web-rtt100 | 428 |
| palette-type | R-slow-web | 417 |
| palette-type | R-fast-web-rtt250 | 406 |
| thread-long | X-vslow-web-rtt100 | 386 |
| project-home | R-fast-web-rtt250 | 377 |

### Regressions (p50 > +20 % and > +10 ms)

- R-slow-web palette-type: 332 -> 417 ms
- R-slow-web type-key: 18 -> 30 ms
- X-fast-web scroll-up: 60 -> 159 ms
- X-vslow-web-rtt100 type-key: 40 -> 59 ms
- R-fast-phone ph-send: 134 -> 161 ms
- S-fast-web palette-type: 128 -> 158 ms
- S-fast-web thread-long: 36 -> 87 ms
- S-fast-web thread-near: 33 -> 64 ms
- S-fast-web account-picker: 17 -> 36 ms

## Layout shift and late content

Late items are visible text or elements added after the result's frame (per warm rep, summed over interactions):
"late in both" were late before too; "NEW" are late only because the result now shows earlier.

| cell | interactions | shifting (>0.01) before -> after | mean CLS / interaction before -> after | late items / rep, all interactions: before | after, late in both | after, NEW (shown earlier) |
|---|---:|---|---|---:|---:|---:|
| R-fast-web | 30 | 16 -> 17 | 0.057 -> 0.090 | 30.2 | 19.2 | 20.6 |
| R-fast-web-rtt30 | 20 | 9 -> 10 | 0.052 -> 0.083 | 26.5 | 12.5 | 18.5 |
| R-fast-web-rtt100 | 20 | 8 -> 9 | 0.045 -> 0.041 | 26.8 | 25.2 | 29.2 |
| R-fast-web-rtt250 | 20 | 9 -> 10 | 0.052 -> 0.052 | 14.2 | 7.8 | 28.5 |
| R-slow-web | 20 | 9 -> 10 | 0.052 -> 0.053 | 29.8 | 18.5 | 35.2 |
| R-vslow-web | 10 | 4 -> 4 | 0.029 -> 0.030 | 3.2 | 1.0 | 5.2 |
| R-fast-desk | 20 | 8 -> 9 | 0.046 -> 0.086 | 23.2 | 9.8 | 8.2 |
| R-fast-phone | 9 | 1 -> 1 | 0.014 -> 0.015 | 14.2 | 3.8 | 11.0 |
| S-fast-web | 20 | 4 -> 4 | 0.059 -> 0.063 | 6.0 | 4.8 | 1.5 |
| X-fast-web | 20 | 7 -> 8 | 0.033 -> 0.045 | 16.5 | 10.2 | 46.6 |
| X-vslow-web-rtt100 | 15 | 5 -> 6 | 0.025 -> 0.032 | 8.3 | 0.0 | 16.7 |
