import Base
import ../check.bend as C
import ../../src/server/proc.bend as P
import ../../src/server/git.bend as G
import ../../src/server/host.bend as H

# Actual checkpoint and worktree APIs against isolated temporary repositories.
# Run with scripts/build-app.sh, then the emitted native executable.

def out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, text}: String.trim(text)

def code(r: P.Ran) -> String:
  match r:
    case P.Ran{c, _}: U32.show(c)

def sh(cwd: String, script: String) -> IO(P.Ran):
  P.Proc.run("sh", ["-c", "set -eu\n" ++ script], cwd)

def git(cwd: String, args: List<&2, String>) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("git", args, cwd), r => IO.pure(String, out(r)))

def chk(name: String, +got: String, +want: String) -> IO(Bool):
  do IO<Bool>:
    IO.print(C.Check.pick(String.eq(got, want), name, got, want))
    return String.eq(got, want)

def shell(cwd: String, name: String, script: String) -> IO(Bool):
  IO.bind(P.Ran, Bool, sh(cwd, script), +r => chk(name, code(r) ++ out(r), "0"))

def all(bs: List<&2, Bool>) -> Bool:
  match bs:
    case Nil{}: True{}
    case b <> t: Bool.and(b, all(t))

def finish(ok: Bool) -> IO(Unit):
  match ok:
    case True{}: IO.print("all ok")
    case False{}: IO.die(Unit, 1, "checkpoint_ignored_test failed")

def Cases.capture(+root: String) -> IO(Bool):
  +repo = root ++ "/capture"
  do IO<Bool>:
    b0 : Bool <- shell(repo, "capture fixture initializes", "git init -q -b main .\ngit config user.email checkpoint@example.invalid\ngit config user.name Checkpoint\ngit config commit.gpgsign false\nprintf '*.log\\n' > .gitignore\nprintf 'v1\\n' > board.log\nprintf 'gone\\n' > removed.log\nprintf 'ordinary old\\n' > ordinary.txt\ngit add -A\ngit add -f board.log removed.log\ngit commit -qm initial\nprintf 'staged\\n' > board.log\ngit add board.log\nprintf 'v2\\n' > board.log\ngit rm -q removed.log\nprintf 'ordinary new\\n' > ordinary.txt\nprintf 'new file\\n' > new.txt\nprintf 'ignored cache\\n' > cache.log\ncp .git/index ../capture.index")
    +snap : String <- G.Git.checkpoint.capture(repo, "capture", 0n)
    a : Bool <- chk("capture returns a commit", Nat.show(String.length(snap)), "40")
    b3 : Bool <- shell(repo, "tracked ignored working contents captured", "test \"$(git show refs/backplane/checkpoints/Y2FwdHVyZQ/turn/0:board.log)\" = v2")
    b4 : Bool <- shell(repo, "tracked deletion stays absent", "! git cat-file -e refs/backplane/checkpoints/Y2FwdHVyZQ/turn/0:removed.log 2>/dev/null")
    b5 : Bool <- shell(repo, "nonignored working changes captured", "test \"$(git show refs/backplane/checkpoints/Y2FwdHVyZQ/turn/0:ordinary.txt)\" = 'ordinary new'\ntest \"$(git show refs/backplane/checkpoints/Y2FwdHVyZQ/turn/0:new.txt)\" = 'new file'")
    b6 : Bool <- shell(repo, "ignored untracked file excluded", "! git cat-file -e refs/backplane/checkpoints/Y2FwdHVyZQ/turn/0:cache.log 2>/dev/null")
    b7 : Bool <- shell(repo, "capture preserves real index bytes", "cmp .git/index ../capture.index")
    b8 : Bool <- shell(repo, "stage later ignored file", "printf 'later staged\\n' > later.log\ngit add -f later.log\nprintf 'later working\\n' > later.log\ncp .git/index ../capture.index")
    again : String <- G.Git.snapshot(repo, "capture")
    b : Bool <- chk("repeated snapshot succeeds", Bool.show(G.Git.is_error(again)), "False")
    b11 : Bool <- shell(repo, "reused private index learns current tracking", "tree=$(git rev-parse --git-path backplane-Y2FwdHVyZQ.index)\ntest \"$(GIT_INDEX_FILE=\"$PWD/$tree\" git show :later.log)\" = 'later working'\ncmp .git/index ../capture.index")
    diff : String <- G.Git.checkpoint.diff_worktree(repo, "capture", 0n)
    c : Bool <- chk("working diff contains newly tracked ignored path", Bool.show(String.contains(diff, "later.log")), "True")
    b14 : Bool <- shell(repo, "prepare fresh private revert index", "rm -f .git/backplane-Y2FwdHVyZQ.index\nprintf 'v3\\n' > board.log\nprintf 'later ignored\\n' > cache.log\nprintf 'extra\\n' > extra.txt\ncp .git/index ../capture.index")
    revert : String <- G.Git.checkpoint.revert(repo, "capture", 0n)
    d : Bool <- chk("fresh-index revert succeeds", revert, "")
    b17 : Bool <- shell(repo, "fresh-index revert restores ignored tracked contents", "test \"$(cat board.log)\" = v2")
    b18 : Bool <- shell(repo, "revert removes tracked ignored path absent at checkpoint", "test ! -e later.log")
    b19 : Bool <- shell(repo, "revert leaves ignored untracked contents", "test \"$(cat cache.log)\" = 'later ignored'")
    b20 : Bool <- shell(repo, "revert removes ordinary file added since checkpoint", "test ! -e extra.txt")
    b21 : Bool <- shell(repo, "revert preserves staged deletion and real index bytes", "test ! -e removed.log\ncmp .git/index ../capture.index\ntest \"$(git show :board.log)\" = staged\ntest \"$(git show HEAD:board.log)\" = v1")
    return all([a, b, c, d, b0, b3, b4, b5, b6, b7, b8, b11, b14, b17, b18, b19, b20, b21])

def Cases.unborn(+root: String) -> IO(Bool):
  +repo = root ++ "/unborn"
  do IO<Bool>:
    b0 : Bool <- shell(repo, "unborn fixture initializes without a real index", "git init -q -b main .\nprintf '*.log\\n' > .gitignore\nprintf 'ordinary\\n' > ordinary.txt\nprintf 'cache\\n' > ignored.log\ntest ! -e .git/index")
    +snap : String <- G.Git.checkpoint.capture(repo, "unborn", 0n)
    a : Bool <- chk("unborn checkpoint has a commit", Nat.show(String.length(snap)), "40")
    b3 : Bool <- shell(repo, "unborn checkpoint captures files without creating real index", "test \"$(git show refs/backplane/checkpoints/dW5ib3Ju/turn/0:ordinary.txt)\" = ordinary\n! git cat-file -e refs/backplane/checkpoints/dW5ib3Ju/turn/0:ignored.log 2>/dev/null\ntest ! -e .git/index\n! git rev-parse --verify HEAD >/dev/null 2>&1")
    b4 : Bool <- shell(repo, "unborn repository can stage ignored file after first capture", "printf 'staged\\n' > tracked.log\ngit add -f tracked.log\nprintf 'working\\n' > tracked.log\ncp .git/index ../unborn.index")
    again : String <- G.Git.checkpoint.capture(repo, "unborn", 1n)
    b : Bool <- chk("second unborn checkpoint succeeds", Bool.show(G.Git.is_error(again)), "False")
    b7 : Bool <- shell(repo, "unborn later tracking uses current working contents", "test \"$(git show refs/backplane/checkpoints/dW5ib3Ju/turn/1:tracked.log)\" = working\ncmp .git/index ../unborn.index")
    b8 : Bool <- shell(repo, "remove real index but retain old private index", "rm .git/index\nrm tracked.log\nprintf 'untracked cache\\n' > tracked.log")
    last : String <- G.Git.checkpoint.capture(repo, "unborn", 2n)
    c : Bool <- chk("snapshot after removing real index succeeds", Bool.show(G.Git.is_error(last)), "False")
    b11 : Bool <- shell(repo, "missing real index discards stale private tracking", "! git cat-file -e refs/backplane/checkpoints/dW5ib3Ju/turn/2:tracked.log 2>/dev/null\ntest ! -e .git/index")
    return all([a, b, c, b0, b3, b4, b7, b8, b11])

def Cases.split(+root: String) -> IO(Bool):
  +repo = root ++ "/split"
  do IO<Bool>:
    b0 : Bool <- shell(repo, "split-index fixture initializes", "git init -q -b main .\ngit config user.email checkpoint@example.invalid\ngit config user.name Checkpoint\ngit config commit.gpgsign false\nprintf '*.log\\n' > .gitignore\nprintf 'original\\n' > tracked.log\ngit add .gitignore\ngit add -f tracked.log\ngit commit -qm initial\ngit update-index --split-index\nprintf 'working split\\n' > tracked.log\ncp .git/index ../split.index")
    +snap : String <- G.Git.checkpoint.capture(repo, "split", 0n)
    a : Bool <- chk("split-index snapshot succeeds", Bool.show(G.Git.is_error(snap)), "False")
    b3 : Bool <- shell(repo, "split-index references work in private index", "test \"$(git show refs/backplane/checkpoints/c3BsaXQ/turn/0:tracked.log)\" = 'working split'\ncmp .git/index ../split.index\ntest -n \"$(git rev-parse --shared-index-path)\"")
    revert : String <- G.Git.checkpoint.revert(repo, "split", 0n)
    b : Bool <- chk("split-index revert succeeds", revert, "")
    b6 : Bool <- shell(repo, "split-index revert keeps real index bytes", "cmp .git/index ../split.index")
    return all([a, b, b0, b3, b6])

def Cases.flags(+root: String) -> IO(Bool):
  +repo = root ++ "/flags"
  do IO<Bool>:
    a : Bool <- shell(repo, "optimization flags fixture initializes", "git init -q -b main .\ngit config user.email checkpoint@example.invalid\ngit config user.name Checkpoint\ngit config commit.gpgsign false\nmkdir sub\nprintf 'v1' > ordinary.txt\nprintf 'v1' > sub/skipped.txt\nprintf 'v1' > 'line\nbreak.txt'\ngit add -A\ngit commit -qm initial\ngit update-index --assume-unchanged ordinary.txt 'line\nbreak.txt'\ngit update-index --skip-worktree sub/skipped.txt\nprintf 'v2' > ordinary.txt\nprintf 'v2' > sub/skipped.txt\nprintf 'v2' > 'line\nbreak.txt'\ncp .git/index ../flags.index")
    snap : String <- G.Git.checkpoint.capture(repo ++ "/sub", "flags", 0n)
    b : Bool <- chk("subfolder flagged snapshot succeeds", Bool.show(G.Git.failed(snap)), "False")
    c : Bool <- shell(repo, "private index refreshes flags throughout repository", "test \"$(git show refs/backplane/checkpoints/ZmxhZ3M/turn/0:ordinary.txt)\" = v2\ntest \"$(git show refs/backplane/checkpoints/ZmxhZ3M/turn/0:sub/skipped.txt)\" = v2\ntest \"$(git show 'refs/backplane/checkpoints/ZmxhZ3M/turn/0:line\nbreak.txt')\" = v2")
    d : Bool <- shell(repo, "snapshot leaves real flags and index bytes unchanged", "cmp .git/index ../flags.index\ngit ls-files -v ordinary.txt | grep -q '^h ' \ngit ls-files -v sub/skipped.txt | grep -q '^S '")
    return all([a, b, c, d])

def Cases.lifecycle(+root: String) -> IO(Bool):
  +repo = root ++ "/lifecycle"
  do IO<Bool>:
    a0 : Bool <- shell(repo, "revival fixture initializes", "git init -q -b main .\ngit config user.email checkpoint@example.invalid\ngit config user.name Checkpoint\ngit config commit.gpgsign false\nprintf '*.log\\n' > .gitignore\nprintf 'v1\\n' > board.log\ngit add .gitignore\ngit add -f board.log\ngit commit -qm initial")
    +wt : String <- G.Git.thread_wt(repo, False{}, "backplane/checkpoint")
    a1 : Bool <- chk("thread worktree created", Bool.show(G.Git.failed(wt)), "False")
    a2 : Bool <- shell(wt, "edit ignored tracked board in thread", "printf 'v2\\n' > board.log\ncp \"$(git rev-parse --git-path index)\" \"$PWD/../../../../lifecycle.index\"")
    snap : String <- G.Git.checkpoint.capture(wt, "revive", 0n)
    a3 : Bool <- chk("thread checkpoint created", Bool.show(G.Git.failed(snap)), "False")
    a4 : Bool <- shell(wt, "worktree checkpoint preserves real index bytes", "cmp \"$(git rev-parse --git-path index)\" \"$PWD/../../../../lifecycle.index\"")
    removed : String <- G.Git.worktree.remove_force(repo, wt)
    a5 : Bool <- chk("modified thread worktree removed", removed, "")
    +revived : String <- G.Git.thread_wt.again(repo, "backplane/checkpoint")
    a6 : Bool <- chk("thread worktree revived", revived, wt)
    restored : String <- G.Git.checkpoint.revert(revived, "revive", 0n)
    a7 : Bool <- chk("removed thread checkpoint restored", restored, "")
    a8 : Bool <- shell(revived, "revival recovers v2 instead of silently reverting to v1", "test \"$(cat board.log)\" = v2\ntest \"$(git show HEAD:board.log)\" = v1\ntest \"$(git show :board.log)\" = v1")
    return all([a0, a1, a2, a3, a4, a5, a6, a7, a8])

def run(+root: String) -> IO(Unit):
  do IO<Unit>:
    dirs : P.Ran <- P.Proc.run("mkdir", ["-p", root ++ "/user", root ++ "/empty-templates", root ++ "/capture", root ++ "/unborn", root ++ "/split", root ++ "/lifecycle", root ++ "/flags"], "/tmp")
    H.Env.put("HOME", root ++ "/user")
    H.Env.put("BACKPLANE_HOME", root ++ "/backplane")
    H.Env.put("GIT_CONFIG_NOSYSTEM", "1")
    H.Env.put("GIT_CONFIG_GLOBAL", "/dev/null")
    H.Env.put("GIT_CONFIG_COUNT", "0")
    H.Env.put("GIT_CONFIG_PARAMETERS", "")
    H.Env.put("GIT_TEMPLATE_DIR", root ++ "/empty-templates")
    H.Env.put("GIT_TERMINAL_PROMPT", "0")
    # Keep host.c's existing CBOR helper linked in this standalone fixture.
    frame : List<&2, U32> <- H.Json.cbor_frame("null", "", "")
    linked : Bool <- chk("host effect linkage frame", Nat.show(List.length(&2, U32, frame)), "3")
    a : Bool <- Cases.capture(root)
    b : Bool <- Cases.unborn(root)
    c : Bool <- Cases.split(root)
    d : Bool <- Cases.lifecycle(root)
    e : Bool <- Cases.flags(root)
    removed : P.Ran <- P.Proc.run("rm", ["-rf", root], "/tmp")
    finish(all([linked, a, b, c, d, e]))

def created(r: P.Ran) -> IO(Unit):
  match r:
    case P.Ran{0, root}: run(String.trim(root))
    case P.Ran{_, error}: IO.die(Unit, 1, "cannot create test directory: " ++ error)

def main() -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("mktemp", ["-d", "/tmp/bp-checkpoint-ignored.XXXXXX"], "/tmp"), created)
