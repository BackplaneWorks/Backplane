# Boat jobs

Heavy commands (checks, proofs, tests, builds) can run on a disposable
[Boat](https://boat.dev) sandbox instead of this machine, so they don't
use its memory or cores. When Boat refuses a job (exit 75), nothing runs
locally by itself: run it locally the usual way instead.

```
work_run {note: "check on Boat",
          command: "cd <your worktree> && scripts/boat-job.sh --timeout 900 -- scripts/check.sh"}
```

Size by default: **large (16 GB) for anything that runs PROOF** (`check.sh`,
`check.sh proof`, `PROOF.bend`, the `dev.sh` gates): since #149 the proof
peaks above the 8 GB machine (killed at 7.45 GB on main 851a915b0;
`check.sh proof` on main 8cf5b48a2 peaks at 8.52 GB locally).
Everything else, `check.sh check` included, runs on default. `--size` wins.

Each thread keeps its sandbox **warm** after a job, for an idle window
(`--idle SECS`, `BOAT_JOB_IDLE`, default 1200, at most 3600; 0 deletes at
once). The thread's next job reuses it (no new Boat start): its lifetime is
set for that job, the previous job's tree is wiped (`build/` too, unless
`--keep-build`), the worktree pushed again, Bend installed only once. A warm
sandbox is used only by its own thread, only when free, and only for a job
of its size or smaller (a default job may use a warm large one, never the
other way). Its lifetime while warm is the idle window + 5 min, so Boat
stops it even if nothing else runs; a sweep deletes ours past the window.
Idle time counts against the daily cap. Runs without a thread (by hand),
cancelled jobs and failed setups delete at once. `--no-reuse` always
starts a new one.

Start guard: past 80% of Boat's hourly or daily start limit
(`BOAT_JOB_START_GUARD`), no new sandbox is started; a job from a thread
with a free warm sandbox still runs. The refusal says so.

Each job gets its own sandbox, deleted when it ends; no lock is needed.
Admission follows Boat's live limits, read before every create: active
sandboxes and the rolling start windows per minute, hour and day (other
clients' starts included, through Boat's own remaining counts; ours also
from our own record, whichever is stricter, less jobs still starting). A
full slot or minute/hour window waits (`--wait`, default 1 h, one limits
read per pause, at least 15 s apart); a full day refuses. A rate-limited
create (429) backs off (retry-after, else 15 s, 30 s, ...; at most 5 tries)
and is admitted again; a create whose answer is unclear is never retried,
so it never makes a second sandbox, and nothing is ever adopted: only an
id Boat returned is treated as ours (size, lifetime and creation time can
match another client's sandbox, so matches are only named for review). It
keeps its slot, its start and its worst-case budget (so does a job stopped
during its create) until a sweep reconciles it after its lifetime + 10 min:
with no matching sandbox still up it is closed at its worst-case cost; with
one still up it is unresolved, and new creates are refused until it is gone
(running jobs carry on). Other clients' sandboxes are never deleted. Run it from the worktree whose source should
be checked (integrate main into your own worktree; don't reset the shared
checkout).

## What is verified (default, 8 GB, 4 vCPU)
| command | result | wall | peak RSS |
|---|---|---|---|
| `scripts/check.sh` (type-check + PROOF) | exit 0 until #149; since then PROOF is OOM-killed at 7.45 GB: use large (the default for it now) | 149 s | 4.29 GB (before #149) |
| `scripts/check.sh proof` | as above | 98 s | 4.32 GB (before #149) |
| `scripts/test.sh` (136 programs) | exit 0 | 294 s | 3.38 GB |

Run proofs and checks through `scripts/check.sh` (or `check.sh proof`): it
sets what `bend PROOF.bend` needs, a 1 GiB stack (`ulimit -s 1048576`) and
`BUN_JSC_maxPerThreadStackUsage=536870912`, and judges the output with
`scripts/bendok.sh` (the "SOME PROOFS FAIL / N defs rely on unsafe or
foreign code" notice with exit 1 is a pass). A raw `bend PROOF.bend` without
both overflows the stack; that is a configuration error, not a failed proof.
The runner raises the stack limit; it does not set the JSC variable.

Small (4 GB) is not validated for any of these. The native build at the default 8 GiB hint does not fit 8 GB (its emit alone is about 8.8 GB); use `BACKPLANE_LOW_MEM=1` there, or `--size large` (16 GB) if it starts failing.

The outcome arrives like any work: the command's exit code and the end of its output.
Every job's files are in `~/.local/state/backplane/boat/jobs/<job>/`:
`job.log` (the full output), `setup.log`, `manifest.sha256`,
`artifacts/` + `artifacts.sha256`, and `provenance.json` (revision, dirty
flag, manifest hash, Bend version and sha256, sandbox, size, command, exit).

## Options
- `--size small|default|large`: 2 vCPU / 4 GB at half rate, 4 vCPU / 8 GB, or 8 vCPU / 16 GB at twice the rate. Default: large for PROOF, default for the rest; ask for large otherwise only when the job is measured to need it.
- `--idle SECS`, `--keep-build`, `--no-reuse`: the warm sandbox (above).
- `--timeout SECS` (default 1800): the command's limit; the sandbox lives at most that + 10 min, and never more than 2 h.
- `--artifact PATH` (repeat): copied back from the sandbox before it is deleted.
- `--exclude GLOB` (repeat): more files to keep out of the upload.
- `--budget-usd X` (or `BOAT_JOB_BUDGET_USD`): a lower daily cap for this job; it can only lower the shared cap, never raise it.
- `--wait SECS` (default 3600, or `BOAT_JOB_WAIT`): how long to wait for a free slot before refusing.
- `--no-setup-bend`: skip installing Bend (the pinned version and sha256 come from `.github/actions/setup-bend/action.yml`).
- `scripts/boat-job.sh sweep`: delete sandboxes this tool started whose job is gone. `status` (or `status --json`, `status --refresh` to read Boat's limits first; the same view is kept in `status.json` after every ledger change, and the last limits read in `account.json`, for the Usage settings): jobs, their state, owner thread, size, runtime, reserved and actual credits, today's spend.

## Rules it keeps
- Off unless the repository's root is listed in `~/.local/state/backplane/boat/enabled`.
- The runner runs from a private copy of itself, so editing or rebasing the worktree during a job doesn't change it; any unexpected exit still deletes the sandbox.
- Fail closed. A job is refused (exit 75) and nothing is created when Boat's limits can't be read, the account can't start, no slot frees up within `--wait` (jobs still starting count as taken), compute is short, the size isn't offered, the daily budget would be exceeded, or an earlier delete was never verified.
- Budget: the daily cap (UTC day) is shared by every job: `~/.local/state/backplane/boat/budget-usd` (set by the person; $0.50 when there is none, or when it can't be read). Each job reserves its worst case (lifetime × rate) under a ledger lock, so jobs started together can't overspend. The real cost (`boat usage`) is recorded when it ends. Boat has no spend cap of its own, so this is the only one.
- Source: `git ls-files -co --exclude-standard` (tracked and untracked-not-ignored files), without `.env*`, keys and certificates, `auth.json`, `*.log`, `*.jsonl` outside `test/`, `build/`, `dist/`, `node_modules/`, `shots-snap/`, `.claude/` and `.backplane*/`. Nothing from the home directory.
- Sandboxes: `boat new --no-env` (none of the account's stored secrets), `--no-snapshots` (erased when stopped), a bounded `--ttl`.
- Cleanup: the sandbox is deleted, then checked gone in `boat list`, on success, failure and cancel (`work_cancel`). A job killed outright is cleaned up by the next job's sweep. Only sandboxes in the ledger are ever deleted. A delete that can't be verified exits 77 and blocks new jobs until a sweep removes it.
- Exit codes: the command's own; 75 refused, 76 setup failed (or a create with an unclear answer whose sandbox couldn't be identified: check `boat list`), 77 cleanup unverified, 143 cancelled. Boat's URLs are never printed or logged.

Test: `test/tools/boat_job_test.sh` (a fake `boat`, `test/tools/boat-fake`; no network).
