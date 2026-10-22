# Terminal

The integrated terminal (T3Code's per-thread panel on mod+j) is two pieces:
a pty effect that runs a shell, and a pure emulator that turns its output
into styled rows. The app and hub wire them together.

## Pty (`src/server/pty.bend`, `effects/pty.c`)

```
Pty.spawn(cmd: String, args: List<&2, String>, cwd: String, cols: U32, rows: U32)
  -> IO(Result<&1, &1, U32 & String, U32 & Socket>)   # (pid, master)
Pty.resize(s: Socket, cols: U32, rows: U32) -> IO(Socket)
```

- `cmd` is searched on PATH; `cwd` "" keeps ours. The child is a session
  leader with the pty as its controlling terminal, stdin/stdout/stderr on
  it, `TERM=xterm-256color`, `COLORTERM=truecolor`, no `LINES`/`COLUMNS`,
  default signal dispositions, and none of our other descriptors.
- Exec failures come back as `Fail` (errno through a close-on-exec pipe):
  `ENOENT` for an unknown command.
- The master is a `Socket`: read it with `H.Sock.recv_bytes` (it parks on
  the event loop like any socket; `[]` once the child and its descendants
  have closed the pty), write keys with `H.Sock.send_bytes`, hang up with
  `Socket.close` (the shell gets SIGHUP). Reap with `H.Proc.wait(pid)`;
  `H.Proc.kill` signals it. `Sock.shutdown` does nothing on a pty (send ^D).
- `Pty.resize` sets the window size; the kernel sends SIGWINCH.
- Linux and macOS (posix_openpt, fork, TIOCSCTTY). Windows builds answer
  ENOSYS.
- The JS twin (`effects/pty.js`, the hub under Bun) cannot fork: it uses
  posix_spawn through bun:ffi with POSIX_SPAWN_SETSID. On Linux the child
  opens the slave as fd 0, which a session leader takes as its controlling
  terminal. On macOS the slave is opened first (macOS refuses TIOCSWINSZ on
  a master whose slave nobody has open) and the child is `/usr/bin/perl`,
  which takes fd 0 with TIOCSCTTY and execs the program. Linux needs glibc
  2.29+ (`addchdir_np`); 2.34+ closes our other fds with `addclosefrom_np`.
  `host.js`'s `Proc.wait` parks on a pidfd (Linux) or a 50 ms timer, so the
  loop never blocks. Test: `bend test/js_pty_test.bend`.

`host.c`'s and `host.js`'s `Sock.recv_bytes`/`send_bytes` fall back from
`recv`/`send` to `read`/`write` on ENOTSOCK, and read EIO (a pty's hang-up)
as end of stream.

## Emulator (`src/core/vt.bend`)

```
Vt.new(cols: U32, rows: U32) -> Vt
Vt.feed(v: Vt, bytes: List<&2, U32>) -> Vt      # any chunking
Vt.resize(v: Vt, cols: U32, rows: U32) -> Vt
Vt.replies(v: Vt) -> List<&2, U32>              # write these to the pty
Vt.drain(v: Vt) -> Vt                           # ... then clear them
Vt.title(v: Vt) -> String                       # OSC 0 / 2
Vt.cursor(v: Vt) -> VCur{x: U32, y: U32, visible: Bool}
Vt.lines(v: Vt, scroll: Nat) -> List<&2, VLine> # rows on screen, scroll back
VLine{runs: List<&2, VRun>}
VRun{text: String, fg: U32, bg: U32, bold: Bool, underline: Bool}
Vt.key(v: Vt, keysym: U32, text: U32, mods: U32) -> List<&2, U32>
Vt.paste(v: Vt, text: String) -> List<&2, U32>
```

Also: `Vt.cols`, `Vt.rows`, `Vt.history` (rows in the scrollback, the most
`scroll` can reach), `Vt.alt` (a full-screen program has the alternate
screen), `Vt.width(cp)` (0, 1 or 2 cells), `Vt.fg()` / `Vt.bg()`.

Rows and runs:

- `Vt.lines` gives exactly `rows` lines; with `scroll > 0` the top ones come
  from the scrollback. Trailing blank cells (space, default background, no
  underline or inverse) are left out, so an empty row has no runs.
- Colors are 0xRRGGBB, resolved: default fg `#d9d9e4`, bg `#0d0f12`, a dark
  One-Dark-like 16-color palette, the xterm 6x6x6 cube and gray ramp, and
  true color. Inverse is applied; dim is fg blended halfway to bg; hidden is
  fg = bg.
- A wide character (CJK, emoji) takes two cells but is one character in
  `text`: a renderer advances two cells for it (`Vt.width`).

Keys: X11 keysyms. `text` is the code point the key typed (0 for none);
`mods` bit 0 shift, bit 2 ctrl, bit 3 alt. Return CR, BackSpace DEL (ctrl:
BS), Tab / shift+Tab `ESC [ Z`, Escape, arrows / Home / End (`ESC O x` in
application cursor mode, `ESC [ 1 ; m x` with modifiers), Insert / Delete /
PageUp / PageDown / F5-F12 (`ESC [ n ~`, `ESC [ n ; m ~`), F1-F4
(`ESC O P..S`), the keypad, ctrl+letter and ctrl+`@[\]^_? 2-8` as control
codes, alt as an ESC prefix. Paste turns LF and CR LF into CR; with
bracketed paste (?2004) it is wrapped in `ESC [ 200~ .. ESC [ 201~` and
ESC bytes are dropped.

## What the emulator does

- UTF-8 across chunk boundaries; bad bytes become U+FFFD. Combining marks
  are dropped.
- C0: BS, HT (stops every 8), LF / VT / FF, CR; BEL ignored; CAN / SUB
  cancel a sequence.
- ESC: 7 / 8 save / restore cursor, D index, M reverse index, E next line,
  c reset (the scrollback stays), `( 0` / `( B` DEC line drawing. DCS, SOS,
  PM, APC strings are skipped to ST.
- CSI: A B C D E F G H f d `` ` `` a e (moves), I Z (tabs), J K (0 / 1 / 2,
  J 3 clears the scrollback), L M P @ X, S T (scroll), m, r, s u, n (5, 6),
  c and `> c` (device attributes), h l (4 insert). `?` h / l: 1 application
  cursor keys, 6 origin, 7 autowrap, 25 cursor, 47 / 1047 / 1049 alternate
  screen (1049 saves the cursor), 1048, 2004 bracketed paste. Others (mouse
  modes, cursor style, window ops) are parsed and ignored.
- SGR: 0, 1, 2, 4, 7, 8, 21, 22, 24, 27, 28, 30-37, 39, 40-47, 49, 90-97,
  100-107, 38/48;5;n, 38/48;2;r;g;b (italic, blink, strike are ignored).
- OSC 0 / 2 set the title (BEL or ST ends it); other OSCs are ignored.
- Replies (DSR, DA) queue up for `Vt.replies`.
- Scroll regions; lines leaving the top of the screen (or of a region that
  starts at the top) go to a 2048-row scrollback, on the main screen only.
- Resize cuts or pads rows (no reflow); a screen that gets shorter than the
  cursor's row gives its top rows to the scrollback.

Not done: tab stops other than every 8 (HTS / TBC), the colon SGR form
(`38:2::r:g:b`), REP, mouse reporting, G1 / SO / SI, double-width lines,
reflow on resize, OSC 8 links and OSC 52.

## Terminals in the chat

An agent can put a terminal in its thread (core/emb.bend): a command run
through the user's shell (`$SHELL -lc`) on a pty of its own, in the
thread's folder, shown in the timeline where the person clicks into it
and types. It is how an agent hands the person a small tool: a TUI, a
picker, an interactive script, a monitor.

- Tools (src/core/mcp.bend): `terminal_open {command, title, cols, rows}`
  logs an Act of tone `term` (kind `terminal`, id the terminal's id,
  summary `title\nCOLSxROWS\ncommand`) and runs it; `terminal_read {id,
  waitMs}` answers its state (`running`, `stopped`, `exited N`) and the
  text on its screen, waiting up to waitMs for it to exit (the server asks
  the terminal actor, off the hub); `terminal_send {id, text}` types;
  `terminal_close {id}` hangs it up.
- Keys: a terminal's bytes travel as `{"t":"term","thread":"x:<thread>:<id>"}`
  like a thread's own shell's, input as `term.input` with that key. The
  hub's terminal actor (src/server/term.bend) keeps each one's screen
  (`ETerm`, a `Vt` fed with everything it prints) and answers the
  program's own queries (cursor reports) itself, so clients don't.
- Snapshots: a client that shows a live terminal it holds no screen for
  asks `term.snap {key}` once (client.bend's `Emb.sync`); the reply is a
  `term` message with `"reset":[cols,rows]`, bytes that draw the screen on
  a blank terminal (`Emb.snap`: alternate screen, rows, modes, cursor,
  title). A restart sends a reset to every client.
- Live or inactive (`Emb.live`, laws `emb_*`): a terminal is live while
  pinned, or until the person sends a message after it was opened or last
  restarted, or for 30 minutes. A live one is drawn as a terminal; an
  inactive one is one gray row whose click sends `term.restart {thread,
  id}` (the command runs again under the same key, the setting
  `emb.at:<key>` makes it live from then). The pin is the setting
  `emb.pin:<key>`; pinned terminals are drawn at the top of the thread with
  the subagents and leave a row in the timeline.
- The next message: when the person writes, the server asks the actor for
  what the thread's terminals show that the agent has not seen
  (`TNote`), puts it after the message as a `<terminals>` block (Claude,
  Codex, Grok and Muse alike), and stops the ones not pinned.
- When a program exits its last screen is written to `<home>/terms/<key>`
  (`COLSxROWS<tab>state`, then the snapshot), so reads and snapshots work
  after the hub starts again.
- Clients: the window (layout.bend's `TL.emb`, kinds 23-26, `Ew.over`
  draws each screen over its rows; `Lay.pins`; nui.bend routes keys while
  the focus is `emb:<key>`), the web (`View.emb`, `View.pins`; host.js
  puts a field's `data-for` first in what it sends), phones (screen.bend's
  `embs`, entry rows of kind `emb`). Actions: `emb-key`, `emb-paste`,
  `emb-restart`, `emb-pin`.
- Test: `test/emb_test.bend`; end to end `bun test/tools/emb_e2e.ts`.

## Cost

Printing a character rewrites one path in the row tree and one in the screen
tree; a full-screen scroll replaces one row. `test/native/vt_bench.bend`
feeds about 1 MB: native, ~0.32 s for colored test-runner output and
~0.38 s for plain text (120x40); the JS target (bun) ~1.3-1.8 s.
`Vt.lines` for 120x40 takes a few ms.

## Tests

- `test/vt_test.bend` (in `scripts/test.sh`): the emulator.
- `test/native/pty_test.bend`: real programs on a pty (printf with color,
  `stty size`, `tput cols`, `cat` echo and ^D, resize, env and cwd, a
  missing command). `bend test/native/pty_test.bend -o build/pty_test &&
  build/pty_test`.
- `test/native/vt_dump.bend`: run a program for 1.5 s and print its screen
  (`build/vt_dump -- vi -u NONE file`).
