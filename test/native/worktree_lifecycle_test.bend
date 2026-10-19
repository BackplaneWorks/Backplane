import Base
import ./checkpoint_ignored_test.bend as T
import ../../src/server/proc.bend as P
import ../../src/server/git.bend as G
import ../../src/server/host.bend as H

# The interaction between project subfolders, staged carry, checkpoint
# tracking and retirement. Exact bytes include NUL, Unicode and no newline.
# Individual components retain their existing broader regression suites.

def checks(+root: String) -> IO(Bool):
  +repo = root ++ "/source"
  +project = repo ++ "/feature/hardware"
  do IO<Bool>:
    a : Bool <- T.shell(root, "isolated lifecycle fixture initializes", "mkdir source\ncd source\ngit init -q -b main\ngit config user.email lifecycle@example.invalid\ngit config user.name Lifecycle\ngit config commit.gpgsign false\nprintf '*.log\\n' > .gitignore\ngit add -A\ngit commit -qm base\ngit init -q --bare -b main ../remote.git\ngit remote add origin ../remote.git\ngit push -q -u origin main\ngit switch -qc feature\nmkdir -p feature/hardware\nprintf base > feature/hardware/board.kicad_pcb\nprintf old > feature/hardware/tracked.log\ngit add -A\ngit add -f feature/hardware/tracked.log\ngit commit -qm feature\nprintf 'staged\\000board' > feature/hardware/board.kicad_pcb\nprintf 'staged\\000new' > feature/hardware/new.log\ngit add feature/hardware/board.kicad_pcb\ngit add -f feature/hardware/new.log\nprintf 'working\\000board 日本語' > feature/hardware/board.kicad_pcb\nprintf 'working\\000new 日本語' > feature/hardware/new.log\nprintf 'working\\000tracked' > feature/hardware/tracked.log\nprintf 'untracked\\000part' > feature/hardware/new.kicad_sym\nprintf 'ignored cache' > feature/hardware/cache.log\ncp .git/index ../source.index")
    +wt : String <- G.Git.thread_wt(project, True{}, "backplane/lifecycle")
    b : Bool <- T.chk("feature subfolder worktree creates with staged carry", Bool.show(G.Git.failed(wt)), "False")
    c : Bool <- T.shell(root, "carry preserves all working bytes and source index", "dest=source/.backplane/worktrees/backplane-lifecycle/feature/hardware\nfor f in board.kicad_pcb tracked.log new.log new.kicad_sym; do cmp \"source/feature/hardware/$f\" \"$dest/$f\"; done\ncmp source/.git/index source.index\ntest ! -e \"$dest/cache.log\"\ntest -z \"$(git -C \"$dest\" diff --cached --name-only)\"")
    +snap : String <- G.Git.checkpoint.capture(wt, "lifecycle", 0n)
    d : Bool <- T.chk("carried feature checkpoint succeeds", Nat.show(String.length(snap)), "40")
    e : Bool <- T.shell(root, "checkpoint retains exact staged ignored addition", "git -C source show refs/backplane/checkpoints/bGlmZWN5Y2xl/turn/0:feature/hardware/new.log > captured\ncmp captured source/feature/hardware/new.log")
    +child : String <- G.Git.thread_wt(wt, True{}, "backplane/lifecycle-child")
    childok : Bool <- T.chk("carried intent-to-add can be carried again", Bool.show(G.Git.failed(child)), "False")
    +childsnap : String <- G.Git.checkpoint.capture(child, "lifecycle-child", 0n)
    childcapture : Bool <- T.chk("second carry checkpoint succeeds", Nat.show(String.length(childsnap)), "40")
    childbytes : Bool <- T.shell(root, "second carry retains ignored intent-to-add bytes", "git -C source show refs/backplane/checkpoints/bGlmZWN5Y2xlLWNoaWxk/turn/0:feature/hardware/new.log > child-captured\ncmp child-captured source/feature/hardware/new.log\ncmp source/.git/index source.index")
    childroot : String <- G.Git.root(child)
    childremoved : String <- G.Git.worktree.remove_force(project, childroot)
    childgone : Bool <- T.chk("second worktree retires independently", childremoved, "")
    +top : String <- G.Git.root(wt)
    rootok : Bool <- T.chk("worktree API resolves repository root", top, repo ++ "/.backplane/worktrees/backplane-lifecycle")
    removed : String <- G.Git.worktree.remove_force(project, top)
    f : Bool <- T.chk("retirement removes worktree after checkpoint", removed, "")
    +revived : String <- G.Git.thread_wt.again(project, "backplane/lifecycle")
    g : Bool <- T.chk("revival returns the same project subfolder", revived, wt)
    restored : String <- G.Git.checkpoint.revert(revived, "lifecycle", 0n)
    h : Bool <- T.chk("revival restores checkpoint", restored, "")
    i : Bool <- T.shell(root, "retirement and revival preserve exact bytes and source staging", "dest=source/.backplane/worktrees/backplane-lifecycle/feature/hardware\nfor f in board.kicad_pcb tracked.log new.log new.kicad_sym; do cmp \"source/feature/hardware/$f\" \"$dest/$f\"; done\ncmp source/.git/index source.index\ntest ! -e \"$dest/cache.log\"\ntest \"$(git -C source branch --show-current)\" = feature")
    bad : String <- G.Git.carry(repo, root ++ "/missing")
    j : Bool <- T.chk("copy failure is a visible error", Bool.show(G.Git.failed(bad)), "True")
    k : Bool <- T.shell(root, "failed carry preserves source index and bytes", "cmp source/.git/index source.index\ncmp captured source/feature/hardware/new.log")
    return T.all([a, b, c, d, e, f, g, h, i, j, k, rootok, childok, childcapture, childbytes, childgone])

def run(+root: String) -> IO(Unit):
  do IO<Unit>:
    dirs : P.Ran <- P.Proc.run("mkdir", ["-p", root ++ "/user", root ++ "/empty-templates"], "/tmp")
    H.Env.put("HOME", root ++ "/user")
    H.Env.put("BACKPLANE_HOME", root ++ "/backplane")
    H.Env.put("GIT_CONFIG_NOSYSTEM", "1")
    H.Env.put("GIT_CONFIG_GLOBAL", "/dev/null")
    H.Env.put("GIT_CONFIG_COUNT", "0")
    H.Env.put("GIT_CONFIG_PARAMETERS", "")
    H.Env.put("GIT_TEMPLATE_DIR", root ++ "/empty-templates")
    H.Env.put("GIT_TERMINAL_PROMPT", "0")
    frame : List<&2, U32> <- H.Json.cbor_frame("null", "", "")
    linked : Bool <- T.chk("host effect linkage frame", Nat.show(List.length(&2, U32, frame)), "3")
    ok : Bool <- checks(root)
    removed : P.Ran <- P.Proc.run("rm", ["-rf", root], "/tmp")
    T.finish(Bool.and(linked, ok))

def physical(+root: String, r: P.Ran) -> IO(Unit):
  match r:
    case P.Ran{0, path}: run(String.trim(path))
    case P.Ran{_, error}:
      do IO<Unit>:
        removed : P.Ran <- P.Proc.run("rm", ["-rf", root], "/tmp")
        IO.die(Unit, 1, "cannot resolve physical test directory: " ++ error)

def created(r: P.Ran) -> IO(Unit):
  match r:
    case P.Ran{0, root}:
      +dir = String.trim(root)
      # Exercise a real symlink even on Linux. Git returns physical roots;
      # Darwin's /tmp also aliases /private/tmp. Keep every exact assertion.
      IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", "set -e\nln -s \"$1\" \"$1/fixture-root\"\ncd \"$1/fixture-root\"\npwd -P", "physical-root", dir], "/tmp"), physical(dir))
    case P.Ran{_, error}: IO.die(Unit, 1, "cannot create test directory: " ++ error)

def main() -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("mktemp", ["-d", "/tmp/bp-worktree-lifecycle.XXXXXX"], "/tmp"), created)
