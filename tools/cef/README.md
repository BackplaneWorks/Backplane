# backplane-cef

Linux X11 CEF (Chromium Embedded Framework) content windows embedded in a
controller's parent window. Agents drive these same pages over CDP. The host
owns all tabs, including popups; controllers must never use CDP
`Target.createTarget` (it makes a separate Chrome window).

```sh
scripts/build-cef.sh
scripts/build-cef.sh --package build/backplane-cef-linux-x64.tar.zst
build/cef/Release/backplane-cef --parent 12345 --port 9333 --cache /tmp/my-cef-profile
```

The build uses the prebuilt CEF distribution cached at
`$XDG_CACHE_HOME/backplane/cef/154.0.34` (default `~/.cache`), downloads only
when missing, builds with CMake/Ninja (`BACKPLANE_JOBS`, default 2), and strips
the executable and libcef.so. Chromium is never built from source. Inside an already admitted systemd scope, build directly with
`BACKPLANE_JOBS=2 nice -n 10 scripts/build-cef.sh`; do not nest bp-admit
inside that scope. Otherwise use the coordinator’s admission control.

`--package OUT.tar.zst` packs a runtime rooted directly at `backplane-cef`,
with stripped libcef.so, all root paks, icudtl.dat, v8_context_snapshot.bin,
locales/en-US.pak, libvk_swiftshader.so, vk_swiftshader_icd.json,
libvulkan.so.1 and the CEF license. A sibling `OUT.tar.zst.sha256` contains
the SHA-256. The host uses `no_sandbox`, so chrome-sandbox is omitted.
The archive needs `tar` and `zstd`; compression uses two threads.

Flags:

- `--parent XID`: parent X window; without it, make a host test window.
- `--size WxH`: initial child size (default 1280x800).
- `--url URL`: optionally create the startup browser with key `main`.
  Without this flag no browser is created. Use `show main X Y W H` to display it.
- `--at XxY`: initial position of the optional startup child; protocol `show` sets placement.
- `--cache DIR`: CEF root profile, **never the Chrome profile**.
- `--port N`: loopback DevTools endpoint (`/json/version`, `/json/list`).
- The X display is `$DISPLAY`; Chromium switches are passed through.
- `BACKPLANE_CEF_NO_GPU=1` adds `--disable-gpu`.

## stdin/stdout protocol

stdin is one command per line, separated by spaces. A final URL may include
spaces. Controller keys match `[A-Za-z0-9_-]`, length 1..64; duplicate keys
and missing/extra arguments answer `error`. Popup keys additionally use `~`.

Every stdout line is JSON with `event` and wall-clock milliseconds `t`.
Command answers also carry `cmd` and `key` (empty when inapplicable).
Diagnostics go to stderr. `created` and `closed` are asynchronous answers.

| command | effect | answer |
|---|---|---|
| `create KEY URL` | create an unmapped child at 0,0 with the initial size | `created` |
| `close KEY` | close only that browser | `closed`, from OnBeforeClose |
| `show KEY X Y W H` | move, resize, map and raise it; unmap every other page | `ok` |
| `size KEY W H` | resize without changing mapped state or position | `ok` |
| `hide` | unmap every browser | `ok` |
| `focus KEY` | CEF SetFocus(true); XSetInputFocus with RevertToParent if viewable | `ok` |
| `shield 1`, `shield 0` | enable/disable an InputOnly window over the shown browser | `ok` |
| `quit` | close all browsers and exit zero | process exits |

EOF acts as `quit`. Closing the last browser, including through CDP, leaves
the host running and ready to create another. `quit` also waits for pending
browser creation to finish and close. The shield follows show/size/hide;
it blocks pointer and wheel input but leaves agent CDP input available.
Only one browser is mapped at a time. Hidden children keep rendering and
remain available for screenshots, recording and screencasts. Neither
`WasHidden` nor lifecycle freezing is used.

| event | fields | meaning |
|---|---|---|
| `init` | `ms` | CEF initialized, elapsed ms from exec |
| `created` | `key`, `cmd:"create"`, `xid`, `target` | browser exists; target is exactly `/json/list`'s id |
| `popup` | `key`, `opener`, `url`, `xid`, `target` | new unmapped Alloy child, key `<opener>~<n>` |
| `closed` | `key`, `why` (`command`, `page`, `other`), `cmd:"close"` for a command | OnBeforeClose |
| `crashed` | `key`, `status` (CEF termination enum integer) | renderer terminated |
| `navigated` | `key`, `url` | main-frame address changed |
| `load` | `key`, `url`, `code` | main-frame load ended |
| `title` | `key`, `title` | document title changed |
| `focus` | `key` | CEF OnGotFocus; shown child receives X keyboard focus |
| `shield-click` | `key`, `x`, `y` | shield ButtonPress, device pixels in the shown page |
| `ok` | `cmd`, `key` | command completed |
| `error` | `cmd`, `key`, `error` | unknown, invalid or failed command |

CEF obtains target ids with `Target.getTargetInfo` through a registered
CefDevToolsMessageObserver. Geometry is in X (device) pixels; the page's
own scale (Xft.dpi, GDK_SCALE) applies inside it. Pages get the person's
languages from `$LANG` (`Accept-Language`); the UI locale is en-US. Clicking
into a shown page transfers keyboard focus to its X child. The host observes
XInput2 raw button presses through dlopen’d libXi.so.6 (without consuming
the browser’s events), since CDP focus emulation can suppress CEF’s usual
focus callbacks. XInput2 2.0 is required. Returning focus
to the embedding app is that app's responsibility.

## Helper and tests

`tools/browser` provides engine `cef`, adopting these targets with Playwright
CDP. Bun needs the guarded WebSocket source transform described in its README.
Never call Playwright `browser.close()` to disconnect from CEF; shut this
owned host down with `quit` and wait for exit.

```sh
BACKPLANE_CEF="$PWD/build/cef/Release/backplane-cef" bun tools/cef/test/host.ts
BACKPLANE_CEF="$PWD/build/cef/Release/backplane-cef" bun tools/browser/test/cef.ts
bun tools/cef/test/chrome.ts
```

Tests choose a free private Xvfb display 96..99, create an independent X11
parent, and use dlopen'd libXtst.so.6 for input. They terminate the helper,
host and parent through stdin/EOF, signal only their own Xvfb, and use temp
profiles. The top-level check excludes only CEF’s unmapped `Chromium clipboard`
selection-owner window; every actual browser UI must be a child. They never use display :0 or live Backplane data. CEF tests skip
with a message if BACKPLANE_CEF is absent.

The zygote changes its stack canary after fork, so `main()` has no stack
protector. See [SPIKE.md](SPIKE.md) for the established performance results
and [bench/](bench/) for the original spike's one-browser benchmark (its old
stdin protocol predates this multi-browser host).
