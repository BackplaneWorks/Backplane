The web and desktop client retain eight recent conversation pictures in memory and a bounded 2 MiB IndexedDB store. Disk records are partitioned by hub origin, pairing cache identity, language, history epoch and client bundle. They contain a display tree with field values removed. They never restore a session, draft, request counter, history cursor or readiness. The live view revalidates on the existing foreground connection.

Bot navigation opens its target directly, removing the old action-then-open round trip. A cached bot picture supplies only its visual backing-conversation id and route; current local drafts remain independent. Outgoing viewport and unread state are captured before changing selection. A send during cached opening appears immediately and waits for destination authority. If the bot's backing conversation changed, the unissued send is refused and its draft restored.

A separate serial preview connection warms a few sidebar destinations. It sends only bounded `view.open` reads with empty conversation drafts, never actions, history pages, unread acknowledgments or returned commands. At most five targets queue and sixteen are attempted per page. Hover queues a preview; touchstart warms disk records. Foreground switching does not resubscribe its socket.

Regression commands:

```sh
node --test desktop/test/recent-views.test.cjs desktop/test/view-prefetch.test.cjs desktop/test/thin-host.test.cjs desktop/test/thin-send-preview.test.cjs
INSTANT_HUB=/absolute/path/to/backplane-serve bun test/tools/view_prefetch_isolated.mjs
PLAYWRIGHT_MODULE=/path/to/playwright-core CHROMIUM_PATH=/path/to/chrome bun test/tools/recent_views_browser.mjs
```

`test/tools/instant_ui_remote.mjs` measures a real hub across the tailnet at 390 px with an iPhone UA. It reads a one-time invitation code as JSON on stdin, enrolls a separately named TEST device, and revokes that device in `finally`. Real sends are confined to a new TEST conversation. `INSTANT_GATE=1` fails cached switches or send previews over 100 ms. Reports distinguish input-to-content, authoritative settlement, all issued RPCs, payload bytes, JS/render/layout measurements and engine limitations. Chromium and WebKit both run; a Linux WebKit result is not a physical iPhone measurement.

For a compiled client comparison, `INSTANT_WEB` substitutes only the emitted JavaScript module at its installed URL. The real hub document and network paths remain intact: replacing the entire HTML response makes Chromium classify the tailnet fetches as a forbidden local-network request. Do not disable browser network protections to hide that harness error.
