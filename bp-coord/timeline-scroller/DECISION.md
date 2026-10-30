# Web/Electron timeline model: pure Bend port

The user superseded the library survey on 2026-10-29: **port the TanStack virtual-core 3.19.9 model into Bend; add no runtime JS library or framework**. The DOM adapter reads measurements and applies plans; pure Bend owns layout, range selection, anchor compensation, following, restoration, and key-jump alignment. Retain hub `view.page` pagination. Phones are excluded.

## Upstream evidence and license

Source is **@tanstack/virtual-core 3.19.9, MIT**, npm-published 2026-10-29 10:55:26 UTC. [npm version record](https://registry.npmjs.org/@tanstack%2Fvirtual-core/3.19.9), [release](https://github.com/TanStack/virtual/releases/tag/%40tanstack%2Fvirtual-core%403.19.9), commit **51b06cd5e078a289fe1cbf25a8d2e647aa98bfff**. Downloaded tarball SHA-512 matches the registry; the package source-map content exactly matches that release's `packages/virtual-core/src/index.ts`. Retain upstream notice in `src/core/timeline-virtual.LICENSE`; this is a model port, not a claim to have imported the complete upstream implementation.

The [released core source](https://github.com/TanStack/virtual/blob/51b06cd5e078a289fe1cbf25a8d2e647aa98bfff/packages/virtual-core/src/index.ts) is the algorithm reference:

- `getMeasurements`: chronological, keyed, single-lane prefix layout, measured size cache with estimates, gaps and padding.
- `setOptions` (around lines 810–937): captures a visible **key plus offset before changing options** from previous measurements; resolves the same key in the new layout. A new `getItemKey` mapping must not retroactively reinterpret the old index layout. The port's explicit `previous.items` snapshot provides this boundary.
- `_willUpdate`: applies pending anchor/follow work after the new geometry is available. The DOM adapter must size the new track before writing the compensated `top`.
- `resizeItem` (around line 2045): detects whether the viewport was pinned before size changes and compensates measured growth.
- `calculateRange`/`getVirtualIndexes`: select a bounded render window. The port uses a pixel overscan band and explicitly retains its anchor.
- `scrollToEnd`, `isAtEnd`, `getDistanceFromEnd`: latest positioning and conditional following. Upstream released options are `anchorTo: 'end'`, `followOnAppend: true`, and `scrollEndThreshold`; these are real shipped APIs, not unreleased documentation.

Official [chat guide](https://tanstack.com/virtual/latest/docs/chat), [working chat playground](https://tanstack.com/virtual/latest/docs/framework/react/examples/chat), and [released example source](https://github.com/TanStack/virtual/blob/51b06cd5e078a289fe1cbf25a8d2e647aa98bfff/examples/react/chat/src/main.tsx) demonstrate prepend, append, and streaming behavior. The example is React; **the port does not adopt React**, `directDomUpdates`, or `containerRef`.

A short Chromium exercise of the unmodified upstream playground confirmed it loaded React adapter3.14.24/core3.19.9. A 30-message prepend changed visible index1→31 while preserving offset−33px exactly. Measured streaming stayed at bottom gap0px; scrolling away during streaming preserved top11424 and row offset−284px through completion. This is upstream evidence, not acceptance of the Bend port.

## Port contract

`src/web/timeline-plan.bend` exports `timeline_plan(input: J.Json) -> J.Json`, importing `src/core/timeline-virtual.bend`. No `app.bend` ownership change and no JS vendor dependency.

Input:

```text
{keys:[stable strings], sizes:{key:measured CSS pixels},
 previous:{items:[{key,index,start,size}],total,top,view},
 top:current DOM scrollTop, view:clientHeight,
 mode:'init'|'update'|'measure'|'scroll'|'restore'|'bottom'|'key',
 follow:Bool, bookmark:{anchor,offset,top,atEnd},
 key:string, align:'start'|'center'|'end'|'auto', offset:number}
```

Output:

```text
{items:ALL {key,index,start,size}, visible:render-only records,
 total,top,logicalTop,trackOffset,view,atEnd,anchor,offset}
```

The top-level `key/align/offset` fields apply to key-jump mode. Bookmark fields apply to restore. The signed offset is **top minus anchor row start**, including a negative leading-gap offset.

- Single lane, chronological order. Default estimated row120px, gap10px, start padding16px/end padding8px; overscan one viewport plus720px each side.
- For update/measure, capture the previous visible nonzero, non-`control:` row against **current DOM top**, find its stable key in new layout, and set top to new start plus the captured signed offset. Controls must not steal a message anchor at the top. Never clamp this compensation against old track geometry.
- Initial/bottom mode selects the latest position. Automatic following requires `follow` and previously being within4px of the end. Scroll mode never follows and preserves supplied DOM top.
- Restore uses the bookmark's key and signed offset, falling back to its saved top; a bookmark marked atEnd selects the end. Key mode aligns even an unmounted row using core geometry, with optional offset.
- Bend Base has **no native F64**. The port preserves fractional JSON number spellings through exact signed decimal digit arithmetic rather than rounding them to F32 or whole pixels. The boundary remains ordinary JSON numbers. This is an intentional numeric implementation difference from upstream's JavaScript double arithmetic.
- Iterative measurement retains the selected reading key while that row still intersects the viewport. It does not switch to a newly prepended row that occupies the previous leading gap; actual user motion releases the preference when the old anchor leaves the view.
- DOM scroll readback can round a fractional command. Bend converts physical scroll positions through `trackOffset`, normalizes the correction to a fractional CSS pixel, and returns `top`, `logicalTop`, and the track translation. The shim applies those values and forwards the next readback. A bounded fallback accepts the actual position after four readbacks; JavaScript performs no compensation arithmetic.
- Sizes are parsed once into a Map; layout does not scan the complete sizes object once per row. The previous complete layout is data, not mutable DOM state.

Three append-only laws cover the real helper policies: moving the keyed anchor carries its signed offset unchanged; the render filter includes a matching anchor regardless of overscan; an unpinned viewport takes the held-position branch regardless of follow request. These are symbolic proofs over the implemented helpers, not a claim that upstream or the DOM/browser has been formally proven. Fractional arithmetic and actual DOM behavior require the focused tests and integrated smoke.

`view.page` cursors, ownership/session fences, history projection, body continuation windows, thread restoration, folds/search, and composer selection remain application contracts. Follow is enabled only when the hub window is actually newest. A virtualizer's loaded-window end is not necessarily the conversation end.

Fetched tarballs, metadata, pinned source, browser check and screenshot remain in [external evidence](/home/h/Documents/i2cjak/bp-coord/timeline-scroller/evidence/). Existing comparison evidence is retained there; further surveying is stopped.

The user narrowed integration qualification to the focused checks plus one largest-thread smoke and the staged desktop with rollback. Before/after regression comparisons and longer Boat qualification are deferred. The integration owner handles that build/smoke; this planner task does not run another full suite or build.

## Accepted integration smoke

The generated web client passed the local largest-thread smoke (5,202 actual copied events). Three measured prepend sequences each preserved the reading anchor at **0 CSS px drift**; the run also scrolled both ways and preserved the exact composer node, draft text, and caret selection. Raw frame timings and limits are in `/home/h/Documents/i2cjak/bp-coord/timeline-scroller/smoke/SMOKE.json`. This is headless desktop-size acceptance, not physical-display, live-streaming, Boat, or native-phone qualification. Failed candidates were not installed and their evidence remains retained.

## Electron regression correction

The user's October 29 desktop recording exposed behavior the original static Chromium smoke missed. Source labels were independent fresh-only controls, so paging removed or reintroduced 32px between retained messages. Body-window stream identities (`@live:`) were also mistaken for historical entries, producing false page overlap. Applying scroll commands before measuring mounted rows fed temporary browser clamps into the readback translation. Bounded live-preview tails could shrink repeatedly while their reply grew. Zero-height output controls still contributed a 10px gap.

The correction groups each incoming source label with its historical message, treats both live key forms as transient, retains measured historical sizes across pages, measures mounted geometry before scroll writes, and gives the physical track the exact rounded extent required by its logical position. Bend marks live rows whose measured height stays reserved until completion; their tail content is aligned at the foot. Zero-height controls take no layout gap. Browser anchoring is disabled and scroll behavior remains `auto`.

`desktop/test/timeline-regression.cjs` runs the packaged Electron binary on a private X display and disposable copied history, records video plus every animation frame's row geometry, and streams through a deterministic provider. Final acceptance uses the exact separately qualified mailroom service, not the previous service binary. It checks scrolling both ways, idle reading hold during streaming, bottom-follow, draft/caret preservation, and the explicit return to latest. Raw recordings and measured limits stay private under `/home/h/Documents/i2cjak/bp-coord/timeline-scroller/desktop-regression/`. This is Electron acceptance on a private display; it does not claim adoption by an already-open user window, native phone acceptance, or every frame meeting 16.7ms.

## Markdown flashing correction

The 14:27 desktop recording shows idle Bub messages switching between Markdown and raw source. Body continuation previews used a plain-text `pre` before their fetched pages used Markdown. A four-reader active cache also evicted completed bodies when five visible messages needed pages, causing repeated fetches and height changes. A newly issued hub page could incorrectly invalidate bodies belonging to retained older rows.

Previews now use the Bend Markdown renderer before DOM publication and cache their structured nodes by entry, version and source within the current account/view scope. Completed active readers share the existing byte budget and a 128-entry bound; parked scopes still retain four readers. Only the authoritative page may invalidate its own body metadata. Rendered preview source retention has its own bounded 1MiB/128-entry cache. No full conversation tree is retained. Virtual row removal waits until the synchronous measurement transaction settles, preserving rows temporarily omitted by an intermediate plan.

Fourteen body-window checks and the Chromium DOM regression pass. Packaged Electron acceptance replays 16,995 real copied Bub events plus five labeled Markdown fixtures and a deterministic Markdown stream against service `0.13.2-dev.local.c3fadd57e.6ae38459`. Across 2,219 captured frames there are zero plain-text message fallback frames, zero overlapping row replacements, zero idle reading drift, no opposite movement during wheel paging, and preserved draft/caret state. Streaming bottom gap is 0px. Warm idle frame p95 is 16.8ms; streaming phases can reach 33.4ms and the run's maximum is 66.7ms. Raw evidence and review are private under `/home/h/Documents/i2cjak/bp-coord/timeline-scroller/markdown-regression/`. The existing global proof fails at the unchanged late import in `LAWS.bend:12298`; this JS-only correction does not change laws or Bend source.

## Opening-picture More footer correction

The October30 screenshot's three full-width More bars are partial activity-text controls from the server footer. Transition and startup pictures bypassed the continuation projection used for authoritative views, so raw controls briefly appeared below the final message. Project both saved-picture paths before DOM creation, remove generic footer controls even without matching current metadata, and attach explicit readers only in the authoritative view. Request cursors and account/view fences remain unchanged; older-history paging keeps its invisible top sentinel.

Seven focused continuation checks pass, including no controls after the final live-end message, three duplicate/transported controls, and no cross-scope reader restoration. The packaged Electron opening harness reproduces three bars on PR500 and current-main562 baselines, then observes zero across520candidateframes, including85saved-pictureframes, seven opens across three threads and a cached reload. Drafts survive switches/reload; no renderer errors occur. The controlled main562 baseline and candidate use the identical service SHA5a0a98ca. Private recordings and receipts are under `/home/h/Documents/i2cjak/bp-coord/timeline-scroller/more-flash-20261030/`. Global proof remains blocked by the unchanged base-main late import at `LAWS.bend:12383`; no Bend source or law changes.
