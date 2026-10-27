# Native performance measurement

The frozen 34de app and ZIP remain the baseline. `build/ios-qualification/performance/baseline-34de.json` records work 699159: XCTest trigger and screenshot upper bounds, separately sampled fresh same-owner readiness, existing slow-call counts and cache size. The trigger is not itself proof of useful painted content; frames were reviewed. Screenshots containing an existing Bub draft remain private and were not exported. No typing or sends occurred.

The follow-up candidate adds diagnostics only. `BACKPLANE_PERFORMANCE=1` enables numeric `BPPerf` output in a Debug simulator build; Release and physical builds leave the default trace disabled. It does not bypass discovery, freshness, frame validation, owner checks or loading deadlines. `BACKPLANE_QUALIFICATION=1` is an independent status overlay and must be recorded separately. No pairing override is needed.

Each line contains stage, operation, monotonic start/end nanoseconds, byte count and numeric outcome. It contains no URLs, owners, paths, drafts, error strings or payloads. Records retain the latest 256 in a thread-safe snapshot. Disabled byte-count expressions are not evaluated.

| Stage | Meaning |
| --- | --- |
| 1 queueWait | Enqueue to engine worker entry; excludes context/invoke/decode |
| 2 context | Initial JavaScriptCore context, bridge evaluation and header installation |
| 3 invoke | Bridge method and string result; excludes native output decoding |
| 4 decode | Native Out decoding; byte count is JSON output bytes |
| 5 cache | Presentation-cache load and native Screen decode on startup |
| 6 discovery | Awaited capability HTTP discovery, including DNS/network |
| 7 modelReady | Attempt start to first accepted fresh same-owner model |
| 8 display | Attempt start to a matching visible timeline frame opportunity; outcome 3 means cached/unready, 0 means fresh at callback |

Operations: startup 1, loadPresentation 2, connection 3, receive 4, flush 5, navigation 6, viewerRestore 7, local 8, reading 9, pulse 10, screen 11, other 12, foreground 13. Outcomes: success 0, skipped 1, failed 2, stale 3.

Display instrumentation requires a configured entry intersecting the visible scroll viewport, layout for the current configuration and two display-link callbacks. Each configuration cancels the previous scheduled callback; freshness also checks the displayed revision against the currently accepted screen. A missing callback under continuous layout churn is a missing measurement, not zero latency. This marks a compositor scheduling opportunity, not physical pixel onset. Screenshots/video independently verify content; report frame/query/capture resolution and overhead. Empty timelines/catalogs have no timeline display marker.

At source `15b8fc5cccb1dc471347f187ab0b3c07363e1dac`, `TimelineScroll.swift:155-175` invalidates an outstanding display link on every configuration while the diagnostic callback is enabled and requests another layout. Lines 196-229 require a current layout, a visible entry and two uninterrupted display-link ticks. Repeated configuration can therefore delay or prevent the observation; the existing receipts do not establish whether it did so or when pixels appeared.

`AppModel.swift:97-102` records stage 7 only once, at the first matching ready model. It is not an interval of continuous freshness: a new thin presenting update can clear the ready marker (`AppModel.swift:1749-1757`), and an accepted screen increments the diagnostic revision (`AppModel.swift:1873-1874`). Stage 8's fresh outcome requires readiness and matching thread and revision at the callback (`AppModel.swift:108-119`; `Views.swift:1108-1115`). Thus the stage 7-to-8 gap includes any later unready periods, observation cancellation and main-thread/layout scheduling; the current numeric records cannot apportion those causes or prove a paint delay. Keep the owner, epoch, navigation and revision fences when gathering further evidence.

Normal launch starts at AppModel initialization, not process spawn. External `simctl`/XCTest timestamps separately bound process launch and automation overhead. Foreground attempts start at native foreground handling; navigation attempts start at the native action with an explicit destination. Direct bot actions without a known destination produce no invented readiness interval. Cached display and fresh readiness remain separate, and continuous freshness is not inferred from one accepted model.

Collect a small repeated cold-process, foreground and board/Bub-switch sample under stable hub provenance. First inspect stage sums and gaps; then change the measured bottleneck and repeat the same relevant sample. Do not rerun unrelated viewer acceptance or adopt unfinished shared loading changes. Never read or overwrite existing drafts for a benchmark.
