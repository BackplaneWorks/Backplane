# Local compiler guard: bend / bun refuse outside an approved capped context

Goal: a `bend`, `bun` or `bunx` started on a developer machine must not run a compile, proof or test unless it is
inside an approved capped context. Boat sandboxes stay parallel and unaffected.

## What it does
`scripts/compiler-guard.py install --apply` (dry run by default; `uninstall --apply` restores the exact bytes)
moves the real `~/.bend/bin/bend` and `~/.bun/bin/bun` to `~/.local/share/backplane/guard/real/` and leaves a small
shell shim at the ORIGINAL path, so an absolute-path call (`/home/USER/.bend/bin/bend ...`) and a PATH lookup run the
same shim. `bunx` (a symlink to `bun`) goes through it with its own `argv[0]`. The shim refuses with exit 75 unless:

| Context | Rule |
|---|---|
| light subcommand | `bend` version/help/guide/base/update and no args; `bun` --version/-v/--revision/--help |
| Boat sandbox | `BACKPLANE_HEAVY_BOAT_SANDBOX=1` and `BACKPLANE_HEAVY_OFFLOADED=1` on a machine with MemTotal <= 8 GiB (the isolated-VM rule proof-shards.py already uses; a spoofed pair on a large host is refused) |
| capped local | the native `/tmp/bp-bend.lock` inherited from heavy-guard (`BACKPLANE_HEAVY_LOCK_FD`) AND the process's own cgroup has finite `memory.max` <= 3,700,000,000 and `memory.swap.max` 0 |

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
- **Relocated real binary** (`~/.local/share/backplane/guard/real/{bend,bun}`): running it directly bypasses the shim.
  Making it non-executable and exec'ing it through the dynamic loader does not work: bun standalone executables read
  their embedded payload from `/proc/self/exe`, which would then be the loader. A copy elsewhere (`cp`, a downloaded
  binary, `node_modules/.bin`, `~/.cache`, `~/.local/backplane-ci/bin/bun`) bypasses it too.
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
`test/tools/heavy_guard_test.py` and `test/tools/compiler_guard_boat.sh` (real pinned bend: absolute-path refusal,
allowed in Boat, allowed under cap + native flock, refused with only one of them, byte-exact uninstall).
