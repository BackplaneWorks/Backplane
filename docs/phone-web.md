# The web client on a phone

A phone's browser gets the phone layout: the hub draws the same views one
screen at a time, with a header (‹ and the screen's name) and, on the root
screens, a tab bar (Now, Projects, Bots, Settings). Detection, the shell,
its gestures and the lists are `src/web/phone-shell.js`, `src/web/phone.bend`
and view.bend's `View.phone.*`; everything else on a phone is in style.css
under `html.phone` / `.app.phone` and in `src/web/phone-other.js`. A
desktop or a narrow desktop window never gets any of it.

## Getting in

- An invitation link opened on the phone shows **Connect this device** right
  under the device name; the other ways in follow it. A phone does not focus
  the link field, so its keyboard does not cover Connect.
- A home-screen app keeps its own cookies, so it pairs on its own: it opens
  on **Short code**, with a line saying why.
- **Settings → Devices** puts ‹ Back first and lists the phone itself first,
  marked **This device** (the browser keeps the device id `/auth/enroll`
  answers; see docs/pairing.md).

## Moving around

- **Tabs** (on the root screens only): **Now** (the default: what needs you
  first, then what runs, then the recently settled, over every project; its
  badge is the need-you count), **Projects** (the project tree, whatever the
  sidebar view setting), **Bots** (bots and rooms; badge: rooms with unread)
  and **Settings**. Which tab is open is this browser's own; other clients
  never follow it. Now groups its rows as Needs you, Finished, At work and
  Idle. Tapping the open tab again goes back to the top of its list.
- **Screens push and pop**: a thread, a project's home, a bot or room or a
  Settings page opens over the tab with a ‹ in its header; the viewer opens
  over its thread. ‹, the system back (Android's key, the browser's) and a
  swipe in from the left edge (the screen follows the finger) go back; back
  first shuts whatever is open over the screen (a menu, a sheet, search, the
  palette, the terminal, the project picker, the viewer's full screen).
- **Rows**: a tap opens; a long press opens the row's or project's menu (what
  a right-click does on a computer) as a sheet; a swipe right settles a
  thread, a swipe left offers Snooze 1h or Delete (asked in the menu).
  Rename from the menu opens a sheet above the keyboard.
- **Headers**: Now and Projects have New thread (in the project at the top of
  the list), search and the command palette; a thread's has ⏮ Go to the
  start (every earlier page, then the top; a line under the header counts
  the pages, with Cancel, then offers Newest ↓), ▾ its menu and its views.
- Every menu is a bottom sheet; a tap on the dimmed page or back shuts it.
  The first open on a slow network shows "Backplane / Loading…" until the
  page is drawn.

## Finding things

- Search (the magnifier) is a full-screen sheet that ends at the keyboard.
  It lists threads whose titles match (the sidebar's at once, then every
  thread's from the hub: thin-view.bend's `Search.titles`) above what was
  said in them.
- The command palette (⚡) is a full-screen list of 52 px rows without the
  keyboard shortcuts; on a root tab it leaves out commands that need an open
  thread (workspace.bend's `WS.scoped`).

## Terminal

Full screen above the keyboard, with a key row: Esc, Tab, a sticky Ctrl
(the next letter typed goes as Ctrl+letter), ^C, the arrows, `| ~ / -` and
Paste. A swipe on the screen scrolls its history. On a phone on its side
with the keyboard up, the path and title bar give way.

## Settings, bots, rooms, projects

- Every Settings control is at least 44 px with 16 px fields; a focused
  field scrolls above the keyboard.
- **Settings → Appearance → Use the computer layout** switches this browser
  to the desktop page; a **Phone layout** button there brings it back.
- **Settings → About → Send an improvement** opens Improvements.
- A bot's pages are a row of chips under its cat; while you drive its
  Browser page, a line under it takes the phone's keyboard (typed text, Enter,
  ⌫). A room's field and Send sit on one line; a project's home lists its
  threads before its viewer; the project picker (Projects' +) has its own ×.
- The Ready to order bar and a buy thread's order card have 44 px buttons.

## Signals

- **Settings → Threads → Notifications on this phone** asks for permission
  (a phone is never asked on its first tap). The hub's notifications are
  shown through the service worker (sw.js), the only way Android Chrome and
  iOS allow; a tap opens the thread. On an iPhone they work once Backplane
  is added to the Home Screen (the page carries a manifest for it).
- The number of threads that need you is the app icon's badge and the
  page's title, "(2) Backplane".
- With the hub out of reach, "Reconnecting…" is a line under the header.
- Over a plain http:// address the row says notifications need HTTPS (see
  below) instead of offering Allow.

## HTTPS, installing, the first open

- A phone usually reaches the hub at its LAN or tailnet address over plain
  http. Browsers give such a page no service worker, no notifications, no
  badge and no real install (Android makes only a bookmark). A hub started
  with `--tailscale-https` is also at `https://<machine>.<tailnet>.ts.net/`
  (`tailscale serve`); a phone paired at that address gets all of them and a
  smaller download (brotli, below).
- Add to Home Screen: the page carries a manifest (standalone, 192/512 px and
  maskable icons, splash and status bar in the page's theme; an iPhone gets
  the 180 px touch icon and, on the light theme, a status bar with dark text).
- The first open downloads the bundle (6.4 MB; 1.22 MB gzip, 0.88 MB brotli,
  which browsers take only over HTTPS or on localhost; the builds put a .br
  beside each chunk where the `brotli` tool exists). On a slow phone (CPU
  4x, 1.6 Mbps) that is 6.75 s over http and 5.1 s over HTTPS; compiling and
  running it is about 0.3 s. Later opens come from the HTTP cache (0.7 s) or
  the service worker (0.5 s).
- The service worker gives the page from its cache when the network has not
  answered in 2.5 s, so the app opens on a weak signal or with the hub away;
  a page that arrives late caches its build's chunks, then itself, and the
  open page reloads into a new build once the link answers.

## Losing the link

- A phone that slept, changed networks or came back online asks the hub for
  a view at once; a link that brings nothing in 6 s is replaced. After a
  Send or an answer, nothing for 12 s means the same (the text goes back in
  the field). Both wait while a viewer is drawing (its pictures can take
  minutes on a slow link). Desktop keeps the 40 s heartbeat lease.
- Each failed retry gives the next one twice the time to connect (up to 4x),
  so a link whose answers take seconds still gets back.

## Testing

`node --test desktop/test/phone-shell.test.cjs desktop/test/phone-other.test.cjs desktop/test/service-worker-notify.test.cjs desktop/test/service-worker-cache.test.cjs`,
`node --test test/tools/static_file_test.mjs`,
`bend test/phone_shell_test.bend`;
in a browser, `?phone=1` on a desktop Chromium forces the phone layout
(`?phone=auto` forgets it).
