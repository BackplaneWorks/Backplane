# The copy guard (BACKPLANE_COPY=1)

A hub on a copy of a real log acts on nothing outside its home. One switch (`copy::on()`), four gates:

1. **The home** (`copy::gate`, `main.rs`, before anything is touched): a home that looks like a real hub's data (symlinked log, a real
   `machine.id`/`tz.checked`, a JS hub's `store.db`, a log over 256 MiB) is refused unless `BACKPLANE_COPY=1`; a home this hub made carries `hub.owner`.
2. **Processes** (`proc.rs`, the ONE place a child is made; a source scan fails the build on any other `Command::new`): every child has a
   `Kind`. On a copy `Net` (curl, tailscale, ssh...), `Terminal`, `Browser` and `Shell` are never started, `Agent` only from the
   `BACKPLANE_AGENTS` stand-in folder, `Local` (git reads, kicad-cli, our own helpers) unless the program is a network command by name.
   `files::git::proc` additionally refuses `git push|fetch|pull|clone|ls-remote` and network commands reached through `sh -c`.
3. **RPCs** (`copy::rpc_refused`, before any handler): `git.act`, `git.revert`, `file.open`, `review.release` are told, not run.
4. **Listeners**: a copy never starts the tailnet listener (`main.rs`: `tailscale: !no_tailscale && !copy::on()`); the only listener is the one
   named by `--host/--port`.

Raw sockets are not child processes, so gate 2 does not see them. There is no HTTP client crate; the non-test `connect` calls are
`presenter.rs` (loopback to our own policy child), `kicad/nng.rs` (the KiCad unix socket) and `net/tailnet.rs` `hello_with` (a raw
`TcpStream::connect` to other machines, which returns at once on a copy: `if copy { return }`). The test
`every_outbound_socket_is_on_the_allowlist` (proc.rs) fails the build on any other non-test `TcpStream::connect`, `UdpSocket`,
`TcpSocket`, `connect_async` or `UnixStream::connect`; a new one is added to its list by a reviewer, with its own copy check.

`kicad/stepglb.rs` `run_merged` is `Kind::Local "sh"` running `exec "$@"`: the by-name check cannot see the real program. Its callers are
fixed local tools today; a new caller that passed a network tool would get through, so keep it to fixed local programs.

## Review of the lanes (2026-11-05)

| lane | reaches outside the home by | on a copy |
|---|---|---|
| term | a shell on a pty (`term/pty.rs`), the browser helper (`term/browser.rs`) | Terminal / Browser: not started |
| net | `curl` (`botnet.rs`), `tailscale status|whois` (`botnet.rs`, `tailnet.rs`, `listener.rs`), the tailnet listener, far push/peers (curl) | Net: not started; listener off |
| settings | `curl` webhook (`webhook.rs`), `secret_run` shell (`vault.rs`), vault tools | Net / Shell: not started; vault tools Local |
| kicad | kicad-cli, plot/step helpers, git show (read) | Local: read-only on the project; no install path exists in the Rust hub |
| files | git (reads, writes, push), gh | writes and `file.open` told; push/fetch/gh refused; reads allowed |
| commands | git writes through `files::git::proc` (worktree on create/fork), folder picker scripts (Local), the worktree job runs on "/" on a copy | writes and network refused by the same gates; `copy::on()` checked before any git write |
| agents | the agent CLIs, the CLI version probe | stand-ins only (`BACKPLANE_AGENTS`); probe of `kicad-cli --version` is Local |

Not covered (no code yet): deploy/restart/update, watched work (`work_run`), webhooks other than settings', push notifications to phones. When
a lane adds one, it builds its process with `crate::proc` and a `Kind`, and adds its RPC to `copy::rpc_refused` if it writes outside the home.

Tests: `copy::tests` (the Bend laws), `proc::tests` (the rule per kind, the source scan, and `a_copy_run_does_nothing_outside_its_home`, which
starts a child test process with `BACKPLANE_COPY=1` and tries every outside-reaching kind), `test/gpui/hub/copy_gate.sh` (the home refusal, CLI).
