# Snapshots before agent turns (#1163)

A review request on a KiCad project that had been copied into a plain folder removed about 90% of the board, and nothing could bring it
back: the per-turn checkpoints (`agents/hist.rs`) skip a folder that is not a git repository. Now the hub, never the agent, guarantees a
way back.

## Every project is a git repository

`files/snapshot.rs` `ensure_repo`, at `project.add`/`project.new` (in the background) and at every turn's start (before the agent gets
its input):

- A folder in no repository: `git init`, git config `backplane.created=true`, `/.backplane/` in `.git/info/exclude`, the default
  `.gitignore` (KiCad's backups, `_autosave-*`, `#auto_saved_files#`, lock files, `fp-info-cache`, `*.kicad_prl`, `.history/`,
  build outputs) appended to the folder's own if it has one (never rewritten), then a first commit of everything there:
  "Backplane: snapshot of existing work before first use".
- A file over 50 MB is left out and named in the thread's line ("Left out, over 50 MB each: ..."), never silently.
- A home folder, the filesystem root, or more than 50,000 files is not made a repository, and the thread says so.
- A folder already inside a repository is left as it is.

The hub's commits are made as `Backplane <backplane@localhost>` with hooks and the repository's filters off (`files/git.rs` `SAFE`), so a
computer where git was never told a name still gets its snapshots. Nothing is stashed, reset or discarded.

## A snapshot before every turn and before its first write

`agents/snapguard.rs` `turn_guard_start` runs at all four places a turn starts (after `settle_ends`, before the agent's input). It takes
the work tree (tracked, untracked, not ignored, not huge) as a commit named `refs/backplane/snapshots/<thread>/<n>` with the
subject "Backplane: snapshot before <thread> turn <n>" (the thread's title in the body):

- In a repository Backplane made, the commit is on the checked-out branch.
- With Settings > Projects and git > Keep Backplane's commits off my branch on (`git.off_branch`, off by default; #1188, docs/git.md), it is on its ref there too, and such a repository's first commit is `refs/backplane/first`, its branch left unborn: no snapshot or restore then writes any branch.
- In a repository the person already had, it is a commit of its own whose parent is HEAD. Their branch, index, staged changes and stash
  are untouched (the KiCad lead's condition: such repositories carry hooks, landing rules and other threads' half-done work).
- When nothing changed, the ref names HEAD.

The turn's first write through the hub (`kicad_edit`, a `kicad_ipc` write, `files_write`/`mkdir`/`rename`/`delete`) snapshots again
(`<n>-edit`) before the write, before the document's gate and any save. A snapshot reads only what is on disk. It never triggers a save
or takes a document's lock. The design files the turn changed are still committed to main at its end by `agents/design.rs`
(`design_record`). Snapshots, restores and those design commits take one lock per repository (`snapshot::repo_lock`).

Git missing: the turn does not start. It fails with what to do, and the thread gets an Install Git ask (setup's own install,
`settings/tools_install.rs`).

## A review is read-only

A request that only asks for a review, an assessment, a summary or a task list (`snapshot::is_review`) puts the thread in review
mode (setting `review.ro.<thread>`). It lasts through follow-ups until a message asks for a change (fix, update, route, delete...).
In review mode:

- The hub refuses writes to design files from `kicad_edit`, `kicad_ipc` (reads, `OpenDocument`, layers, selection, checks and exports
  go on: `snapguard::review_allows`) and the `files_*` tools. Each refusal says why and how the person asks for the change. The
  review's own `.backplane/review.md` is no design file.
- Claude is started with deny rules for the design files' Edit/Write and for `rm`, `mv` and the git commands that discard work
  (`snapguard::CLAUDE_DENY`). Deny rules hold in every permission mode, and a change of mode restarts Claude.
- At the turn's end, any design file a review changed (whatever tool it used) opens the restore ask.

## Mass deletions

- Through KiCad's IPC, `mass_guard` counts a document's net items deleted in the turn. `DeleteItems` counts against, `CreateItems`
  counts back (a reroute nets out), and a commit dropped with `CMA_DROP` gives its deletions back. Past a fifth of the document's items,
  and at least 10, the call waits for the person's approval (an approval ask). The agent cannot answer it, and a decline refuses the
  call.
  - In a turn the person did not start (a bot's, a delegated subthread's, a wake, a queued or relayed message), nobody can answer, so
    the call is refused at once and no ask is opened.
  - In the person's own turn it waits at most 5 minutes (`MASS_WAIT`); then the ask closes and the call is refused.
  - While it waits, no KiCad request is sent and this call takes no document lock. A lock the turn's earlier writes took stays held
    until the turn ends, as always.
  - The denominator is the saved file's item count at the turn's first write (Hubble_DK, 2.5 MB: about 4 ms). The person's unsaved
    edits can shift it a little, but only the turn's own IPC deletions are ever counted as the agent's.
- At the turn's end, `turn_guard_end` compares every board and schematic in the snapshot with the disk (`count_items`: footprints,
  tracks, vias, zones, drawings; placed symbols, wires, labels...). A loss past the same line, or a document deleted, opens the restore
  ask, whatever tool did it.

## Restore to before a turn

- The restore ask (kind input, tool `snapshot_restore`): **Restore to before this turn** or **Keep this turn's changes**. It shows on
  every client, as asks do, and the next turn closes it as kept.
- `/restore` (the thread's last turn) or `/restore <n>` sent as a message. The hub restores, no agent turn starts.

`snapshot::restore` first snapshots the current state (`<n>-before-restore`). It then writes every file of the snapshot, removes the
files that were added since (ignored and huge files are in neither snapshot, so they are never touched), and snapshots the result
(`<n>-restored`). That last snapshot is a new commit on the branch in Backplane's own repository, and on the ref otherwise. Nothing is
lost either way. A running turn is not restored under.

## Setup: Git

On a Windows with no Git yet, adding, making and listing project folders run in Rust (their scripts need Git's sh), so the first
project and its Install Git ask are reachable. The ask closes at the next turn once git runs. The hub's own asks alert with their text,
never their tool id.

Design tools has a Git row: its version, or Install.

- Windows: winget `Git.Git`. Without winget, Git for Windows' pinned installer (`GIT_WIN`: version, address, size and sha256) is
  downloaded and run silently: for this user into `%LOCALAPPDATA%\Programs\Git` from a hub that is not elevated, for all users into
  Program Files from an elevated one (its installer ignores `/CURRENTUSER`). It is never run over a Git installed for all users: the
  press says where that Git is and to update it there (on bp-win, running it over one moved the machine's Git into one user's folder).
- macOS: Homebrew's git, else `xcode-select --install`.
- Linux: the distribution's package through `pkexec` when a desktop can ask for the password, else the command to copy.

Git older than 2.31 counts as missing. Afterwards the hub finds it without a restart. On Windows `tools_install::git_pick_up` puts
Git's `cmd` folder on the hub's PATH, and the shell lookup tries again after a miss. Snapshots need no `user.name`/`user.email`; they
are Backplane's own.

Tests: `agents/tests/snapguard.rs` (#1163's scenario, an existing repository with uncommitted work, an edit then `/restore`, the
mass-deletion ask), `files/snapshot.rs` and `agents/snapguard.rs` units, `present/onboard.rs` and `settings/tools_install.rs` for
setup.
