# Thread folder boundary security fix

A valid hub token was required. No live hub, real event log, real credential or
user repository was used for reproduction. Tests ran on disposable Boat workers.
This change belongs after the frozen 0.14.2 release.

## Confirmed reach

The baseline is `fc31e97f9772bc5a15b78262cfc05d45cdd32ebf`.

| Request / owner | Baseline result in the isolated fixture |
| --- | --- |
| `files.dir`, unknown or empty ID, empty path | Listed `/`: the shell starts at `/`, and `cd ""` does not select a thread folder. |
| Same IDs, path `etc` | Refused by the physical-directory containment check. `/etc` was **not** listed. |
| Same IDs, `..` / `../etc` | Refused by the relative-path check. |
| `files.list`, unknown or empty ID | Listed files under the hub's startup working directory. |
| `git.status`, unknown or empty ID | Reported on the disposable startup repository, including its dirty fixture file. |
| `git.diff`, unknown ID | Dispatched against the startup repository; absent checkpoints can produce an empty diff. |
| `git.revert`, unknown ID | **Restored the dirty startup-repository fixture file** when a matching fixture checkpoint existed. |
| Internal checkpoint job, unknown ID | **Created a checkpoint ref in the startup repository.** This is an internal job, not a public `git.checkpoint` RPC. |
| `git.act` commit, unknown or empty ID | **Created a commit in the startup repository.** |
| `mech.list`, unknown or empty ID | Reported the synthetic model in the startup directory. |
| Deleted thread | Still resolved to its project: directory listings and `/view/get`, `/files/get`, `/files/zip` remained accessible. |
| Archived thread | Kept its correct project folder. This remains supported. |

The write findings raise this above a directory-listing disclosure. Git writes
require the startup directory to be in a Git repository; revert also needs the
requested checkpoint. All demonstrated changes were restricted to newly created
fixture repositories. Nothing establishes compromise of a real installation.

Mechanical rendering, KiCad opening/export, review, history and terminal routes
also produced plans without a resolved folder on the baseline. Those plans were
checked without starting renderers, KiCad, terminals, release jobs or agents.
Actual writes by those programs were not claimed or tested.

`/view/get` and downloads already refused unknown/empty IDs; their lifecycle
checks missed deletion. Their existing traversal and static symlink escape
checks passed. The patch also checks mechanical model containment before a
render can start and uses the download descriptor checks for directory listings.

`/img` is an authenticated explicit-path viewer API, not a thread lookup. An
unrelated `thread` parameter does not scope it. Its cwd-relative text-file request
read a synthetic startup-directory file before the fix and now returns 404.
Authenticated absolute-path image/text/PDF viewing remains supported and is **not**
made thread-scoped by this PR. Likewise, `kicad.watch` is an explicit viewer-source
API; an active request with neither an absolute root nor an absolute source now
refuses. Plot-cache requests do not resolve a thread folder.

## Fix and verification

The shared thread/project resolver returns `None` for an empty or missing thread,
a deleted thread, a removed project or an invalid folder. Folder jobs refuse
before scheduling IO. The request-plan guard also discards changes and effects
with unresolved working folders, including turn checkpoints and terminal starts.
The process helper refuses an empty cwd as a second boundary. Directory listing
opens through the existing descriptor-confined download machinery; it omits
symlinks and names that could forge listing records.

The `thread_folder_refuses_job` law proves that no job, including a listing, is
scheduled when folder resolution returns `None`. No existing law was weakened.

Passing final evidence on Boat, under a 3,700,000,000-byte cgroup with no swap:

- `j20261105024745-2585314`: shared-boundary assertions, seven existing regression
  programs (`folder`, `rview`, `fallback`, `scratch`, `mailrun`, `files`,
  `files_state`), scoped refusal proof, directory/download effect tests, and
  before/after authenticated HTTP matrices. Peak reported process RSS was
  1,873,988 KiB for this job. The focused stage passed; the later whole-proof
  attempt was terminated, making the overall work job exit 143.
- The isolated handler hubs retain production request planning, root resolution,
  file handlers and relevant Git sinks byte for byte. The fixture substitutes
  startup/model/network wiring and supplies temporary files, not folder policy.
  This is not a full application/WebSocket end-to-end build.
- `j20261105024300-2542962`: the source guard passed. `scripts/check.sh check` and
  `scripts/check.sh proof` refused admission (exit 75): they require 14/19 GiB,
  produced no full verdict. Earlier full baseline hub-build attempts under an
  8 GiB cap were also terminated. No full build/proof pass is claimed.

The fixed regression gate needs no historical archive:

```sh
scripts/boat-job.sh --size default --wait 0 --timeout 700 \
  --artifact build/thread-folder -- \
  scripts/heavy-boat.sh bash test/tools/thread_folder_boat.sh
```

For the optional before/after negative control, prepare the pinned source archive
before transport (source packaging only), then set `THREAD_FOLDER_BEFORE=1` in
the remote test command:

```sh
mkdir -p .ci-packet
git archive --format=tar.gz fc31e97f9772bc5a15b78262cfc05d45cdd32ebf src \
  > .ci-packet/thread-folder-before.tar.gz
```

The snapshot loader checks Git's archive revision header. Generated archives,
fixture builds, logs and matrices stay outside the committed source.
