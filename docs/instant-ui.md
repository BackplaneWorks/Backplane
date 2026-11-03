The web and desktop client retain eight recent conversation pictures in memory and a bounded 2 MiB IndexedDB store. Disk records are partitioned by hub origin, pairing cache identity, language, history epoch and client bundle. They contain a display tree with field values removed. They never restore a session, draft, request counter, history cursor or readiness. The live view revalidates on the existing foreground connection.

Bot navigation opens its target directly, removing the old action-then-open round trip. A cached bot picture supplies only its visual backing-conversation id and route; current local drafts remain independent. Outgoing viewport and unread state are captured before changing selection. A send during cached opening appears immediately and waits for destination authority. If the bot's backing conversation changed, the unissued send is refused and its draft restored.

A separate serial preview connection warms a few sidebar destinations. It sends only bounded `view.open` reads with empty conversation drafts, never actions, history pages, unread acknowledgments or returned commands. At most five targets queue and sixteen are attempted per page. Hover queues a preview; touchstart warms disk records. Foreground switching does not resubscribe its socket.

Regression commands:

```sh
node --test desktop/test/recent-views.test.cjs desktop/test/view-prefetch.test.cjs desktop/test/thin-host.test.cjs desktop/test/thin-send-preview.test.cjs
INSTANT_HUB=/absolute/path/to/backplane-serve bun test/tools/view_prefetch_isolated.mjs
PLAYWRIGHT_MODULE=/path/to/playwright-core CHROMIUM_PATH=/path/to/chrome bun test/tools/recent_views_browser.mjs
```

`test/tools/instant_ui_remote.mjs` runs a mobile browser against a hub child that it starts itself on `127.0.0.1`, using a newly generated throwaway home, synthetic threads/bots and stand-in agents. Run the entire fixture on the remote machine; do not point it at an installed service. The binary and web assets must be local files on that machine:

```sh
INSTANT_HUB=/absolute/path/to/backplane-serve \
INSTANT_WEB=/absolute/path/to/emitted-web \
PLAYWRIGHT_MODULE=/path/to/playwright-core \
CHROMIUM_PATH=/path/to/chrome INSTANT_GATE=1 \
bun test/tools/instant_ui_remote.mjs </dev/null
node --test desktop/test/instant-ui-isolation.test.cjs
```

There is no target-address or home-directory option. `INSTANT_ORIGIN`, supplied credentials, command-line arguments and nonempty stdin are rejected before startup or any network access. The fixture verifies its child PID/port lock and fresh private bearer before issuing a private invitation. Its child environment does not inherit live hub settings, peers, proxies or provider credentials. Browser HTTP and WebSocket routes reject any other origin; browser-memory blob resources must also belong to the fixture origin. Playwright with `routeWebSocket` support is required. Both sends go to an existing synthetic thread and must produce stand-in ACKs in the private event log. The private paired device is revoked, the child process group is stopped, and its private directory is retained for inspection. No live-hub cleanup runs.

`INSTANT_GATE=1` fails cached switches or send previews over 100 ms. Reports retain input-to-content, authoritative settlement, RPCs, bytes and JS/render/layout measurements, along with the fixture home/PID/port and private event-log evidence. Set `INSTANT_ENGINE=webkit` and `WEBKIT_PATH` to use Linux WebKit. A loopback fixture does not reproduce tailnet RTT or physical iPhone hardware; use it for isolated interaction regressions, and do not label these results as live-tailnet measurements.

Earlier versions of this harness wrongly used the live hub and created six real conversations. Those threads must remain untouched for auto-settle. The old invitation pipeline and installed-build retest subscription were retired. The historical reports retain their observations with an explicit isolation correction; a TEST device or conversation label never makes a live write acceptable.

WebKit’s exact `ResizeObserver loop completed with undelivered notifications.` diagnostic is retained in `warnings`; other page errors and every blocked external request fail the run. The cached/send budget is unchanged.
