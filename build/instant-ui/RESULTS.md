# Instant UI results

Client candidate: merged-main base `92ddc7e46` plus the instant-actions changes, normally emitted with pinned Bend 2.0.36. Both final runs used the **installed 273b268cd backend**. The candidate module alone was substituted at its installed URL; the real hub HTML, origin, cookies, HTTP and WebSocket paths were retained. Raw artifact/source SHA256 receipts are in `web-manifest.json` and the Boat manifest. Nothing was installed by the test.

Tests ran on `h@backplane` over Tailscale to `100.64.84.41:3787`, at 390×844 CSS pixels, DPR3, touch and an iPhone Safari UA. Chromium is emulation; WebKit is the real Linux WPE engine, not iPhone hardware or Mobile Safari. These are observed ranges from five thread switches, two bot switches and one send plus one reply, not p95/p99 claims.

| Engine / action | Before first content ms | After first content ms | After settled ms | Result |
|---|---:|---:|---:|---|
| Chromium: Cached threads | 21–36 | 22–44 | 3337–4137 | PASS ≤100 ms |
| Chromium: Cached bots | 166–563 | 24–27 | 2672–3881 | PASS ≤100 ms |
| Chromium: Send / reply | 16–21 | 17–26 | 2357–2415 | PASS ≤100 ms |
| Chromium: Uncached threads | 308–3644 | 2823–5804 | 2825–5805 | Cold ≤300 ms not met on old hub |
| Chromium: Uncached bots | 389–804 | 2252–5469 | 2253–5470 | Cold ≤300 ms not met on old hub |
| WebKit 27.2: Cached threads | 49–73 | 30–43 | 292–719 | PASS ≤100 ms |
| WebKit 27.2: Cached bots | 405–541 | 26–37 | 257–392 | PASS ≤100 ms |
| WebKit 27.2: Send / reply | 20–21 | 17–20 | 1077–1252 | PASS ≤100 ms |
| WebKit 27.2: Uncached threads | 769–1458 | 456–898 | 462–934 | Cold ≤300 ms not met on old hub |
| WebKit 27.2: Uncached bots | 1397–2054 | 338–397 | 338–397 | Cold ≤300 ms not met on old hub |

Cached/optimistic first-content gate passes in both engines. Cached bot navigation now issues **one `view.open`**, removing the previous sequential `view.action:bot` then `view.open`. Existing text-send optimism remains immediate and now also works during a cached view opening; an unresolved/changed backing conversation refuses an unissued send and restores the captured draft.

**The entire request is not yet met:** completely uncached first content still exceeds 300 ms on the old hub. This client patch does not claim next-frame delivery of unknown server data or a fast canonical settlement. Server performance PR776 has merged but was not installed for these comparisons. Re-measure the installed merged build before claiming the cold target.

## Per-action final measurements

Issued RPC counts include auxiliary `desk.look`, `fab.look`, message-body reads and post-paint refreshes. The critical first-content path for a cold switch remains one `view.open`; cached first content has zero required round trips. Sends prepare `view.action:send`, issue ordinary `turn.start`, then read a canonical `view.update`. Requests without a reply before the boundary are counted but their late response bytes are not invented.

| Engine / action | Cached | First ms | Settled ms | RPCs issued | Reply bytes before boundary | JS ms | Layout ms | Style ms | Out / in wire B |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Chromium select `t1793584394-1163165` | False | 2823.3 | 2825.1 | 4 | 138612 | 77.0 | 16.6 | 27.4 | 2086 / 140573 |
| Chromium select `t1793648028-1245355` | False | 3733.7 | 3735.0 | 5 | 132653 | 106.7 | 3.7 | 27.0 | 2137 / 267564 |
| Chromium select `t1793670038-1308809` | False | 3675.7 | 3677.0 | 4 | 130562 | 76.0 | 2.9 | 12.4 | 2077 / 133800 |
| Chromium select `t1793604413-1188217` | False | 5804.1 | 5804.8 | 5 | 138414 | 97.9 | 2.1 | 37.9 | 2138 / 271861 |
| Chromium select `t1793587884-1171553` | False | 3071.2 | 3072.0 | 3 | 138435 | 95.9 | 3.0 | 13.5 | 1490 / 141157 |
| Chromium select `t1793584394-1163165` | True | 28.5 | 4137.4 | 4 | 143451 | 87.0 | 2.4 | 23.0 | 2270 / 282696 |
| Chromium select `t1793648028-1245355` | True | 22.4 | 3858.2 | 4 | 134133 | 67.4 | 4.2 | 9.9 | 2268 / 136132 |
| Chromium select `t1793670038-1308809` | True | 44.4 | 3336.8 | 5 | 131441 | 110.8 | 2.3 | 20.7 | 2313 / 267852 |
| Chromium select `t1793604413-1188217` | True | 32.6 | 3790.0 | 3 | 139037 | 70.2 | 3.2 | 9.2 | 1436 / 141758 |
| Chromium select `t1793587884-1171553` | True | 24.7 | 4033.8 | 2 | 138805 | 63.5 | 2.0 | 5.3 | 839 / 140283 |
| Chromium bot `b1790403469-15249` | False | 2252.5 | 2253.4 | 3 | 115364 | 71.4 | 2.7 | 9.0 | 1152 / 115805 |
| Chromium bot `b1790403473-15254` | False | 5468.8 | 5469.9 | 3 | 121767 | 82.0 | 2.5 | 7.9 | 1153 / 123421 |
| Chromium bot `b1790403469-15249` | True | 27.1 | 2672.1 | 3 | 115599 | 71.7 | 1.5 | 4.2 | 1404 / 117904 |
| Chromium bot `b1790403473-15254` | True | 23.8 | 3880.6 | 3 | 121764 | 67.5 | 1.2 | 2.7 | 1405 / 123276 |
| Chromium new-thread `p1790213730-40` | False | 8196.6 | 8196.6 | 6 | 136188 | 146.3 | 1.4 | 8.2 | 3291 / 138518 |
| Chromium send `TEST` | False | 26.2 | 2415.0 | 4 | 4532 | 164.4 | 1.0 | 6.4 | 1974 / 5620 |
| Chromium send `TEST` | False | 17.1 | 2357.4 | 4 | 5463 | 99.7 | 1.0 | 5.0 | 2032 / 5904 |
| WebKit 27.2 select `t1793584394-1163165` | False | 752.0 | 804.0 | 4 | 139961 | unavailable | unavailable | unavailable | 2087 / 140402 |
| WebKit 27.2 select `t1793648028-1245355` | False | 898.0 | 934.0 | 4 | 133923 | unavailable | unavailable | unavailable | 2086 / 135471 |
| WebKit 27.2 select `t1793670038-1308809` | False | 456.0 | 462.0 | 5 | 130519 | unavailable | unavailable | unavailable | 2128 / 263709 |
| WebKit 27.2 select `t1793672679-1318451` | False | 559.0 | 584.0 | 3 | 115463 | unavailable | unavailable | unavailable | 1490 / 118709 |
| WebKit 27.2 select `t1793635403-1205889` | False | 555.0 | 557.0 | 5 | 140264 | unavailable | unavailable | unavailable | 2139 / 255957 |
| WebKit 27.2 select `t1793584394-1163165` | True | 35.0 | 704.0 | 2 | 143552 | unavailable | unavailable | unavailable | 838 / 145171 |
| WebKit 27.2 select `t1793648028-1245355` | True | 42.0 | 719.0 | 3 | 133718 | unavailable | unavailable | unavailable | 1437 / 134307 |
| WebKit 27.2 select `t1793670038-1308809` | True | 36.0 | 396.0 | 3 | 131019 | unavailable | unavailable | unavailable | 1425 / 132577 |
| WebKit 27.2 select `t1793672679-1318451` | True | 30.0 | 292.0 | 2 | 115708 | unavailable | unavailable | unavailable | 838 / 117477 |
| WebKit 27.2 select `t1793635403-1205889` | True | 43.0 | 663.0 | 2 | 140256 | unavailable | unavailable | unavailable | 838 / 140550 |
| WebKit 27.2 bot `b1790403469-15249` | False | 397.0 | 397.0 | 3 | 115054 | unavailable | unavailable | unavailable | 1152 / 115343 |
| WebKit 27.2 bot `b1790403473-15254` | False | 338.0 | 338.0 | 3 | 121457 | unavailable | unavailable | unavailable | 1153 / 122827 |
| WebKit 27.2 bot `b1790403469-15249` | True | 26.0 | 257.0 | 3 | 115289 | unavailable | unavailable | unavailable | 1404 / 116336 |
| WebKit 27.2 bot `b1790403473-15254` | True | 37.0 | 392.0 | 3 | 121457 | unavailable | unavailable | unavailable | 1405 / 122685 |
| WebKit 27.2 new-thread `p1790213730-40` | False | 926.0 | 926.0 | 6 | 120524 | unavailable | unavailable | unavailable | 3291 / 122144 |
| WebKit 27.2 send `TEST` | False | 17.0 | 1077.0 | 5 | 7377 | unavailable | unavailable | unavailable | 2840 / 8318 |
| WebKit 27.2 send `TEST` | False | 20.0 | 1252.0 | 4 | 5448 | unavailable | unavailable | unavailable | 2017 / 6070 |

## Network and service attribution

The raw reports retain each RPC send/reply wall time, payload bytes, frame counts and client decode/render timings. Installed273 does not expose per-request server timing. Network transport, hub queueing and server execution therefore cannot be separated exactly for an individual action; request wall time must not be mislabeled pure network RTT or server CPU time. Process-wide server spans are in BASELINE.md and each raw JSON, with this limitation. The service and other agents were active throughout. Baseline and final cohorts share the core long/running threads and both bots; sidebar changes replaced some completed thread targets. No timing ratio is claimed for cold/server work across those changing load conditions.

Chromium opened two foreground connections during startup (a retry) and one separate preview connection. WebKit opened one foreground and one preview connection. Switching did not create a new foreground socket for each thread. The preview connection is bounded and serial, sends only `view.open`, and ignores invalidation notices instead of following history.

## State and regression validation

- Final remote gate: cached first content and optimistic sends ≤100 ms in Chromium and WebKit, no relaxed budget.
- Production ThinHost/ThinViews tests cover delayed ACK/echo ordering, exactly-one message-id reconciliation, identical text sends, refusal/error draft restoration, old-view responses and pending-state retirement.
- New source-sensitive bot→thread and thread→bot tests capture the outgoing viewport/unread/cursor exactly once before selection changes. Source and destination drafts are preserved.
- A send during cached opening publishes its bubble before any response; a changed bot backing conversation refuses it and restores its draft.
- A private hub using the production ViewPrefetch passed7 checks: only `view.open`, no local drafts sent, exact canonical event-log equality, foreground actor unchanged and window ≤32 rows.
- Real IndexedDB browser tests passed10 checks: bounded eviction, reload restore (1 ms), owner isolation, draft-field stripping and malformed-record rejection.
- The focused-cache source-shape test already disagrees with main273/92: it expects a live DOM class where View.live.n.loc delegates to View.pending.node. The instant patch changes neither Bend renderer. The running-turn owner has this finding; it is not counted as a new passing regression.

## Cleanup and limits

- Chromium: TEST device revoked = True; budget failures = []; browser diagnostics = [].
- WebKit 27.2: TEST device revoked = True; budget failures = []; browser diagnostics = ['ResizeObserver loop completed with undelivered notifications.', 'ResizeObserver loop completed with undelivered notifications.'].

No real device was revoked. Real messages were confined to dedicated TEST conversations. The live device registry was checked and contained no active instant-UI TEST session after prior runs. WebKit ResizeObserver warnings also occurred in its baseline. Full proof/check receipts are reported separately; a refused memory/lock admission is not a proof pass. No physical iPhone trace or end-to-end persistent-cache UI reload timing is claimed; the measured IndexedDB reload test is isolated.

## Proof and final source checks

On integrated92ddc7e, the explicit `bend PROOF.bend` run completed with the repository-accepted foreign-code notice (2558 effect-dependent definitions), no other failure lines. This does not prove those foreign effects or the JavaScript state machine. Targeted final Node suite:256 pass; the send lifecycle test also reports13 internal cases. The `scripts/check.sh check` type-check phase is recorded separately.
