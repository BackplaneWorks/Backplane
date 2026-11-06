# backplane-hub M1: protocol for native clients (Bend stream, 2026-11-05)

Crate `native/host` (binary `backplane-hub`, 1.5 MB, built on Boat). Native clients only: `renderer: mobile`, `bp-policy` as the presenter, no Bun or JS in its process tree. Tests: `test/gpui/bend/boat-cargo-job.sh` (30 Rust tests), `test/gpui/hub/` (session driver, comparer, fold-scale), results `build/gpui/results/hub/`.

## Routes the clients use (read from the clients, not assumed)

| Route | Used by | In backplane-hub |
|---|---|---|
| `GET /ws?view=1&mobile=1&enc=cbor&token=` | iOS (`Hub.swift` viewSocket), Android (`Hub.kt`), GPUI (`hub.rs`) | yes: token, bearer, cookie, paired device (devices-v1.json); greeting, view.open/update/action, `view.control`, `view.changed`, `hb` every 15 s, 256 KB bound; every frame binary CBOR (`hub_frames_cbor`) |
| `GET /client/capabilities` | iOS, Android (`ViewCapability.kt`) | yes; advertises renderer `mobile` only (the JS hub also lists `web`) |
| `GET /hello` | Android, hub discovery | yes, byte-identical to the JS hub (188 bytes, headers and body) |
| `/pair`, `/auth/pair`, `/auth/enroll`, `/auth/devices`, `/auth/invitations`, `/auth/revoke` | iOS, Android | no: pairing writes devices-v1.json (effects/pairing.js) |
| `/oauth/google` | iOS, Android | no (accounts, M5/M6) |
| `/img`, `/attach` | iOS, Android | no (files, M6) |
| plots, `/view/*` | phones' plot child | no (M6); GPUI uses none of these |
Dropped with the web page: `renderer: web`, static files, `viewDelta` (only `src/web` offers it; grep of src/mobile, mobile/, native/gpui finds none).
Unimplemented methods answer `{"ok":false,"error":"unknown method"}` as the JS hub does for unknown ones; writes (turn.start, project.add...) arrive with M3-M5.

## Measured against the JS hub (this desktop, same synthetic logs, stand-in agents, one run each: smoke numbers, not a load test)

| Log | | startup to usable | view.open recent | view.update | view.open thread | hub RSS |
|---|---|---:|---:|---:|---:|---:|
| 250 threads (251 lines) | JS hub | 585 ms | 96 ms | 59 ms | 66 ms | 416 MB |
| | Rust + bp-policy-O2 | 106 ms | 86 ms | 69 ms | 72 ms | 12.5 MB (+46 MB policy) |
| 20 threads x 100 messages (2021 lines) | JS hub | 759 ms | 52 ms | 23 ms | 33 ms | 435 MB |
| | Rust + bp-policy-O2 | 105 ms | 7 ms | 4 ms | 5 ms | 9.3 MB (+43 MB policy) |

Parity of the replies (identity fields and the hub URL's port ignored): 250-thread log, 4 replies compared: the picture is identical except
1. `ago` on every thread: the policy's clock is not wall clock ("now" vs "102d") - slim is fixing (reads `p.now`, which the hub now sends).
2. `overlay` missing (slim is returning it).
3. the JS hub's startup events (it writes about 8 settings into the log at boot, so `hubs[0].changes` 259 vs 251) - hub bootstrap, M4.
4. `shown.version` (we now send "" as the JS hub shows) and a few empty scratch keys in `session.drafts`.
On a thread with 100 messages: `window` is a placeholder and the timeline has 20 entries against the JS hub's 32 from a store page (`earlier` 1 vs 68). That is windowed history (M2) and needs the pure parts of `thin-view.bend` split from its `thinstore` import (an `src/**` edit: coordinator).
Phones also validate `window` and `overlay`; GPUI reads neither. So the M1 gate holds for GPUI's flow on logs without long threads, and not yet for phones.

## What the real log taught (1.69M events, 1.55 GB, the store copy via a symlink, no copy made)

* The store as an offset index: open in 1.27 s (warm cache), hub RSS 31 MB (peak 35 MB). My first version kept every line as a String and was OOM-killed at 4 GiB; the index is 16 bytes a line.
* **The presenter cannot be fed the whole log.** bp-policy folding the first N events (6 GiB scope, `BACKPLANE_FOLD_LIMIT`):

| N events | time to listening | policy RSS | KB / event |
|---:|---:|---:|---:|
| 25,000 | 2.3 s | 188 MB | 7.7 |
| 50,000 | 2.7 s | 355 MB | 7.1 |
| 100,000 | 5.5 s | 696 MB | 7.1 |
| 200,000 | 14.5 s | 1.39 GB | 7.1 |
| 400,000 | 41.2 s | 4.12 GB | 10.3 |

Worse than linear in both time and memory past 200k. At 1.69M events the full fold was OOM-killed at 6 GiB by about 60 s. The JS hub avoids this by keeping a windowed state. Consequence for the plan: Rust must own the read model and the indexes (M2/M4) and hand the presenter small windows (a thread's page, the sidebar rows), not the log. This is the staging in PORT.md made urgent: the sidecar is viable only with windows.

## Gates reported earlier
* Native `State.apply` over 200k events: 2.5-2.9 s (13 us/event) vs hub-style JS 3.3-3.9 s, raw JS 11.6-12.4 s, digests equal. Rust has not been measured on this yet.
* Persistent policy child: 6.7 us median round trip over loopback, 20.5 us to fold one event; the spawn-per-call spike cost about 700 us.

## Laws (same commit as the code)
12 laws now state `rust`: `auth_foreign_origin_denied`, `auth_empty_remote_token_denied`, `auth_cookie_prefix_denied`, `auth_loopback_host_needs_token`, `auth_localhost_needs_token`, `auth_no_host_needs_token`, `auth_loopback_with_token`, `store_torn_tail_newline`, `store_torn_tail_resyncs`, `store_load_append`, `cbor_trees_exact`, `hub_frames_cbor`. The rest of `auth.bend` (OAuth envelope, owner pairing, pair proofs, cache key) and the hub-added identity assembly have no law mapped yet and stay `unported`/`bend`.

## Not done in M1
* No real phone or GPUI binary was run against it (none is built here); `test/gpui/hub/session.mjs` replays the GPUI client's flow (hub.rs).
* Pairing/enrollment routes, `/img`, plots; `view.page/search/message/cancel` (answer `UNSUPPORTED`); view.changed pacing is a flat 50 ms debounce, not Pace.step; slow-client bounds (M3).
