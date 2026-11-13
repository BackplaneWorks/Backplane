# Streamer mode (desktop client)

While on, the desktop client draws no identifier of the person: for screen recordings and streams. Settings → This device → Streaming, the
palette ("Streamer mode"), and the top bar's mark (a click turns it off) switch it.

## What is hidden

`native/gpui/src/streamer.rs` (`Masker`), a pure function with tests, applied to every string gpui lays out (the vendored gpui's text
filter, `vendor/gpui/PATCH.md`), to editable fields and the terminal (same length, `*`), and to the KiCad pop-out's window title:

| What | Drawn |
|---|---|
| any email address (`git@` remotes stay) | `•••@•••` |
| any home folder: `/home/NAME`, `/Users/NAME`, `C:\Users\NAME`, and this machine's and the hub's own | `~` |
| machine and host names the hub knows (linked machines, tailnet peers, this host) and this computer's; generic names like `desktop` stay | `machine` |
| a login name (`h@` before a host; whole words when 4+ letters) | `user` |
| `*.ts.net` names | `•••.ts.net` |
| IPv4 (with `:port`; loopback stays) and IPv6 addresses | `•.•.•.•` |
| account labels (Claude/Codex profiles) | `account N` |
| key-shaped tokens (`sk-`, `ghp_`, `tskey-`, `AKIA`, `Bearer …`), a link's `token=`/`invite=`/`code=` value, a pairing or sign-in code after "code", phone numbers | `•••` |
| Settings' pairing QR code and its short code | "The code is hidden in streamer mode." |

The identifiers come from the hub (`privacy.ids`, `native/host/src/settings/privacy.rs`) and from this computer. Secret values in the vault are
masked by the hub itself before anything reaches a client (`settings/mask.rs`), streamer mode or not. Not hidden: pictures (screenshots in a
thread, the Browser tab's page, the 3D and board views) and the embedded KiCad's own window, which draws itself.

## Turning on by itself

| Platform | Seen | Not seen |
|---|---|---|
| Linux | a portal screen cast (a running PipeWire `Video/Source` with no device: GNOME, KDE, wlroots), by `pw-dump` every 4 s; on by default | X11 capture (OBS's XSHM Screen Capture, ffmpeg x11grab): use the recording-app check |
| all | opt-in: a known recording app running (OBS, SimpleScreenRecorder, Kazam, vokoscreen, Peek, Kooha, GPU Screen Recorder, wf-recorder, ffmpeg grabbing; Loom, ScreenFlow, CleanShot, Kap, Ecamm; Streamlabs, XSplit, Bandicam, Camtasia) | whether it is recording now |
| macOS, Windows | only the recording-app check | the OS tells no app that the screen is captured |

Turned off by hand while auto-detect sees a capture, it stays off until that capture ends or another starts.

Phones: not yet (iOS could use `UIScreen.isCaptured`; masking there needs the same text filter in each app).

Rig: `scripts/desktop-streamer-shot.sh CLIENT HUB ASSETS OUTDIR` shoots the thread, account menu, Settings and Accounts, off and on.
