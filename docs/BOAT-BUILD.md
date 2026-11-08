# Builds on Boat when the local slots are taken

`scripts/cargo-q` and `scripts/dev-build.sh` take one of the machine's build slots (`scripts/lib-buildslot.sh`). When none is free they used to wait. Now they run the
same command on a Boat sandbox and the caller cannot tell, except for a line on stderr: `boat: all build slots are busy: ... on Boat bp-build-0 (bx_...)`.

## Flow (`scripts/lib-boat.sh`)

1. `bq_run` finds every slot taken and `BQ_OFFLOAD=1` → `bo_try` runs the script again under a Boat lease (`BUILD_LOCK_DIR/boat.N`, N < `BP_BOAT_POOL`).
2. `bo_box`: pool box `bp-build-N`, running (resumed if stopped, created from the snapshot `bp-build-base` if missing; `boat list --all`, stopped boxes are not in the plain list).
3. `bo_sync`: `git ls-files --cached --others --exclude-standard` → rsync over `boat ssh` (`scripts/boat-ssh.sh`) into `~/ws/src`, by checksum, mtimes not kept (cargo sees a changed file as new);
   `scripts/boat-prune.py` removes what the worktree does not have. Targets live in `~/target/*`, outside the copy, so they stay warm. sccache: `~/.cache/sccache` on the box.
4. `bo_run`: the job script (build knobs only, `bo_env`) runs `scripts/cargo-q ...` / `scripts/dev-build.sh` on the box through `scripts/boat-run.py` (detached, attached, re-attached after a cut connection without repeating output; ^C or TERM kills the remote command).
   stdout and stderr stay separate; the exit code is the command's. `--offline` is dropped there (the box has the network, no registry copy).
5. `bo_pull`: `build` → executables (`scripts/boat-artifacts.py`); `dev-build.sh` → hub, window, voice, guard (and Chromium's files only when this machine has no cached runtime). Hashed on the box, rsynced, `sha256sum -c` here.
   `dev-build.sh` then stages, **smoke-starts** (always, for a box's build) and publishes locally as ever.
6. `bo_finish`: removes the job, sets the box's own TTL (`BP_BOAT_IDLE`+600 s) and starts `scripts/boat-pool.sh reap`, which stops a box idle for `BP_BOAT_IDLE` (300 s). A stopped box keeps its disk (14 days): the next job resumes it warm.

Anything wrong before the command starts is exit 98: the caller waits for a local slot as before, and offloading is off for `BP_BOAT_COOLDOWN` (300 s). A command that cannot give the same result on a box is never offloaded
(`bo_cargo_ok`). Not offloaded: the main builder (`BQ_MAIN`), iOS/macOS (Mac mini, `mac-heavy`).

## Secrets

The snapshot is made by `scripts/boat-pool.sh base` on a sandbox created with `--no-env` and the environment `bp-build` (it injects nothing); boxes are created `--no-env`. Only `git ls-files` sources are copied (no `.git`, no home).
The job gets `RUST_*`, `CARGO_*` knobs, `PROFILE`, `BACKPLANE_COMMIT`, ...; a name with TOKEN, KEY, SECRET, PASS, CRED, AUTH, COOKIE or SESSION never goes, even in `BP_BOAT_PASS`.

## Cost

Large box (8 vCPU, 16 GB) at the plan's rate; `scripts/boat-pool.sh status` shows seconds and dollars per box. Idle stop after 300 s, a pool of 2, boxes only when slots are busy.
Env: `BP_NO_BOAT`, `BP_BOAT_POOL`, `BP_BOAT_TYPE`, `BP_BOAT_IDLE`, `BP_BOAT_COOLDOWN`, `BP_BOAT_PASS`, `BP_BOAT_BASE`.
