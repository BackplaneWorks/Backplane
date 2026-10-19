import Base
import ./proc.bend as P
import ./gh.bend as GH
import ../core/sha1.bend as S
import ../core/utf8.bend as U
import ../core/diff.bend as D
import ../core/hist.bend as HI
import ./kicad.bend as K

# Git, run as a subprocess with an argv list (never a shell line). Every
# action answers text: the result, "" when there is nothing to say, or
# "error: <git's output>" so the hub can show it. docs/git.md has the API and
# the checkpoint ref layout.

# Running git
# -----------

def Git.run(cwd: String, args: List<&2, String>) -> IO(P.Ran):
  P.Proc.run("git", args, cwd)

# git with a private index file (GIT_INDEX_FILE), through env(1)
def Git.run.idx(cwd: String, idx: String, args: List<&2, String>) -> IO(P.Ran):
  P.Proc.run("env", ("GIT_INDEX_FILE=" ++ idx) <> "git" <> args, cwd)

def Git.is_error(+s: String) -> Bool:
  String.starts_with(s, "error: ")

def Git.then.k(ok: Bool, raw: Bool, out: String, k: String -> IO(String)) -> IO(String):
  match ok raw:
    case True{} True{}:
      k(out)
    case True{} False{}:
      k(String.trim(out))
    case False{} _:
      IO.pure(String, "error: " ++ String.trim(out))

def Git.then(raw: Bool, r: P.Ran, k: String -> IO(String)) -> IO(String):
  match r:
    case P.Ran{code, out}:
      Git.then.k(U32.is_eq(code, 0), raw, out, k)

# run, then continue with the trimmed output; a non-zero exit ends the chain
# with "error: <output>"
def Git.seq(m: IO(P.Ran), k: String -> IO(String)) -> IO(String):
  IO.bind(P.Ran, String, m, r => Git.then(False{}, r, k))

# the same with the output untouched (diffs)
def Git.seq.raw(m: IO(P.Ran), k: String -> IO(String)) -> IO(String):
  IO.bind(P.Ran, String, m, r => Git.then(True{}, r, k))

def Git.and.k(bad: Bool, +s: String, k: String -> IO(String)) -> IO(String):
  match bad:
    case True{}:
      IO.pure(String, s)
    case False{}:
      k(s)

# continue after a step that answers text, unless it answered an error
def Git.and(m: IO(String), k: String -> IO(String)) -> IO(String):
  IO.bind(String, String, m, +s => Git.and.k(Git.is_error(s), s, k))

def Git.done(s: String) -> IO(String):
  IO.pure(String, s)

def Git.code(r: P.Ran) -> U32:
  match r:
    case P.Ran{code, _}:
      code

def Git.text(r: P.Ran) -> String:
  match r:
    case P.Ran{_, out}:
      String.trim(out)

# Repository
# ----------

def Git.is_repo.k(r: P.Ran) -> Bool:
  match r:
    case P.Ran{code, out}:
      Bool.and(U32.is_eq(code, 0), String.eq(String.trim(out), "true"))

def Git.is_repo(cwd: String) -> IO(Bool):
  IO.bind(P.Ran, Bool, Git.run(cwd, ["rev-parse", "--is-inside-work-tree"]), r => IO.pure(Bool, Git.is_repo.k(r)))

def Git.or_empty(+s: String) -> String:
  Bool.pick(String, Git.is_error(s), "", s)

# the top of the work tree, or "" outside a repository
def Git.root(cwd: String) -> IO(String):
  IO.bind(String, String, Git.seq(Git.run(cwd, ["rev-parse", "--show-toplevel"]), Git.done), s => IO.pure(String, Git.or_empty(s)))

# the current branch (also on an unborn one), or "" when detached
def Git.branch(cwd: String) -> IO(String):
  IO.bind(String, String, Git.seq(Git.run(cwd, ["symbolic-ref", "--short", "-q", "HEAD"]), Git.done), s => IO.pure(String, Git.or_empty(s)))

# `git status --porcelain=v1 --branch -uall` text, or "error: ..."
def Git.status(cwd: String) -> IO(String):
  Git.seq.raw(Git.run(cwd, ["-c", "core.quotepath=false", "status", "--porcelain=v1", "--branch", "-uall"]), Git.done)

# Status parsing
# --------------

type GStatus is Data:
  GStatus{branch: String, ahead: Nat, behind: Nat, changed: Nat, files: List<&2, String>}

# the text after the first `key` in s, or "" when absent
def Git.after.go(s: String, hit: Bool, +key: String) -> String:
  match s hit:
    case SNil{} _:
      ""
    case +x True{}:
      String.drop(x, String.length(key))
    case SCon{_, +t} False{}:
      Git.after.go(t, String.starts_with(t, key), key)

def Git.after(+s: String, +key: String) -> String:
  Git.after.go(s, String.starts_with(s, key), key)

def Git.digit_stop(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{c, _}:
      Bool.not(Char.is_digit(c))

def Git.digits.take.go(s: String, stop: Bool, acc: String) -> String:
  match s stop:
    case _ True{}:
      String.reverse(acc)
    case SNil{} False{}:
      String.reverse(acc)
    case SCon{c, +t} False{}:
      Git.digits.take.go(t, Git.digit_stop(t), SCon{c, acc})

def Git.digits.take(+s: String, acc: String) -> String:
  Git.digits.take.go(s, Git.digit_stop(s), acc)

# a leading run of digits as a Nat (0 when none)
def Git.lead_num(s: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(Git.digits.take(s, SNil{})), 0n)

def Git.name_stop(+s: String) -> Bool:
  Bool.or(String.starts_with(s, "..."), String.starts_with(s, " "))

def Git.name.go(s: String, stop: Bool, acc: String) -> String:
  match s stop:
    case _ True{}:
      String.reverse(acc)
    case SNil{} False{}:
      String.reverse(acc)
    case SCon{c, +t} False{}:
      Git.name.go(t, Git.name_stop(t), SCon{c, acc})

# "main...origin/main [ahead 1]" -> "main"
def Git.name(+s: String) -> String:
  Git.name.go(s, Git.name_stop(s), "")

def Git.head.branch(+h: String) -> String:
  Bool.pick(String, Bool.or(String.starts_with(h, "No commits yet on "), String.starts_with(h, "Initial commit on ")), String.drop(h, 18n), Git.name(h))

def Git.head.num(+h: String, +key: String) -> Nat:
  Bool.pick(Nat, String.contains(h, key), Git.lead_num(Git.after(h, key)), 0n)

type SAcc is Data:
  SAcc{branch: String, ahead: Nat, behind: Nat, changed: Nat, files: List<&2, String>}

def Git.status.head(+h: String, acc: SAcc) -> SAcc:
  match acc:
    case SAcc{_, _, _, c, fs}:
      SAcc{Git.head.branch(h), Git.head.num(h, "ahead "), Git.head.num(h, "behind "), c, fs}

def Git.status.file(l: String, acc: SAcc) -> SAcc:
  match acc:
    case SAcc{b, a, d, c, fs}:
      SAcc{b, a, d, 1n+c, String.drop(l, 3n) <> fs}

def Git.status.line.k(hd: Bool, empty: Bool, +l: String, acc: SAcc) -> SAcc:
  match hd empty:
    case True{} _:
      Git.status.head(String.drop(l, 3n), acc)
    case False{} True{}:
      acc
    case False{} False{}:
      Git.status.file(l, acc)

def Git.status.line(+l: String, acc: SAcc) -> SAcc:
  Git.status.line.k(String.starts_with(l, "## "), String.is_empty(String.trim(l)), l, acc)

def Git.status.go(ls: List<&2, String>, acc: SAcc) -> SAcc:
  match ls:
    case Nil{}:
      acc
    case l <> t:
      Git.status.go(t, Git.status.line(l, acc))

def Git.status.fin(acc: SAcc) -> GStatus:
  match acc:
    case SAcc{b, a, d, c, fs}:
      GStatus{b, a, d, c, List.reverse(&2, String, fs)}

# lines of a text (tail-recursive)
def Git.lines(s: String) -> List<&2, String>:
  D.Diff.lines(s)

# `git status --porcelain=v1 --branch` text as a record; files keep the
# porcelain path text ("old -> new" for a rename)
def Git.Status.parse(text: String) -> GStatus:
  Git.status.fin(Git.status.go(Git.lines(text), SAcc{"", 0n, 0n, 0n, Nil{}}))

# Branch names
# ------------

# 0 a kept character, 1 '.', 2 '-' (and anything not allowed), 3 '/'
def Git.cls.k(keep: Bool, dot: Bool, slash: Bool) -> U32:
  match keep dot slash:
    case True{} _ _:
      0
    case False{} True{} _:
      1
    case False{} False{} True{}:
      3
    case False{} False{} False{}:
      2

def Git.cls(+c: Char) -> U32:
  Git.cls.k(Bool.or(Bool.or(Char.is_digit(c), Bool.and(U32.is_ge(Char.to_u32(c), 97), U32.is_le(Char.to_u32(c), 122))), Char.is_eq(c, '_')), Char.is_eq(c, '.'), Char.is_eq(c, '/'))

def Git.sep_chr(+k: U32) -> Char:
  Chr{Bool.pick(U32, U32.is_eq(k, 3), 47, Bool.pick(U32, U32.is_eq(k, 2), 45, 46))}

# the reversed output, the strongest separator waiting, whether anything was kept
type SZ is Data:
  SZ{out: String, pend: U32, started: Bool}

def Git.sz.emit(started: Bool, +pend: U32, +out: String) -> String:
  match started:
    case True{}:
      Bool.pick(String, U32.is_eq(pend, 0), out, SCon{Git.sep_chr(pend), out})
    case False{}:
      out

def Git.sz.put.k(keep: Bool, +k: U32, c: Char, out: String, +pend: U32, started: Bool) -> SZ:
  match keep:
    case True{}:
      SZ{SCon{c, Git.sz.emit(started, pend, out)}, 0, True{}}
    case False{}:
      SZ{out, U32.max(pend, k), started}

def Git.sz.put(+k: U32, +c: Char, st: SZ) -> SZ:
  match st:
    case SZ{out, +pend, started}:
      Git.sz.put.k(U32.is_eq(k, 0), k, c, out, pend, started)

def Git.sz.go(s: String, st: SZ) -> SZ:
  match s:
    case SNil{}:
      st
    case SCon{+c, t}:
      Git.sz.go(t, Git.sz.put(Git.cls(Char.to_lower(c)), Char.to_lower(c), st))

def Git.sz.fin(st: SZ) -> String:
  match st:
    case SZ{out, _, _}:
      String.reverse(out)

def Git.is_sep(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{c, _}:
      U32.is_ne(Git.cls(c), 0)

def Git.drop_seps.go(s: String, sep: Bool) -> String:
  match s sep:
    case SCon{_, +t} True{}:
      Git.drop_seps.go(t, Git.is_sep(t))
    case x _:
      x

def Git.drop_seps(+s: String) -> String:
  Git.drop_seps.go(s, Git.is_sep(s))

def Git.trim_end_seps(s: String) -> String:
  String.reverse(Git.drop_seps(String.reverse(s)))

# a safe branch name: lowercase [a-z0-9._/-], each run of separators becomes
# one ('/' over '-' over '.'), none leading or trailing, at most 64 chars;
# "" when nothing is left
def Git.branch.sanitize(s: String) -> String:
  Git.trim_end_seps(String.take(Git.sz.fin(Git.sz.go(s, SZ{SNil{}, 0, False{}})), 64n))

def Git.hex.chr(+n: U32) -> Char:
  Chr{Bool.pick(U32, U32.is_lt(n, 10), (n + 48 : U32), (n + 87 : U32))}

def Git.hex.go(n: Nat, +x: U32, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+p:
      Git.hex.go(p, U32.shrn(x, 4n), SCon{Git.hex.chr(U32.and(x, 15)), acc})

def Git.mix.c(+h: U32) -> U32:
  U32.xor(h, U32.shrn(h, 16n))

def Git.mix.b(+h: U32) -> U32:
  Git.mix.c(U32.mul(U32.xor(h, U32.shrn(h, 13n)), 3266489909))

# murmur3's 32-bit finalizer
def Git.mix(+h: U32) -> U32:
  Git.mix.b(U32.mul(U32.xor(h, U32.shrn(h, 16n)), 2246822507))

# 8 hex digits from a seed
def Git.hex8(seed: Nat) -> String:
  Git.hex.go(8n, Git.mix(U32.from_nat(Nat.mod(seed, Nat.mul(65536n, 65536n)))), "")

# a temporary branch name: backplane/<8 hex>
def Git.branch.temp(seed: Nat) -> String:
  "backplane/" ++ Git.hex8(seed)

def Git.slash_dash.go(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{+c, t}:
      Git.slash_dash.go(t, SCon{Bool.pick(Char, Char.is_eq(c, '/'), '-', c), acc})

# <base_dir>/worktrees/<repo_name>/<branch with '/' as '-'>
def Git.worktree.path(base_dir: String, repo_name: String, branch: String) -> String:
  base_dir ++ "/worktrees/" ++ repo_name ++ "/" ++ Git.slash_dash.go(branch, "")

# k(v), or msg when bad
def Git.guard(bad: Bool, msg: String, v: String, k: String -> IO(String)) -> IO(String):
  match bad:
    case True{}:
      IO.pure(String, msg)
    case False{}:
      k(v)

def Git.named(+b: String, k: String -> IO(String)) -> IO(String):
  Git.guard(String.is_empty(b), "error: empty branch name", b, k)

def Git.branch.rename.k(cwd: String, old: String, +b: String) -> IO(String):
  Git.named(b, _ => Git.seq(Git.run(cwd, ["branch", "-m", old, b]), _ => Git.done(b)))

# rename a branch; answers the new (sanitized) name
def Git.branch.rename(cwd: String, old: String, new: String) -> IO(String):
  Git.branch.rename.k(cwd, old, Git.branch.sanitize(new))

# Worktrees
# ---------

def Git.base_args(+base: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(base), Nil{}, [base])

def Git.worktree.create.k(+cwd: String, +path: String, +b: String, base: String) -> IO(String):
  Git.named(b, _ => Git.seq(Git.run(cwd, List.append(&2, String, ["worktree", "add", "-b", b, path], Git.base_args(base))), _ => Git.done(path)))

def Git.worktree.create.b(repo_cwd: String, base_dir: String, repo_name: String, +b: String, base_branch: String) -> IO(String):
  Git.worktree.create.k(repo_cwd, Git.worktree.path(base_dir, repo_name, b), b, base_branch)

# a new worktree on a new branch from base_branch ("" = HEAD); answers its
# path. branch is sanitized; pass Git.branch.temp(seed) for a fresh one.
def Git.worktree.create(repo_cwd: String, base_dir: String, repo_name: String, branch: String, base_branch: String) -> IO(String):
  Git.worktree.create.b(repo_cwd, base_dir, repo_name, Git.branch.sanitize(branch), base_branch)

# remove a clean worktree (git refuses one with changes); "" when done
def Git.worktree.remove(repo_cwd: String, path: String) -> IO(String):
  Git.seq(Git.run(repo_cwd, ["worktree", "remove", path]), _ => Git.done(""))

# remove a worktree even with changes (they are lost)
def Git.worktree.remove_force(repo_cwd: String, path: String) -> IO(String):
  Git.seq(Git.run(repo_cwd, ["worktree", "remove", "--force", path]), _ => Git.done(""))

# Thread worktrees
# ----------------
# A new thread gets a worktree inside its repository, at
# <top>/.backplane/worktrees/<branch with / as ->. The repository's
# info/exclude lists /.backplane/, so neither git status nor a checkpoint of
# the main folder ever sees them. The thread works at the project's place
# in the worktree (a project may be a folder inside a bigger repository).

def Git.thread_wt.path(+top: String, branch: String) -> String:
  top ++ "/.backplane/worktrees/" ++ Git.slash_dash.go(branch, "")

def Git.thread_wt.sub(+prefix: String) -> String:
  Bool.pick(String, String.ends_with(prefix, "/"), String.take(prefix, Nat.sub(String.length(prefix), 1n)), prefix)

# where the thread works: the worktree, plus the project's place in the
# repository (git rev-parse --show-prefix of the project folder)
def Git.thread_wt.cwd(+path: String, +prefix: String) -> String:
  Bool.pick(String, String.is_empty(prefix), path, path ++ "/" ++ Git.thread_wt.sub(prefix))

def Git.exclude.sh() -> String:
  "f=\"$1/info/exclude\"; mkdir -p \"$1/info\" && { grep -qxF '/.backplane/' \"$f\" 2>/dev/null || printf '\\n# Backplane thread worktrees\\n/.backplane/\\n' >> \"$f\"; }"

# list /.backplane/ in the repository's info/exclude (once)
def Git.exclude(+cwd: String) -> IO(String):
  Git.seq(Git.run(cwd, ["rev-parse", "--path-format=absolute", "--git-common-dir"]),
    dir => Git.seq(P.Proc.run("sh", ["-c", Git.exclude.sh(), "sh", dir], cwd), _ => Git.done("")))

# Copy the folder's uncommitted work (changed and untracked files, not
# ignored ones) into a new worktree, and delete there what is deleted here.
# Compare tracked paths with HEAD so staged edits count too. Disabling
# rename detection carries a rename as deleting its old path and copying
# its new one; the destination's index stays at the base commit.
# KiCad's own droppings (backups, locks, autosaves, the footprint cache)
# stay behind.
def Git.carry.sh() -> String:
  "cd \"$1\" || exit 1; { git diff --no-renames --name-only -z --diff-filter=d HEAD --; git ls-files -z -o --exclude-standard; } | grep -zv -e '-backups/' -e '\\.lck$' -e 'fp-info-cache$' -e '_autosave-' | tar -c --null -T - -f - 2>/dev/null | tar -x -f - -C \"$2\" 2>/dev/null; git diff --no-renames --name-only -z --diff-filter=D HEAD -- | xargs -0 -r sh -c 'd=$1; shift; for p do [ -e \"$p\" ] || [ -L \"$p\" ] || rm -f -- \"$d/$p\"; done' sh \"$2\"; exit 0"

def Git.carry(+top: String, +path: String) -> IO(String):
  Git.seq(P.Proc.run("sh", ["-c", Git.carry.sh(), "sh", top, path], top), _ => Git.done(""))

def Git.thread_wt.carry(carry: Bool, +top: String, +path: String, +cwd: String) -> IO(String):
  match carry:
    case True{}:
      Git.and(Git.carry(top, path), _ => Git.done(cwd))
    case False{}:
      Git.done(cwd)

# Where a new thread starts: origin's default branch (origin/main), fetched
# first (30 s at most; offline, the last fetched one), so a thread never
# starts behind upstream. "" (HEAD) when there is no origin, or when the
# folder's uncommitted work is carried over (it belongs on the HEAD it was
# made against; copied onto a newer tree it would undo upstream's edits).
def Git.upstream.sh() -> String:
  "[ \"$1\" = 1 ] && { git ls-files -m -o -d --exclude-standard; git diff --cached --name-only --no-renames --; } | grep -v -e '-backups/' -e '\\.lck$' -e 'fp-info-cache$' -e '_autosave-' | grep -q . && exit 0; git remote get-url origin >/dev/null 2>&1 || exit 0; b=$(git symbolic-ref -q --short refs/remotes/origin/HEAD); b=${b:-origin/main}; t=; command -v timeout >/dev/null 2>&1 && t='timeout 30'; GIT_TERMINAL_PROMPT=0 $t git fetch -q origin \"${b#origin/}\" </dev/null >/dev/null 2>&1; git rev-parse -q --verify \"refs/remotes/$b\" >/dev/null && printf %s \"$b\"; exit 0"

def Git.upstream.of(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      String.trim(out)
    case _:
      ""

def Git.upstream(+carry: Bool, +top: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", Git.upstream.sh(), "sh", Bool.pick(String, carry, "1", "0")], top), r => IO.pure(String, Git.upstream.of(r)))

# A subdirectory project may only exist on the checked-out branch. Keep
# upstream when it contains the directory, else use HEAD when it does.
# A blob at that path is not a directory. Empty base already means HEAD
# (no origin, or uncommitted hardware work that will be carried over).
def Git.thread_wt.tree(r: P.Ran) -> Bool:
  match r:
    case P.Ran{code, out}:
      Bool.and(U32.is_eq(code, 0), String.eq(String.trim(out), "tree"))

def Git.thread_wt.base.head(found: Bool, +base: String, +prefix: String) -> IO(String):
  match found:
    case True{}:
      Git.done("")
    case False{}:
      Git.done("error: project directory " ++ Git.thread_wt.sub(prefix) ++ " is not a directory in " ++ base ++ " or HEAD")

def Git.thread_wt.base.upstream(found: Bool, +top: String, +base: String, +prefix: String) -> IO(String):
  match found:
    case True{}:
      Git.done(base)
    case False{}:
      IO.bind(P.Ran, String, Git.run(top, ["cat-file", "-t", "HEAD:" ++ Git.thread_wt.sub(prefix)]),
        r => Git.thread_wt.base.head(Git.thread_wt.tree(r), base, prefix))

def Git.thread_wt.base.check(needed: Bool, +top: String, +base: String, +prefix: String) -> IO(String):
  match needed:
    case True{}:
      IO.bind(P.Ran, String, Git.run(top, ["cat-file", "-t", base ++ ":" ++ Git.thread_wt.sub(prefix)]),
        r => Git.thread_wt.base.upstream(Git.thread_wt.tree(r), top, base, prefix))
    case False{}:
      Git.done(base)

def Git.thread_wt.base(+top: String, +base: String, +prefix: String) -> IO(String):
  Git.thread_wt.base.check(Bool.and(Bool.not(String.is_empty(base)), Bool.not(String.is_empty(prefix))), top, base, prefix)

# --no-track: the thread's branch is its own; a push never goes to main
def Git.thread_wt.add(+base: String, +carry: Bool, +top: String, +prefix: String, +b: String) -> IO(String):
  Git.seq(Git.run(top, List.append(&2, String, ["worktree", "add", "--no-track", "-b", b, Git.thread_wt.path(top, b)], Git.base_args(base))),
    _ => Git.thread_wt.carry(carry, top, Git.thread_wt.path(top, b), Git.thread_wt.cwd(Git.thread_wt.path(top, b), prefix)))

def Git.thread_wt.made(+carry: Bool, +top: String, +prefix: String, +b: String) -> IO(String):
  Git.and(Git.exclude(top), _ =>
    IO.bind(String, String, Git.upstream(carry, top), base =>
      Git.and(Git.thread_wt.base(top, base, prefix), chosen => Git.thread_wt.add(chosen, carry, top, prefix, b))))

def Git.thread_wt.head(r: P.Ran, +carry: Bool, +top: String, +prefix: String, +b: String) -> IO(String):
  match r:
    case P.Ran{0, _}:
      Git.thread_wt.made(carry, top, prefix, b)
    case _:
      IO.pure(String, "skip: no commit yet")

def Git.thread_wt.top(+top: String, +cwd: String, +carry: Bool, +b: String) -> IO(String):
  Bool.pick(IO(String), String.is_empty(top), IO.pure(String, "skip: not a git repository"),
    Git.and(Git.seq.raw(Git.run(cwd, ["rev-parse", "--show-prefix"]), p => Git.done(String.trim(p))),
      +prefix => IO.bind(P.Ran, String, Git.run(cwd, ["rev-parse", "--verify", "-q", "HEAD"]), r => Git.thread_wt.head(r, carry, top, prefix, b))))

# a new thread's worktree on a new branch from upstream (Git.upstream),
# or HEAD when only HEAD contains the project's subdirectory, for the project folder
# cwd; carry: bring the folder's uncommitted work along. Answers the
# thread's directory, "skip: <why>" when there is no repository to branch
# (the thread then works in the folder), or "error: ...".
def Git.thread_wt(+cwd: String, +carry: Bool, branch: String) -> IO(String):
  IO.bind(String, String, Git.root(cwd), top => Git.thread_wt.top(top, cwd, carry, Git.branch.sanitize(branch)))

def Git.thread_wt.again.top(+top: String, +cwd: String, +b: String) -> IO(String):
  Bool.pick(IO(String), String.is_empty(top), IO.pure(String, "skip: not a git repository"),
    Git.and(Git.seq.raw(Git.run(cwd, ["rev-parse", "--show-prefix"]), p => Git.done(String.trim(p))),
      +prefix => Git.and(Git.exclude(top), _ =>
        IO.bind(P.Ran, String, Git.run(top, ["worktree", "prune"]),
          _ => Git.seq(Git.run(top, ["worktree", "add", Git.thread_wt.path(top, b), b]), _ => Git.done(Git.thread_wt.cwd(Git.thread_wt.path(top, b), prefix)))))))

# a removed thread worktree made again on its kept branch; answers like
# Git.thread_wt
def Git.thread_wt.again(+cwd: String, +branch: String) -> IO(String):
  IO.bind(String, String, Git.root(cwd), top => Git.thread_wt.again.top(top, cwd, branch))

def Git.skipped(+s: String) -> Bool:
  String.starts_with(s, "skip: ")

def Git.b64url.map(+c: Char) -> U32:
  Bool.pick(U32, Char.is_eq(c, '+'), 45, Bool.pick(U32, Char.is_eq(c, '/'), 95, Bool.pick(U32, Char.is_eq(c, '='), 0, Char.to_u32(c))))

def Git.b64url.put(+u: U32, +acc: String) -> String:
  Bool.pick(String, U32.is_eq(u, 0), acc, SCon{Chr{u}, acc})

# Checkpoints
# -----------

def Git.b64url.go(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{+c, t}:
      Git.b64url.go(t, Git.b64url.put(Git.b64url.map(c), acc))

# base64url (RFC 4648 section 5) of the UTF-8 bytes, without padding
def Git.b64url(s: String) -> String:
  Git.b64url.go(S.B64.encode(U.Utf8.encode(s)), "")

# refs/backplane/checkpoints/<b64url(thread_id)>/turn/<n>
def Git.checkpoint.ref(thread_id: String, n: Nat) -> String:
  "refs/backplane/checkpoints/" ++ Git.b64url(thread_id) ++ "/turn/" ++ Nat.show(n)

def Git.checkpoint.prev(thread_id: String, n: Nat) -> String:
  match n:
    case 0n:
      "refs/backplane/none"
    case 1n+p:
      Git.checkpoint.ref(thread_id, p)

def Git.index_path(gd: String, thread_id: String) -> String:
  gd ++ "/backplane-" ++ Git.b64url(thread_id) ++ ".index"

# Seed tracked paths and staged blobs from the real index every time.
# A fresh private index omits cached stat data and optimization flags: its
# add -A must read today's working files, including ignored tracked paths.
# Git reads split indexes itself; the source index is never rewritten.
def Git.index.seed.sh() -> String:
  String.join([
    "set -eu",
    "src=$(git rev-parse --path-format=absolute --git-path index)",
    "if [ ! -f \"$src\" ]; then rm -f \"$1\"; exit 0; fi",
    "paths=$(mktemp \"$1.paths.XXXXXX\")",
    "trap 'rm -f \"$paths\"' 0",
    "git ls-files --full-name --stage -z -- :/ > \"$paths\"",
    "export GIT_INDEX_FILE=\"$1\"",
    "rm -f \"$1\"",
    "git read-tree --empty",
    "git update-index -z --index-info < \"$paths\""
  ], "\n")

def Git.index.seed(cwd: String, idx: String) -> IO(P.Ran):
  P.Proc.run("sh", ["-c", Git.index.seed.sh(), "sh", idx], cwd)

# the whole work tree (tracked, including ignored tracked files, and
# untracked non-ignored files) as a tree object; answers the tree id
# without changing the real index or its staged contents.
def Git.snapshot.k(+cwd: String, +idx: String) -> IO(String):
  Git.seq(Git.index.seed(cwd, idx), _ =>
    Git.seq(Git.run.idx(cwd, idx, ["add", "-A"]), _ =>
      Git.seq(Git.run.idx(cwd, idx, ["write-tree"]), Git.done)))

def Git.snapshot(+cwd: String, thread_id: String) -> IO(String):
  Git.seq(Git.run(cwd, ["rev-parse", "--absolute-git-dir"]), gd => Git.snapshot.k(cwd, Git.index_path(gd, thread_id)))

def Git.parents(r: P.Ran) -> List<&2, String>:
  match r:
    case P.Ran{+code, out}:
      Bool.pick(List<&2, String>, U32.is_eq(code, 0), ["-p", String.trim(out)], Nil{})

def Git.commit_tree(cwd: String, tree: String, parents: List<&2, String>, msg: String) -> IO(P.Ran):
  P.Proc.run("env", List.append(&2, String,
    ["GIT_AUTHOR_NAME=Backplane", "GIT_AUTHOR_EMAIL=backplane@localhost",
     "GIT_COMMITTER_NAME=Backplane", "GIT_COMMITTER_EMAIL=backplane@localhost",
     "git", "commit-tree", "--no-gpg-sign", "-m", msg, tree], parents), cwd)

def Git.capture.k(+cwd: String, +r: String, +tree: String, prev: P.Ran, msg: String) -> IO(String):
  Git.seq(Git.commit_tree(cwd, tree, Git.parents(prev), msg), +sha =>
    Git.seq(Git.run(cwd, ["update-ref", r, sha]), _ => Git.done(sha)))

# capture the work tree as turn n of a thread (n = 0 at thread start, before
# the first turn); no commit lands on any branch. Answers the commit id.
def Git.checkpoint.capture(+cwd: String, +thread_id: String, +n: Nat) -> IO(String):
  Git.and(Git.snapshot(cwd, thread_id), tree =>
    IO.bind(P.Ran, String, Git.run(cwd, ["rev-parse", "--verify", "-q", Git.checkpoint.prev(thread_id, n)]), prev =>
      Git.capture.k(cwd, Git.checkpoint.ref(thread_id, n), tree, prev, "backplane checkpoint " ++ thread_id ++ " turn " ++ Nat.show(n))))

def Git.diff.args(a: String, b: String) -> List<&2, String>:
  ["-c", "core.quotepath=false", "diff", "--no-color", "--no-ext-diff", "--src-prefix=a/", "--dst-prefix=b/", "-M", a, b, "--"]

# unified diff text from checkpoint a to checkpoint b ("" when equal)
def Git.checkpoint.diff(cwd: String, +thread_id: String, +from_n: Nat, +to_n: Nat) -> IO(String):
  Git.seq.raw(Git.run(cwd, Git.diff.args(Git.checkpoint.ref(thread_id, from_n), Git.checkpoint.ref(thread_id, to_n))), Git.done)

# unified diff text from checkpoint n to the files as they are now
def Git.checkpoint.diff_worktree(+cwd: String, +thread_id: String, n: Nat) -> IO(String):
  Git.and(Git.snapshot(cwd, thread_id), tree =>
    Git.seq.raw(Git.run(cwd, Git.diff.args(Git.checkpoint.ref(thread_id, n), tree)), Git.done))

def Git.revert.k(+cwd: String, +idx: String, r: String) -> IO(String):
  Git.seq(Git.run(cwd, ["rev-parse", "--verify", r ++ "^{tree}"]), tree =>
    Git.seq(Git.index.seed(cwd, idx), _ =>
      Git.seq(Git.run.idx(cwd, idx, ["add", "-A"]), _ =>
        Git.seq(Git.run.idx(cwd, idx, ["read-tree", "-u", "--reset", tree]), _ => Git.done("")))))

# put the work tree back exactly as checkpoint n had it: changed files are
# restored, files it did not have are removed; ignored untracked files stay.
# HEAD, the branch and the real index do not move. "" when done.
def Git.checkpoint.revert(+cwd: String, +thread_id: String, n: Nat) -> IO(String):
  Git.seq(Git.run(cwd, ["rev-parse", "--absolute-git-dir"]), gd =>
    Git.revert.k(cwd, Git.index_path(gd, thread_id), Git.checkpoint.ref(thread_id, n)))

# Steps
# -----
# The KiCad files after each tool call, for the viewer's history
# (core/hist.bend). One sh script a capture: the thread's .kicad_sch and
# .kicad_pcb files (KiCad's autosaves and backups left out) staged through
# its own index, and when that tree differs from the last step's, a commit
# on refs/backplane/checkpoints/<b64url(thread)>/steps whose parent is the
# last step (update-ref checks the old value, so two captures at once
# cannot fork the chain; a lock directory makes them take turns). A
# folder outside git keeps its steps in <folder>/.backplane/history.git.
# A capture whose owner died leaves its lock (it holds the owner's pid) and
# the index's own lock for the next to clear. KiCad files the repository
# tracks are staged even when ignored. The ref is created with
# update-ref --stdin and the empty tree hashed, so a SHA-256 repository
# works too. It prints the commit, the one before ("" for the first), the
# git dir and the top of the work tree a line each, then git's -z
# name-status of the files changed (Hist.record); nothing when nothing
# changed. A repository with no KiCad files (and no steps yet) is left
# at once, so software threads pay one quick look. A capture runs after
# the tool call is reported, while the agent goes on: two quick edits may
# land in one step, and a turn's first may race the agent's first edit.

def Git.step.script() -> String:
  String.join([
    "exec 2>/dev/null",
    "cd \"$1\" || exit 0",
    "name=$2",
    "if top=$(git rev-parse --show-toplevel) && [ -n \"$top\" ]; then",
    "  gd=$(cd \"$(git rev-parse --git-common-dir)\" && pwd) || exit 0",
    "  pd=$(git rev-parse --absolute-git-dir) || exit 0",
    "  shadow=",
    "else",
    "  top=$(pwd); gd=$top/.backplane/history.git; pd=$gd; shadow=1",
    "  [ \"$top\" = / ] || [ \"$top\" = \"$HOME\" ] && exit 0",
    "  [ -d \"$gd\" ] || [ -n \"$(find . -maxdepth 4 \\( -name '*.kicad_pcb' -o -name '*.kicad_sch' \\) -print | head -n 1)\" ] || exit 0",
    "  export GIT_DIR=\"$gd\" GIT_WORK_TREE=\"$top\"",
    "  if [ ! -d \"$gd\" ]; then",
    "    mkdir -p \"$top/.backplane\" && git init -q >/dev/null || exit 0",
    "    printf '/.backplane/\\n' >> \"$gd/info/exclude\"",
    "  fi",
    "fi",
    "ref=refs/backplane/checkpoints/$name/steps",
    "spec='*.kicad_[ps]c[bh]'",
    "[ -n \"$shadow\" ] || git rev-parse -q --verify \"$ref\" >/dev/null || [ -n \"$(git ls-files -- \"$spec\" | head -n 1)\" ] || [ -n \"$(find . -maxdepth 4 \\( -name '*.kicad_pcb' -o -name '*.kicad_sch' \\) -not -path './.backplane/*' -print | head -n 1)\" ] || exit 0",
    "idx=$pd/backplane-$name.steps.index",
    "lock=$pd/backplane-$name.steps.lock",
    "n=0",
    "until mkdir \"$lock\"; do",
    "  o=$(cat \"$lock/pid\")",
    "  if [ -n \"$o\" ] && ! kill -0 \"$o\"; then rm -rf \"$lock\"; continue; fi",
    "  n=$((n+1)); [ $n -gt 600 ] && exit 0; sleep 0.1",
    "done",
    "echo $$ > \"$lock/pid\"",
    "trap '[ \"$(cat \"$lock/pid\")\" = $$ ] && rm -rf \"$lock\"' EXIT",
    "rm -f \"$idx.lock\"",
    "[ -n \"$shadow\" ] || git ls-files -z -- \"$spec\" | GIT_INDEX_FILE=$idx xargs -0 git add -f -- >/dev/null",
    "GIT_INDEX_FILE=$idx git add -A -- \"$spec\" ':(exclude)*_autosave-*' ':(exclude)*-backups/*' >/dev/null",
    "tree=$(GIT_INDEX_FILE=$idx git write-tree) || exit 0",
    "prev=$(git rev-parse -q --verify \"$ref\")",
    "if [ -n \"$prev\" ]; then",
    "  [ \"$(git rev-parse \"$prev^{tree}\")\" = \"$tree\" ] && exit 0",
    "else",
    "  [ \"$tree\" = \"$(git hash-object -t tree /dev/null)\" ] && exit 0",
    "fi",
    "c=$(GIT_AUTHOR_NAME=Backplane GIT_AUTHOR_EMAIL=backplane@localhost GIT_COMMITTER_NAME=Backplane GIT_COMMITTER_EMAIL=backplane@localhost git commit-tree --no-gpg-sign -m \"backplane step\" ${prev:+-p \"$prev\"} \"$tree\") || exit 0",
    "if [ -n \"$prev\" ]; then git update-ref \"$ref\" \"$c\" \"$prev\" || exit 0",
    "else printf 'create %s %s\\n' \"$ref\" \"$c\" | git update-ref --stdin || exit 0; fi",
    "printf '%s\\n%s\\n%s\\n%s\\n' \"$c\" \"$prev\" \"$gd\" \"$top\"",
    "git diff-tree -r -z --no-commit-id --name-status --root ${prev:+\"$prev\"} \"$c\""], "\n")

def Git.step.ran(r: P.Ran) -> IO(String):
  match r:
    case P.Ran{0, out}:
      IO.pure(String, out)
    case _:
      IO.pure(String, "")

# a step of the thread's KiCad files in cwd: the script's output, or ""
def Git.step.capture(+cwd: String, +thread_id: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", Git.step.script(), "sh", cwd, Git.b64url(thread_id)], "/"), Git.step.ran)

# What a file may be compared with: the git dir and top (as Git.step
# finds them), then lines "kind\tlabel\tcommit": HEAD, the newest
# branches, commits that touched KiCad files, the thread's turns
def Git.refs.script() -> String:
  String.join([
    "exec 2>/dev/null",
    "cd \"$1\" || exit 0",
    "name=$2",
    "if top=$(git rev-parse --show-toplevel 2>/dev/null) && [ -n \"$top\" ]; then",
    "  gd=$(cd \"$(git rev-parse --git-common-dir)\" && pwd) || exit 0",
    "  printf '%s\\n%s\\n' \"$gd\" \"$top\"",
    "  h=$(git rev-parse -q --verify HEAD) && printf 'head\\tHEAD\\t%s\\n' \"$h\"",
    "  for b in origin/HEAD origin/main origin/master main master; do",
    "    m=$(git merge-base HEAD \"$b\" 2>/dev/null) || continue",
    "    [ \"$m\" != \"$h\" ] && printf 'base\\t%s\\t%s\\n' \"$(git rev-parse --abbrev-ref \"$b\" 2>/dev/null || echo \"$b\")\" \"$m\"",
    "    break",
    "  done",
    "  git for-each-ref --sort=-committerdate --count=40 --format='branch%09%(refname:short)%09%(objectname)' refs/heads refs/remotes",
    "  git for-each-ref --sort=-creatordate --count=40 --format='tag%09%(refname:short)%09%(if)%(*objectname)%(then)%(*objectname)%(else)%(objectname)%(end)' refs/tags",
    "  git log -n 40 --format='commit%x09%h %s%x09%H' -- '*.kicad_[ps]c[bh]'",
    "  git for-each-ref --sort=-committerdate --format='turn%09%(refname)%09%(objectname)' \"refs/backplane/checkpoints/$name/turn/\"",
    "else",
    "  top=$(pwd)",
    "  printf '%s\\n%s\\n' \"$top/.backplane/history.git\" \"$top\"",
    "fi"], "\n")

def Git.refs(+cwd: String, +thread_id: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", Git.refs.script(), "sh", cwd, Git.b64url(thread_id)], "/"), Git.step.ran)

def Git.show.ran(r: P.Ran) -> IO(Maybe<&2, String>):
  match r:
    case P.Ran{0, out}:
      IO.pure(Maybe<&2, String>, Some{out})
    case _:
      IO.pure(Maybe<&2, String>, None{})

# a file as a commit has it (a version path, core/hist.bend); None when
# the commit or the file is not there
def Git.show(+g: String, +obj: String) -> IO(Maybe<&2, String>):
  IO.bind(P.Ran, Maybe<&2, String>, P.Proc.run("sh", ["-c", "exec 2>/dev/null; git --git-dir=\"$1\" show \"$2\"", "sh", g, obj], "/"), Git.show.ran)

def Git.version.got(m: Maybe<&2, String>) -> IO(List<&2, U32>):
  match m:
    case Some{t}:
      IO.pure(List<&2, U32>, U.Utf8.encode(t))
    case None{}:
      IO.pure(List<&2, U32>, Nil{})

def Git.version.k(k: Nat, +path: String) -> IO(List<&2, U32>):
  match k:
    case 1n:
      IO.bind(Maybe<&2, String>, List<&2, U32>, Git.show(HI.Hist.git.dir(path), HI.Hist.git.obj(path)), Git.version.got)
    case _:
      K.Io.bytes(path)

# the bytes of a version the viewer names (core/hist.bend): a live file,
# or a file in a commit; [] when it cannot be read
def Git.version(+path: String) -> IO(List<&2, U32>):
  Git.version.k(HI.Hist.kind(path), path)

# Stacked actions
# ---------------

type AStep is Data:
  ACommit{}
  APush{}
  APr{}

def Git.act.steps(+a: String) -> Maybe<&2, List<&2, AStep>>:
  Bool.pick(Maybe<&2, List<&2, AStep>>, String.eq(a, "commit"), Some{[ACommit{}]},
  Bool.pick(Maybe<&2, List<&2, AStep>>, String.eq(a, "push"), Some{[APush{}]},
  Bool.pick(Maybe<&2, List<&2, AStep>>, String.eq(a, "create_pr"), Some{[APr{}]},
  Bool.pick(Maybe<&2, List<&2, AStep>>, String.eq(a, "commit_push"), Some{[ACommit{}, APush{}]},
  Bool.pick(Maybe<&2, List<&2, AStep>>, String.eq(a, "commit_push_pr"), Some{[ACommit{}, APush{}, APr{}]}, None{})))))

# "error: x" -> "error: <step>: x"
def Git.label.k(bad: Bool, name: String, +s: String) -> String:
  match bad:
    case True{}:
      "error: " ++ name ++ ": " ++ String.drop(s, 7n)
    case False{}:
      s

def Git.label(name: String, m: IO(String)) -> IO(String):
  IO.bind(String, String, m, +s => IO.pure(String, Git.label.k(Git.is_error(s), name, s)))

def Git.commit.go(clean: Bool, +cwd: String, msg: String) -> IO(String):
  match clean:
    case True{}:
      Git.done("commit: nothing to commit")
    case False{}:
      Git.seq(Git.run(cwd, ["commit", "-m", msg]), _ =>
        Git.seq(Git.run(cwd, ["log", "-1", "--format=%h %s"]), s => Git.done("commit: " ++ s)))

def Git.commit.k(+cwd: String, r: P.Ran, msg: String) -> IO(String):
  match r:
    case P.Ran{code, _}:
      Git.commit.go(U32.is_eq(code, 0), cwd, msg)

def Git.message(+m: String) -> String:
  Bool.pick(String, String.is_empty(String.trim(m)), "Update from Backplane", m)

def Git.step.commit(+cwd: String, msg: String) -> IO(String):
  Git.label("commit", Git.seq(Git.run(cwd, ["add", "-A"]), _ =>
    IO.bind(P.Ran, String, Git.run(cwd, ["diff", "--cached", "--quiet"]), r => Git.commit.k(cwd, r, Git.message(msg)))))

def Git.push.go(has_up: Bool, +cwd: String, +b: String) -> IO(String):
  match has_up:
    case True{}:
      Git.seq(GH.GhRun.push(cwd, ["push"]), _ => Git.done("push: " ++ b))
    case False{}:
      Git.seq(GH.GhRun.push(cwd, ["push", "-u", "origin", b]), _ => Git.done("push: " ++ b ++ " -> origin/" ++ b))

def Git.push.k(+cwd: String, +b: String, up: P.Ran) -> IO(String):
  match up:
    case P.Ran{code, _}:
      Git.push.go(U32.is_eq(code, 0), cwd, b)

def Git.need_branch(+b: String, k: String -> IO(String)) -> IO(String):
  Git.guard(String.is_empty(b), "error: detached HEAD", b, k)

def Git.step.push(+cwd: String) -> IO(String):
  Git.label("push", IO.bind(String, String, Git.branch(cwd), b =>
    Git.need_branch(b, b2 =>
      IO.bind(P.Ran, String, Git.run(cwd, ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]), up => Git.push.k(cwd, b2, up)))))

def Git.last_line.go(ls: List<&2, String>, last: String) -> String:
  match ls:
    case Nil{}:
      last
    case +l <> t:
      Git.last_line.go(t, Bool.pick(String, String.is_empty(String.trim(l)), last, String.trim(l)))

# the last non-blank line of a text
def Git.last_line(s: String) -> String:
  Git.last_line.go(Git.lines(s), "")

def Git.pr.args(+b: String, +title: String, body: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(String.trim(title)),
    ["pr", "create", "--fill", "--head", b],
    ["pr", "create", "--title", title, "--body", body, "--head", b])

def Git.step.pr(+cwd: String, title: String, body: String) -> IO(String):
  Git.label("pr", IO.bind(String, String, Git.branch(cwd), b =>
    Git.need_branch(b, b2 =>
      Git.seq(GH.GhRun.gh(cwd, Git.pr.args(b2, title, body)), out => Git.done("pr: " ++ Git.last_line(out))))))

def Git.act.step(s: AStep, +cwd: String, +message: String, +title: String, +body: String) -> IO(String):
  match s:
    case ACommit{}:
      Git.step.commit(cwd, message)
    case APush{}:
      Git.step.push(cwd)
    case APr{}:
      Git.step.pr(cwd, title, body)

def Git.join(+log: String, +line: String) -> String:
  Bool.pick(String, String.is_empty(log), line, log ++ "\n" ++ line)

def Git.act.next(+line: String, log: String, k: String -> IO(String)) -> IO(String):
  Git.and.k(Git.is_error(line), Git.join(log, line), k)

def Git.act.go(steps: List<&2, AStep>, +cwd: String, +message: String, +title: String, +body: String, log: String) -> IO(String):
  match steps:
    case Nil{}:
      Git.done(log)
    case s <> t:
      IO.bind(String, String, Git.act.step(s, cwd, message, title, body), line =>
        Git.act.next(line, log, l2 => Git.act.go(t, cwd, message, title, body, l2)))

def Git.act.k(m: Maybe<&2, List<&2, AStep>>, +action: String, cwd: String, message: String, title: String, body: String) -> IO(String):
  match m:
    case None{}:
      Git.done("error: unknown action " ++ action)
    case Some{steps}:
      Git.act.go(steps, cwd, message, title, body, "")

# a stacked action: commit | push | create_pr | commit_push | commit_push_pr.
# One line per step ("commit: <sha> <subject>", "push: <branch>",
# "pr: <url>"); the first failing step ends it with "error: <step>: ...".
def Git.act(cwd: String, +action: String, message: String, title: String, body: String) -> IO(String):
  Git.act.k(Git.act.steps(action), action, cwd, message, title, body)

# whether an answer (from any action here) reports a failure
def Git.failed(+s: String) -> Bool:
  Bool.or(Git.is_error(s), String.contains(s, "\nerror: "))
