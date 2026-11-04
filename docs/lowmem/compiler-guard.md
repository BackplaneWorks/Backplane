# Local compiler guard: bend / bun refuse outside an approved capped context

Goal: a `bend`, `bun` or `bunx` started on a developer machine must not run a compile, proof or test unless it is
inside an approved capped context. Boat sandboxes stay parallel and unaffected.

## What it does
`scripts/compiler-guard.py install --apply` (dry run by default; `uninstall --apply` restores the exact bytes)
renames the real `~/.bend/bin/bend` and `~/.bun/bin/bun` to `bend.real` / `bun.real` in the SAME directory (the
Bun-compiled `bend` finds its Base library and guide relative to its own location, so it cannot be moved elsewhere;
found on Boat) and leaves a small shell shim at the ORIGINAL path, so an absolute-path call (`/home/USER/.bend/bin/bend ...`) and a PATH lookup run the
same shim. `bunx` (a symlink to `bun`) goes through it with its own `argv[0]`. The shim refuses with exit 75 unless:

| Context | Rule |
|---|---|
| light subcommand | `bend` version/help/guide/base/update and no args; `bun` --version/-v/--revision/--help |
| Boat sandbox | `BACKPLANE_HEAVY_BOAT_SANDBOX=1` and `BACKPLANE_HEAVY_OFFLOADED=1` on a machine with MemTotal <= 8 GiB (the isolated-VM rule proof-shards.py already uses) AND the process's own cgroup, read from the kernel, has finite `memory.max` <= 3,700,000,000 and `memory.swap.max` 0. No local lock: Boat runs stay parallel. `scripts/heavy-boat.sh` runs its command only inside such a scope: it verifies its own cgroup from the kernel, wraps with `systemd-run` otherwise, re-verifies from inside the new scope (a substitute `systemd-run` that ignores the cap is caught), ignores any `BACKPLANE_HEAVY_SCOPED` variable, and exits 75 before the command or tool setup when no cap can be verified |
| capped local | the native `/tmp/bp-bend.lock` GENUINELY held through the inherited descriptor (`BACKPLANE_HEAVY_LOCK_FD`; a fresh open of the lock file must be refused the flock while the inherited descriptor can still take it, so an opened but unlocked descriptor fails) AND the same kernel-read cgroup cap |

`bend update` is allowed and re-applies the shim afterwards. `scripts/heavy-guard.py` now also refuses to admit a
whole-workload profile (check, proof, test, build, web, browser, compile-js) locally without that verified cap, so
`check.sh` and the other guarded entry points send the work to Boat instead of running it uncapped; the shard
profiles (proof-shard, compile-shard, fixture-check) already required the cap.

## Coverage
- Hub `work_run`, agent shells and terminals: launcher independent, because the shim is at the installed path that
  PATH resolves to; no PATH shim or launcher change is needed.
- Repo entry points (`scripts/check.sh`, `test.sh`, `build-*.sh`, `dev.sh`, `smoke.sh`, ... and any direct `bend`/`bun`
  call in a script): refused at exec time by the shim; the heavy-guard-wrapped ones are routed to Boat first.
- `bend2` (the TypeScript compiler run as `bun .../bend2/main.ts`) runs through the `bun` shim.

## What cannot be enforced without operating-system support (investigated on this machine, kernel 7.0, uid 1000)
LSMs present: lockdown, capability, **landlock**, yama, apparmor, bpf, ima, evm. Unprivileged user namespaces are on.
- **Relocated real binary** (`~/.bend/bin/bend.real`, `~/.bun/bin/bun.real`): running it directly bypasses the shim.
  Making it non-executable and exec'ing it through the dynamic loader does not work: bun standalone executables read
  their embedded payload from `/proc/self/exe`, which would then be the loader. A copy elsewhere (`cp`, a downloaded
  binary, `node_modules/.bin`, `~/.cache`, `~/.local/backplane-ci/bin/bun`) bypasses it too.
- **A `bun` installed in a root-owned directory** (the Boat image has `/usr/local/bin/bun`) cannot be replaced by an
  unprivileged installer; `--path bun=/usr/local/bin/bun` guards it only where the directory is writable.
- **Anything started by something else with its own PATH or absolute path**: other users, systemd units, cron,
  containers, editors that bundle bun, `python -c "os.execv(real, ...)"`.
- **Reinstall outside `bend update`** (`curl | sh`, a bun self-upgrade): the path holds a real binary again until
  `compiler-guard.py check` (exit 1) is run or the installer is re-applied.
- **Environment claims**: the capped-local rule trusts the kernel's cgroup files (not spoofable by environment) but
  the lock descriptor is a plain environment variable; the Boat rule trusts an environment pair bounded by MemTotal.
  This is a guard against accidental heavy runs, not a sandbox.
- Hard enforcement options that do exist here, none done by this PR:
  * **Landlock** (available, unprivileged, irrevocable, inherited by children): a launcher can start agent
    processes in a ruleset that allows `EXECUTE` only under an allowlist (`/usr`, `/bin`, `~/.nvm`, the shim
    directories, the repo, ...) and not under `~/.local/share/backplane/guard/real`. Rules only add allowances to a
    path hierarchy, so it is an allowlist, not "deny this binary"; it does not reach processes the launcher did not
    start, and a copy inside an allowed directory still runs. It needs the hub launcher owner to apply it.
  * **AppArmor / SELinux profile, fanotify exec permission, binfmt_misc, BPF LSM**: need root.
  * **seccomp**: cannot match an exec path without a user-notification supervisor; not practical here.
  * **Root-owned shims** (`chattr +i`, a root-owned `/usr/local/bin` first on PATH): needs root.

## Install / remove (run by the person, not by CI)
    python3 scripts/compiler-guard.py install --tool bend --tool bun            # dry run
    python3 scripts/compiler-guard.py install --tool bend --tool bun --apply
    python3 scripts/compiler-guard.py check
    python3 scripts/compiler-guard.py uninstall --tool bend --tool bun --apply
Refusing `bun` also refuses a legitimate local bun-run service; install only `--tool bend` if that matters.
The shims read the vendored `~/.local/share/backplane/guard/compiler-guard.py`, not the worktree.

## Validation
Boat only (normal size, scope MemoryMax=3700000000, MemorySwapMax=0): `test/tools/compiler_guard_test.py`,
`test/tools/heavy_guard_test.py` and `test/tools/compiler_guard_boat.sh` (real pinned bend: absolute-path refusal with
no context, allowed on Boat inside the cap, Boat environment without a cap refused, allowed under cap + a genuinely held
native flock, refused with only the cap, only the flock, or an opened-but-unlocked descriptor, byte-exact uninstall).
First Boat run (head 60462c7) failed the real-executable cases because the relocated bend could not find its Base
library; that evidence is kept in bp-coord/lowmem-swarm/lane3/guard-boat-evidence/.

## Broad impact on Boat (requested by the current rule: every Boat compiler/test run has a 3.7 GB / swap-0 cap)
`scripts/heavy-boat.sh` is the wrapper for offloaded Boat jobs, so EVERY command it starts (checks, proofs, tests,
builds) now runs under `MemoryMax=3700000000`, `MemorySwapMax=0`, whatever the sandbox size. A job that needs more
(for example the full proof, which needed 14 GB on an xlarge sandbox) will now be OOM-killed instead of succeeding
on a big VM; that is the intended consequence of the rule, but it changes behaviour for any CI job that relied on
the larger sandbox, and a Boat image without a user systemd (`systemd-run --user --scope`) refuses everything
(exit 75). The check applies to the cgroup of the process that starts the command, not to its children's total if a
command itself creates sibling scopes.
