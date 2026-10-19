# Git

Two modules:
- `src/core/diff.bend` (pure): parses unified diffs.
- `src/server/git.bend` (IO): runs git as a subprocess. Arguments are always an argv list, never a shell line.

Every IO action returns text:
- the result, or
- `""` when there is nothing to report, or
- `error: <git output>`.

`Git.failed(s)` tells whether an answer reports an error. That includes a failed step inside a multi-line stacked action.

## Diffs (`src/core/diff.bend`)

```
DLine{kind: U32, text, old_no: Nat, new_no: Nat}   kind: 0 context, 1 add, 2 del, 3 meta
DHunk{header, lines: List<&2, DLine>}
DFile{old, new, status, hunks: List<&2, DHunk>}   status: modified | added | deleted | renamed | copied
DStats{files, adds, dels}

Diff.parse(text) -> List<&2, DFile>
Diff.stats(fs) -> DStats
Diff.summary(fs) -> String       "3 files, +12 −4"
Diff.binary(f) -> Bool
Diff.lines(text) -> List<&2, String>
```

- Paths drop the `a/` and `b/` prefixes. `old` and `new` both hold a path, even for an added or deleted file; `status` says which case it is.
- Line numbers:
  - Context lines carry both numbers.
  - Added lines have `old_no = 0`.
  - Deleted lines have `new_no = 0`.
- `\ No newline at end of file` becomes a meta line.
- A binary file has a single hunk with an empty header, whose meta line is git's `Binary files ...` line.
- The parser counts the lines each hunk header promises, so a deleted line that starts with `-- ` is not mistaken for a file header.
- Parsing makes one tail-recursive pass over the lines.

## Repository

```
Git.is_repo(cwd) -> IO(Bool)
Git.root(cwd) -> IO(String)          "" outside a repository
Git.branch(cwd) -> IO(String)        "" when HEAD is detached; works on an unborn branch
Git.status(cwd) -> IO(String)        git status --porcelain=v1 --branch -uall
Git.Status.parse(text) -> GStatus{branch, ahead: Nat, behind: Nat, changed: Nat, files: List<&2, String>}
```

## Branches and worktrees

```
Git.branch.sanitize(s) -> String
Git.branch.temp(seed: Nat) -> String                   backplane/<8 hex>
Git.worktree.path(base_dir, repo_name, branch) -> String
Git.worktree.create(repo_cwd, base_dir, repo_name, branch, base_branch) -> IO(String)
Git.branch.rename(cwd, old, new) -> IO(String)
Git.worktree.remove(repo_cwd, path) -> IO(String)
Git.worktree.remove_force(repo_cwd, path) -> IO(String)
```

- `Git.branch.sanitize(s)` gives a lowercase name built from `[a-z0-9._/-]`. Any other character becomes `-`. Each run of separators collapses to one: `/` wins over `-`, and `-` wins over `.`. So `..`, `//`, `/.` and `-/` cannot appear. The name has no leading or trailing separator and is at most 64 characters. The result is `""` when nothing is left.
- `Git.branch.temp(seed)` hashes the seed with murmur3's 32-bit finalizer.
- `Git.worktree.path` answers `<base_dir>/worktrees/<repo_name>/<branch with / as ->`. Threads use `Git.thread_wt` instead (below).
- `Git.worktree.create` runs `git worktree add -b <sanitized branch> <path> [base_branch]` and answers the path. To get a fresh branch, pass `Git.branch.temp(seed)`.
- `Git.branch.rename` sanitizes `new` and answers the new name.
- `Git.worktree.remove` answers `""`. Git refuses to remove a worktree that has changes.
- `Git.worktree.remove_force` removes the worktree even when it has changes.

## Thread worktrees

```
Git.thread_wt(project_dir, carry, branch) -> IO(String)     the thread's directory, "skip: <why>" or "error: ..."
Git.thread_wt.again(project_dir, branch) -> IO(String)      the same, on a kept branch
Git.thread_wt.path(top, branch) -> String                   <top>/.backplane/worktrees/<branch with / as ->
Git.exclude(cwd) -> IO(String)                              /.backplane/ in <common git dir>/info/exclude, once
Git.skipped(s) -> Bool
```

- Every new thread gets a worktree of its project's repository on a fresh `backplane/<8 hex>` branch from upstream: origin's default branch (`origin/main`), fetched first (30 s at most; offline, the last fetched one), with no tracking so a push never goes to main. Without an origin it starts from HEAD (the thread setting `thread.env`: `auto`, the default, or `worktree`; `local` works in the folder). The first message names the branch.
- For a project inside a repository, if its relative path is not a directory in the upstream commit but is a directory in HEAD, the new worktree starts from HEAD instead. For example, a checked-out feature branch may add `mechanical/fixture/` before that folder reaches `origin/main`; a thread for that project then has a working directory at the same relative path. Repository-root projects and folders already present upstream keep the usual upstream base. This exception selects the starting commit; the uncommitted-work rules below still apply.
- The worktree lives in the repository, under `.backplane/worktrees/`, listed in `info/exclude` (not `.gitignore`), so the folder's status, file list and checkpoints never see it. A project that is a folder inside a bigger repository works at the same place in its worktree.
- No repository, or no commit yet: the thread works in the project folder (`WorktreeSkipped`). Until the worktree exists (at most two minutes), the thread's messages wait in its queue.
- A hardware project (KiCad files in the project folder) brings its uncommitted work into the new worktree: staged and unstaged changes, untracked files (not ignored ones, KiCad backups, locks, autosaves or the footprint cache), and deletions. The new worktree gets the files' current contents; the source's index is unchanged. Such a worktree starts from HEAD instead of upstream, since the carried files were made against it. A software project starts clean from upstream.
- The viewer shows the selected thread's worktree, so it shows the board that thread edits.

Removal (`M.Worktree.drop`, laws `worktree_kept_*`, `worktree_dropped_*`):
- A worktree goes once its thread is idle and settled for `worktree.days` (default 3, since it settled), hidden (archived or deleted) that long since its last activity, or settled by a merge (at once, on the next tick).
- Never while a turn runs or something waits on the user, never while the thread is active, pinned or snoozed, and never a directory outside `.backplane/worktrees/`.
- First a checkpoint of the worktree is taken, then `git worktree remove --force`; the branch stays (`WorktreeRemoved`).
- A message to such a thread makes the worktree again from its branch, restores that last checkpoint, and then runs (law `worktree_revives`).

## GitHub accounts (`src/core/gh.bend`, `src/server/gh.bend`)

gh may be logged in to several accounts per host (`gh auth login` more than once). Backplane runs every gh call (listing merged pull requests, `gh pr create`) as the repository's account, and never switches gh's active account.
- The first account tried: the one in the repository's git config `backplane.ghuser` (set it by hand to choose), else the one named like the remote's owner (`origin`, else the first remote), else gh's active one. The other logged-in accounts on that host follow (laws `gh_pick_*`, `gh_order_first`, `gh_without_drops`).
- The next account is tried only when GitHub refuses the call (401/403/404, "Could not resolve to a Repository", "Resource not accessible", ...). Any other failure, such as a pull request that already exists, is the answer. An account that worked after another was refused is saved as `backplane.ghuser`.
- With one account, or none, or a token already in the environment, gh runs as it is. The same goes for a remote that is not on a GitHub host.
- A token never goes on a command line, where any user on the machine could read it. `sh` fetches it with `gh auth token --hostname H --user U` and exports it as `GH_TOKEN` (`GH_ENTERPRISE_TOKEN` for GitHub Enterprise hosts) for that one command. Nothing prompts: `GH_PROMPT_DISABLED`, `GIT_TERMINAL_PROMPT=0`.
- Pushes run as your git is set up. Only when an https push is refused, it runs again through gh's credential helper (`-c credential.helper=!gh auth git-credential`) with each account in turn. SSH remotes are left to your keys.
- Accounts come from the gh config of the user the server runs as (`GH_CONFIG_DIR` is honoured).
- Test: `test/gh_test.bend`, and `test/native/gh_test.bend` (a fake gh with three accounts, or the real one, read-only).

## Merges

- Each minute, a project with threads on the active or pinned shelf that have a branch asks `gh pr list --state merged --json headRefName,url` (one call per project, 30 s timeout). A thread whose branch shows up settles (`ThreadMerged`, kept as the setting `merged.<id>`), with a note linking the pull request.
- A merge settles a thread once (law `merge_once`): brought back, it stays back. Without `gh`, its login or a GitHub remote, nothing happens.

## Checkpoints

```
Git.checkpoint.ref(thread_id, n) -> String
Git.checkpoint.capture(cwd, thread_id, n) -> IO(String)        the commit id
Git.checkpoint.diff(cwd, thread_id, from_n, to_n) -> IO(String) unified diff text
Git.checkpoint.diff_worktree(cwd, thread_id, n) -> IO(String)   checkpoint n vs the files now
Git.checkpoint.revert(cwd, thread_id, n) -> IO(String)          "" when done
Git.b64url(s) -> String                                         base64url, no padding
```

Ref layout:

```
refs/backplane/checkpoints/<b64url(thread_id)>/turn/<n>
```

- Turn 0 is the state before the first turn. Capture it when the thread starts, then capture turn n after each turn.
- Each checkpoint is a commit object:
  - authored by `Backplane <backplane@localhost>` and unsigned;
  - its parent is turn n−1 when that checkpoint exists;
  - it is pointed to only by its hidden ref, so no branch moves and no commit lands on the user's branch.

How a capture works:
- The real index's staged path records seed a fresh private index file, `<git-dir>/backplane-<b64url(thread_id)>.index`, before each capture. Git runs as `env GIT_INDEX_FILE=<idx> git add -A`, which records the current working contents of tracked files (including tracked files that match ignore rules) and non-ignored untracked files. An unborn repository with no index starts with an empty private index. Cached file metadata and optimization flags are omitted so working contents are always read. The real index and its staged contents are never changed.
- `git write-tree` turns that index into a tree.
- `git commit-tree` makes the commit, and `git update-ref` points the hidden ref at it.
- In a worktree, `<git-dir>` is that worktree's private git directory.

`diff_worktree` snapshots the current files the same way and diffs the checkpoint against that tree, so untracked new files show up.

How a revert works:
1. It seeds the thread index from the real index, then syncs it with the current files (`add -A`).
2. It runs `read-tree -u --reset <checkpoint tree>`. Files that differ are restored. Files the checkpoint lacks are deleted if they are tracked, or untracked and not ignored.
- Ignored untracked files stay outside that index and are left alone. Ignored tracked files are restored like other tracked files.
- HEAD, the branch and the real index do not move.

All diffs use `-c core.quotepath=false diff --no-color --no-ext-diff --src-prefix=a/ --dst-prefix=b/ -M`.

## Design steps

```
Git.step.capture(cwd, thread_id) -> IO(String)   the capture's output, "" when nothing changed
Git.refs(cwd, thread_id) -> IO(String)           git dir, top, then "kind\tlabel\tcommit" lines
Git.show(git_dir, "<commit>:<path>") -> IO(Maybe<String>)
Git.version(path) -> IO(List<U32>)               a live file, or a file in a commit (core/hist.bend's paths)
```

Ref layout: `refs/backplane/checkpoints/<b64url(thread_id)>/steps`, one ref whose history is the thread's steps.

- After each tool call, and when a turn starts and ends, the hub asks for a step (job `step`, its `a` the tool call or message it follows).
- One `sh` script stages the thread's `.kicad_sch` and `.kicad_pcb` files (not KiCad's autosaves or backups; tracked ones even when ignored) through the thread's own index (`<git-dir>/backplane-<b64>.steps.index`), and when that tree differs from the last step's, commits it with the last step as parent. `update-ref` checks the old value (a new ref is made with `update-ref --stdin`'s `create`), so two captures cannot fork the chain; a lock directory holding the owner's pid makes them take turns, and a dead owner's lock is cleared.
- A folder outside git keeps its steps in `<folder>/.backplane/history.git` (its `info/exclude` lists `/.backplane/`), made only when the folder has KiCad files, never for `/` or the home folder.
- The output is the commit, the step before (`""` for the first), the git dir, the top, then git's `-z` name-status of the files changed. The server logs it as an Act of tone `step` (core/hist.bend's `Hist.record`).
- A capture runs beside the agent, not before it: two quick edits may share a step.

`history.refs` answers what a file may be compared with: the git dir and top as the capture finds them, `HEAD`, the 40 newest branches, the 40 newest commits that touched KiCad files, the thread's turn checkpoints, and the project's board and schematic.

## Stacked actions

```
Git.act(cwd, action, message, title, body) -> IO(String)
```

`action` is one of `commit`, `push`, `create_pr`, `commit_push` or `commit_push_pr`. The answer has one line per step. The first failing step ends the chain with `error: <step>: ...`.

- **commit**: runs `git add -A`, then `git commit -m <message>`. The default message is "Update from Backplane". The step answers `commit: <short sha> <subject>`, or `commit: nothing to commit`. Nothing to commit is not a failure, so a later push still runs.
- **push**: runs `git push` when the branch has an upstream. Otherwise it runs `git push -u origin <branch>`. It answers `push: <branch>[ -> origin/<branch>]`. A detached HEAD is an error.
- **create_pr**: runs `gh pr create --title T --body B --head <branch>`, or `--fill` when the title is empty. It answers `pr: <url>`, taken from the last line of gh's output.
