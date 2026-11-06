# The copy guard (BACKPLANE_COPY=1)

A hub on a copy of a real log acts on nothing outside its home. One switch (`copy::on()`), four gates:

1. **The home** (`copy::gate`, `main.rs`, before anything is touched): a home that looks like a real hub's data (symlinked log, a real
   `machine.id`/`tz.checked`, a JS hub's `store.db`, a log over 256 MiB) is refused unless `BACKPLANE_COPY=1`; a home this hub made carries `hub.owner`.
2. **Processes** (`proc.rs`, the ONE place a child is made; a source scan fails the build on any other `Command::new`): every child has a
   `Kind`. On a copy `Net` (curl, tailscale, ssh...), `Terminal`, `Browser` and `Shell` are never started, `Agent` only from the
   `BACKPLANE_AGENTS` stand-in folder, `Local` (git reads, kicad-cli, our own helpers) unless the program is a network command by name.
   `files::git::proc` additionally refuses `git push|fetch|pull|clone|ls-remote` and network commands reached through `sh -c`.
   **Correction (2026-11-05):** an earlier version of this file said a copy refuses writes. It did not: `Local` processes ran any program, and
   `copy::git_reads` only decided what the ** hub keeps (`Copy.out`); no Rust call site consulted it, and `agents/hist.rs` and others wrote.
3. **RPCs** (`copy::rpc_refused`, before any handler): `git.act`, `git.revert`, `file.open`, `review.release` are told, not run.
   This list is a second line, not the gate: see "Writes are default-deny" below.
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

Tests: `copy::tests`, `proc::tests` (the rule per kind, the source scan, and `a_copy_run_does_nothing_outside_its_home`, which
starts a child test process with `BACKPLANE_COPY=1` and tries every outside-reaching kind), `test/hub/copy_gate.sh` (the home refusal, CLI).

## Writes are default-deny (files::git, kicad_ipc)

`files::git::spawn` is the one runner for the files, commands and review lanes' `sh`/`git`/`kicad-cli` children. On a copy a command runs only
if its caller said it is a read: `read`, `proc_ro`, `run_ro`, `sh_ro` (the last three are not killed on drop; `read` is). `proc`, `run`, `run_idx`
and `sh` are "write or unknown" and answer code 126 ("writes nothing outside its home"). A new call site is therefore refused until someone
lists it as a read. "Read" here also covers a write only into the hub's home or a scratch folder the command made itself. `kicad_ipc` runs only
`Get*` and `Ping` on a copy (`copy_ipc_refused`); `mech.render` (which writes `renders/` into the project) is told on a copy.
Tests: `files::git::tests::a_copy_refuses_every_command_not_listed_as_a_read`, `a_copy_run_writes_nothing_outside_its_home` (a child with `BACKPLANE_COPY=1`
tries `touch`, `sh -c touch`, `git init`, a checkpoint and a step init; nothing is created, and the listed reads still run), `kicad::ipc::tests::a_copy_runs_only_the_listed_kicad_reads`.

### Call-site audit (2026-11-05; R = read, H = writes only inside the home or its own scratch folder, W = writes outside the home)

| call site | what | class | on a copy |
|---|---|---|---|
| files/git.rs `is_repo`, `status`, `refs`, `checkpoint_count`, `checkpoint_diff` (two refs), `diff` rev-parse | rev-parse, status, for-each-ref, diff | R | `*_ro`: runs |
| files/git.rs `snapshot`, `checkpoint_capture`, `checkpoint_revert`, `checkpoint_diff_worktree`, `diff` against the work tree (`to=-1`) | a private index file and blob objects in the project's `.git`, `update-ref`, `read-tree -u` | W | refused (a diff against the work tree is refused too: it adds objects to the real repository) |
| files/git.rs `step_capture`, `step_init`, `step_commit`, `step_push`, `step_pr`, `plain`, push | history.git, commits, network | W | refused (default-deny; network also by `copy::refused`) |
| files/git.rs `show` | `git show` | R | runs (own `Kind::Local` call) |
| files/misc.rs `history.resolve`, `files.list` (ls-files), `mech.list`, `pdf.page` | R (pdf: cache in the home) | R/H | runs |
| files/misc.rs `file.open` | opens a file with the desktop | W | told by `rpc_refused` and by its own check |
| files/misc.rs `mech.render` | writes `renders/*.png` into the project | W | told |
| files/rv.rs `review.bom`, `review.checks` (PREP archive, bound, kicad-cli checks, rm of its scratch) | scratch folder under TMPDIR | H | runs |
| files/rv.rs checks.json commands (`proc("sh", c.run)`) | arbitrary project commands | W | refused (and never trusted on a copy) |
| files/rv.rs `review.release`, `review.tick` (file write), release scripts, `kicad()` exports, PR | tag, outputs, gh | W | release told + default-deny; `review.tick` (writes `.backplane/checklist.md`) refuses on its own `copy()` check |
| files/export10.rs `kicad.export10` | kicad-cli writes `<home>/exports`; the project is only read; opening is skipped | H | runs |
| commands/fs.rs `list`, `resolve`, `find` | folder picker | R | runs |
| commands/fs.rs `mkdir`, `make` | creates a folder anywhere | W | refused |
| commands/worktree.rs reads (rev-parse, cat-file) | R | R | runs |
| commands/worktree.rs exclude edit, fetch/upstream, `worktree add`, carry | writes the project's repo, network | W | refused (also `copy::on()` at `spawn_with`) |
| commands/mod.rs scratch `git init` | `<home>/scratch/<thread>` | H | runs |
| bots/rpc.rs bot home `git init`, uberbot backup script | `<home>/bots`, `<home>/backups` | H | runs (not through `git::proc`) |
| kicad/ipc.rs `kicad_ipc` (agents/mcp.rs) | edits and saves the project's files in headless KiCad | W | only `Get*`/`Ping` |
| kicad/watch.rs, plotread.rs, stepglb.rs, presenter.rs, window.rs, agents/host.rs `--version`, settings/zone.rs, net/botnet.rs `hostname` | plots, GLB cache in the home, own helpers, probes | R/H | run |
| agents/hist.rs | writes history in the project folder | W | the agents lane is fixing it |

### Findings outside the files lane (not changed here)

* `settings/vault.rs` `offer()` picks the OS keyring (`secret-tool store`, macOS `security add-generic-password`) on a copy and writes the user's login keyring. It should choose `file` when `copy::on()`.
* `kicad/stepglb.rs` and `kicad/kipc.rs` are `Local` direct children (home cache; the IPC server). Their programs are fixed; keep them so.
