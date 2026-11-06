# Browser recovery and allowance display

The old topbar button was the manual way to replace a failed private desktop
Browser controller after its socket/heartbeat or CDP guard dropped. That controller
disposed its guests and never tried another authenticated connection.

Main now retries a failed connection after 0.5, 1, 2, 4, 8, 16 and at most 30
seconds, with one attempt in flight. Resume retries immediately. Discovery still
requires the same selected loopback origin, authenticated home, exact replacement
version and normal protocol/source/grant checks. Renderer reloads re-present an
existing healthy guest. A replacement restores the idle active http(s) URL when
that owner's Browser tab is next presented, through the authenticated preview service from desktop main.
Outstanding calls and UNKNOWN outcomes are never replayed. Errors are inline in
the Browser tab; there is no topbar recovery control or reconnect IPC action.

Default Codex retains its five-minute collector. The account snapshot worker also
starts a serialized metadata sweep: no more than one attempt per named account
per five minutes, skipping login, launch and active children until idle. Codex
uses its app-server metadata endpoints; Claude uses the CLI's structured
`get_usage` control request with `skip_behaviors`, without a user prompt or model
turn. This Claude interface is experimental: unsupported versions leave the
measurement absent, or retain the previous same-identity measurement. Default
Claude uses the same CLI request after verifying its current subscription login.
No tokens or OAuth endpoints are accessed by the collector.

The presentation hides an absent/invalid measurement and keeps the latest valid
percentage during refresh, across a passed reset or age deadline. The displayed
sample timestamp stays unchanged. Account selection still receives the original
measurement time and internal freshness state, so retaining a label cannot make
an old allowance eligible as a fresh reading. A changed verified identity clears
its former measurement. The composer layout and shared-pool phone label are
unchanged.

The sidebar dependency review covers the five changed modules in
`sidebar-draft-inputs.json`: allowance projection and web output use only the
existing snapshot/provider/model/clock inputs; `Acv.note` removes a status label;
`View.chrome.tools` and `WB.browser.surface.k` remove static buttons. Main supplies the missing `composer.offline.back` catalog key and regenerated
lookup tables. No draft reader, sidebar key or bridge projection was added. The complete retained/full-output allowance and
sidebar hit tests qualify this source review before the full server build.

Validation: full `scripts/check.sh` typechecking and the repository's accepted
2.0.36 foreign-effect proof verdict; focused allowance, publication, named
account, phone composer, retained sidebar and sidebar-hit tests; 49 desktop
browser unit cases, 13 account refresh/registry cases and 31 provider-account IO
cases. Chromium renders cover 1440 and 390 px, old and new unknown/aged/browser
states, one-row composer height, and a readable shared-pool label. The images in
`docs/media/ui-removals/` are production-page fixtures with synthetic readings,
not screenshots of a person's live provider usage.

Isolated native Electron acceptance also passed: an actual guest opens the URL,
the private channel recovers after a hub restart, the active URL/thread/draft
return without a click, and a renderer reload keeps the same guest. No provider
turn or message is replayed. Headless guest compositing is outside this check;
resume is covered by the main-process retry tests, rather than a physical sleep.
