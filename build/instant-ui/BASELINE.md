# Baseline: mobile web actions

Measured 2026-11-03T01:51:07.918Z from `h@backplane` to `http://100.64.84.41:3787` across Tailscale. Installed source baseline: `273b268cd`. Chromium, 390×844 CSS pixels, touch, DPR 3, iPhone Safari UA. This is mobile emulation on a Linux CPU, **not Safari/iPhone hardware**.

First content is the first post-frame observation of the requested thread, bot header, or optimistic message. Settlement requires a ready reply for the destination and a subsequent paint; sends additionally require the committed row. A bot can paint a retained destination after its action response but before its open response.

| Action | Cached | First content ms | Settled ms | View RPCs issued | Received reply bytes | Client JS ms | Layout ms | Style ms |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| select t1793665018-1293799 | no | 308.4 | 311.2 | 2 | 121176 | 84.7 | 8.0 | 11.9 |
| select t1793584394-1163165 | no | 674.1 | 675.7 | 2 | 144444 | 83.1 | 18.9 | 15.8 |
| select t1793648028-1245355 | no | 755.5 | 757.1 | 2 | 131910 | 124.9 | 4.8 | 17.9 |
| select t1793669258-1305224 | no | 3643.5 | 3644.2 | 2 | 126273 | 78.0 | 2.3 | 34.8 |
| select t1793670038-1308809 | no | 1205.5 | 1206.1 | 2 | 124392 | 74.6 | 2.2 | 14.2 |
| select t1793665018-1293799 | yes | 28.1 | 777.4 | 1 | 122236 | 82.2 | 4.6 | 12.6 |
| select t1793584394-1163165 | yes | 35.6 | 504.8 | 1 | 145207 | 57.8 | 5.1 | 12.5 |
| select t1793648028-1245355 | yes | 26.2 | 383.0 | 1 | 132597 | 54.6 | 2.2 | 8.1 |
| select t1793669258-1305224 | yes | 21.0 | 409.5 | 1 | 126528 | 39.7 | 1.4 | 5.8 |
| select t1793670038-1308809 | yes | 26.8 | 430.9 | 1 | 124392 | 58.4 | 2.5 | 9.1 |
| bot b1790403469-15249 | no | 803.6 | 805.2 | 2 | 111603 | 50.8 | 2.9 | 11.9 |
| bot b1790403473-15254 | no | 389.3 | 390.9 | 2 | 117784 | 56.4 | 2.8 | 8.2 |
| bot b1790403469-15249 | yes | 165.7 | 368.8 | 2 | 112384 | 46.1 | 0.9 | 3.6 |
| bot b1790403473-15254 | yes | 563.3 | 1275.1 | 2 | 118070 | 81.8 | 1.2 | 5.0 |
| new-thread p1790213730-40 | no | 2988.1 | 2988.1 | 4 | 115452 | 65.6 | 0.7 | 6.2 |
| send (new message) | no | 16.2 | 1656.8 | 2 | 7311 | 86.2 | 1.3 | 6.9 |
| send (reply) | no | 20.6 | 1086.2 | 2 | 5404 | 74.5 | 1.4 | 8.6 |

## Request chains

Counts above include follow-up requests issued before settlement. The cold first-content path is one `view.open`; its extra `view.action:fab-look` is issued after content arrives. Cached bot switches prepare `view.action:bot` and then `view.open`. Send uses `view.action:send`, an ordinary send RPC, and a canonical `view.update`. The view RPC recorder counts only view requests; ordinary send RPCs are not included in its count.

- select `t1793665018-1293799` cached=False: view.open (221.5 ms, 121176 B) → view.action:fab-look (reply after measurement boundary).
- select `t1793584394-1163165` cached=False: view.open (583.3 ms, 144444 B) → view.action:fab-look (reply after measurement boundary).
- select `t1793648028-1245355` cached=False: view.open (690.9 ms, 131910 B) → view.action:fab-look (reply after measurement boundary).
- select `t1793669258-1305224` cached=False: view.open (3600.7 ms, 126273 B) → view.action:fab-look (reply after measurement boundary).
- select `t1793670038-1308809` cached=False: view.open (1162.2 ms, 124392 B) → view.action:fab-look (reply after measurement boundary).
- select `t1793665018-1293799` cached=True: view.open (740.0 ms, 122236 B).
- select `t1793584394-1163165` cached=True: view.open (465.0 ms, 145207 B).
- select `t1793648028-1245355` cached=True: view.open (349.2 ms, 132597 B).
- select `t1793669258-1305224` cached=True: view.open (380.4 ms, 126528 B).
- select `t1793670038-1308809` cached=True: view.open (384.7 ms, 124392 B).
- bot `b1790403469-15249` cached=False: view.action:bot (211.0 ms, 2143 B) → view.open (545.5 ms, 109460 B).
- bot `b1790403473-15254` cached=False: view.action:bot (138.7 ms, 2379 B) → view.open (197.6 ms, 115405 B).
- bot `b1790403469-15249` cached=True: view.action:bot (146.4 ms, 2639 B) → view.open (197.5 ms, 109745 B).
- bot `b1790403473-15254` cached=True: view.action:bot (544.2 ms, 2639 B) → view.open (680.8 ms, 115431 B).
- new-thread `p1790213730-40` cached=False: view.action:new-thread (1733.4 ms, 1347 B) → view.update (578.2 ms, 3083 B) → view.open (596.9 ms, 111022 B) → view.action:fab-look (reply after measurement boundary).
- send `TEST` cached=False: view.action:send (511.7 ms, 2328 B) → view.update (634.9 ms, 4983 B).
- send `TEST` cached=False: view.action:send (629.7 ms, 1412 B) → view.update (408.5 ms, 3992 B).

## Network, server and rendering

Cold boot to usable sidebar: 1330.0 ms. Five independent `/hello` request durations: 16.5, 17.6, 13.9, 20.6, 15.1 ms. These include service queueing and cannot be called pure network RTT. The hub does not expose per-request Server-Timing, so precise network/server separation is unavailable on the installed baseline.

The first investigative run under heavier concurrent load measured cold opens at 3483–6779 ms, cached threads at 17–28 ms and `/hello` at 70–2144 ms. Its bot/send settlement detector was found to admit late prior-action paints; those settlement numbers are excluded. This repeat is fenced by actual destination and committed message. Both runs used the same installed source. Variability is material.

The table uses CDP cumulative ScriptDuration, LayoutDuration and RecalcStyleDuration deltas. The raw JSON also has per-function render/patch timing, decode time, layout/style counts, request/reply timestamps, and frame traces. Those client measurements overlap wall time and must not be added to request latency as independent spans.

Hub `/debug/perf.json` spans are process-wide aggregate service work since startup, including other active clients. They support attribution of busy service work but are not per-action measurements:

| Server span | Samples | Mean ms | p99 upper bound ms | Max ms |
|---|---:|---:|---:|---:|
| hub.rpc | 3069 | 401.7 | 2097.2 | 3288.5 |
| thin.rpc.1 | 2566 | 414.9 | 2097.2 | 2778.6 |
| thin.rpc.3 | 238 | 626.7 | 3287.7 | 3287.7 |
| hub.tick | 28 | 1766.1 | 3625.6 | 3625.6 |
| hub.tick.decide | 28 | 1569.5 | 3404.5 | 3404.5 |

## Coverage and cleanup

Five existing threads include long/running work and a short completed pairing thread. Two existing bots were viewed. Real sends/replies were confined to a newly created TEST conversation `t1793670683-1311270`. Pairing used the new single-use invitation enrollment endpoint. The independently enrolled TEST device was revoked: **True**. No real device was revoked. Browser errors: [].

Limitations: emulated DOM/frame observation approximates first paint; no physical iPhone trace was available. Browser input uses the same production delegated click/input handlers through DOM events, excluding Playwright selector/scroll delays. This run did not reload to test disk restoration, throttle the CPU to an iPhone, or include a deep paging acceptance suite. Raw reports remain in `build/instant-ui/`.

## WebKit comparison

WebKit 27.2/WPE on Linux, same 390×844 touch/iPhone UA and tailnet route, still installed273b268cd. This is a real WebKit engine but not Mobile Safari on an iPhone. Per-action CDP JS/layout counters are unavailable in WebKit; no zero-cost claim is made. Private dependency files supplied missing libraries without administrator changes.

| Action | Cached | First content ms | Settled ms |
|---|---|---:|---:|
| select `t1793584394-1163165` | False | 1143 | 1165 |
| select `t1793648028-1245355` | False | 769 | 784 |
| select `t1793669258-1305224` | False | 936 | 953 |
| select `t1793670038-1308809` | False | 1188 | 1199 |
| select `t1792701529-415584` | False | 1458 | 1458 |
| select `t1793584394-1163165` | True | 73 | 875 |
| select `t1793648028-1245355` | True | 56 | 792 |
| select `t1793669258-1305224` | True | 49 | 1037 |
| select `t1793670038-1308809` | True | 52 | 2015 |
| select `t1792701529-415584` | True | 71 | 3706 |
| bot `b1790403469-15249` | False | 2054 | 2054 |
| bot `b1790403473-15254` | False | 1397 | 1397 |
| bot `b1790403469-15249` | True | 541 | 1029 |
| bot `b1790403473-15254` | True | 405 | 772 |
| new-thread `p1790213730-40` | False | 1576 | 1576 |
| send `TEST` | False | 21 | 1730 |
| send `TEST` | False | 20 | 1428 |

TEST revoked: True. Browser diagnostics: ['ResizeObserver loop completed with undelivered notifications.', 'ResizeObserver loop completed with undelivered notifications.']. The ResizeObserver warnings came from the baseline client. Initial WebKit installation garbage-collected the shared Chromium cache; exact Chromium1243/headless1243 were restored, and later installs disable browser GC.
