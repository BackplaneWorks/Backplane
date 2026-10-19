import Base
import ../check.bend as C
import ../../src/server/proc.bend as P
import ../../src/server/host.bend as H
import ../../src/server/git.bend as G
import ../../src/core/diff.bend as D

# Git against a scratch repository under /tmp: checkpoints (capture, diff,
# diff against the work tree, revert), worktrees (create, rename, remove),
# stacked actions against a local bare remote, and the pure helpers.
#   bend test/native/git_test.bend -o build/git_test && build/git_test

def sh(cwd: String, script: String) -> IO(P.Ran):
  P.Proc.run("sh", ["-c", script], cwd)

def out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, o}:
      String.trim(o)

def code(r: P.Ran) -> String:
  match r:
    case P.Ran{c, _}:
      U32.show(c)

def chk(name: String, +got: String, +want: String) -> IO(Bool):
  do IO<Bool>:
    IO.print(C.Check.pick(String.eq(got, want), name, got, want))
    return String.eq(got, want)

def cat(path: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("cat", [path], ""), r => IO.pure(String, out(r)))

def exists(path: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("test", ["-e", path], ""), r => IO.pure(String, Bool.show(String.eq(code(r), "0"))))

def git(cwd: String, args: List<&2, String>) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("git", args, cwd), r => IO.pure(String, out(r)))

def status.show(s: G.GStatus) -> String:
  match s:
    case G.GStatus{b, a, d, c, fs}:
      b ++ " +" ++ Nat.show(a) ++ " -" ++ Nat.show(d) ++ " " ++ Nat.show(c) ++ " " ++ String.join(fs, ",")

def summary(+text: String) -> String:
  Bool.pick(String, String.starts_with(text, "error: "), text, D.Diff.summary(D.Diff.parse(text)))

def file.show(f: D.DFile) -> String:
  match f:
    case D.DFile{_, n, s, _}:
      s ++ ":" ++ n

def files.go(fs: List<&2, D.DFile>, acc: List<&2, String>) -> List<&2, String>:
  match fs:
    case Nil{}:
      List.reverse(&2, String, acc)
    case f <> t:
      files.go(t, file.show(f) <> acc)

def files(text: String) -> String:
  String.join(files.go(D.Diff.parse(text), Nil{}), " ")

def len(s: String) -> String:
  Nat.show(String.length(s))

def all(bs: List<&2, Bool>) -> Bool:
  match bs:
    case Nil{}:
      True{}
    case b <> t:
      Bool.and(b, all(t))

def finish(ok: Bool) -> IO(Unit):
  match ok:
    case True{}:
      IO.print("all ok")
    case False{}:
      IO.die(Unit, 1, "git_test failed")

def setup() -> String:
  "git init -q -b main repo && cd repo && git config user.email t@example.com && git config user.name T && git config commit.gpgsign false && printf 'one\\n' > a.txt && printf 'keep\\n' > b.txt && printf '*.log\\n' > .gitignore && git add -A && git commit -qm init && printf 'u1\\n' > u.txt"

def edit1() -> String:
  "printf 'two\\n' > a.txt && rm b.txt && printf 'new\\n' > c.txt && printf 'ignored\\n' > ig.log && printf 'u2\\n' > u.txt"

def edit2() -> String:
  "printf 'three\\n' > a.txt && mkdir -p sub && printf 'd\\n' > sub/d.txt"

def pure_checks() -> IO(Bool):
  do IO<Bool>:
    a1 : Bool <- chk("sanitize words", G.Git.branch.sanitize("Feature/Add Thing!"), "feature/add-thing")
    a2 : Bool <- chk("sanitize edges", G.Git.branch.sanitize("  --Hello__World..//x--  "), "hello__world/x")
    a3 : Bool <- chk("sanitize dots", G.Git.branch.sanitize("a..b/.c"), "a.b/c")
    a4 : Bool <- chk("sanitize runs", G.Git.branch.sanitize("fix---the   bug"), "fix-the-bug")
    a5 : Bool <- chk("sanitize empty", G.Git.branch.sanitize("!!!"), "")
    a6 : Bool <- chk("sanitize unicode", G.Git.branch.sanitize("Café crème"), "caf-cr-me")
    a7 : Bool <- chk("sanitize long", len(G.Git.branch.sanitize(String.repeat("ab", 50n))), "64")
    a8 : Bool <- chk("sanitize long tail", len(G.Git.branch.sanitize(String.repeat("x-", 40n))), "63")
    a9 : Bool <- chk("temp branch", len(G.Git.branch.temp(42n)), "18")
    b1 : Bool <- chk("temp branch stable", G.Git.branch.temp(42n), G.Git.branch.temp(42n))
    b2 : Bool <- chk("temp branch differs", Bool.show(String.eq(G.Git.branch.temp(1n), G.Git.branch.temp(2n))), "False")
    b3 : Bool <- chk("checkpoint ref", G.Git.checkpoint.ref("thread-1", 3n), "refs/backplane/checkpoints/dGhyZWFkLTE/turn/3")
    b4 : Bool <- chk("b64url", G.Git.b64url("?>?"), "Pz4_")
    b5 : Bool <- chk("b64url plus", G.Git.b64url("~~~"), "fn5-")
    b6 : Bool <- chk("worktree path", G.Git.worktree.path("/b", "repo", "backplane/ab12"), "/b/worktrees/repo/backplane-ab12")
    b7 : Bool <- chk("status parse", status.show(G.Git.Status.parse("## main...origin/main [ahead 2, behind 1]\n M a.txt\n?? new file.txt\n")), "main +2 -1 2 a.txt,new file.txt")
    b8 : Bool <- chk("status unborn", status.show(G.Git.Status.parse("## No commits yet on dev\n")), "dev +0 -0 0 ")
    b9 : Bool <- chk("status behind", status.show(G.Git.Status.parse("## x/y...origin/x/y [behind 12]\nR  o -> n\n")), "x/y +0 -12 1 o -> n")
    return all([a1, a2, a3, a4, a5, a6, a7, a8, a9, b1, b2, b3, b4, b5, b6, b7, b8, b9])

def checkpoint_checks(+root: String) -> IO(Bool):
  +repo = root ++ "/repo"
  do IO<Bool>:
    +s0 : P.Ran <- sh(root, setup())
    c0 : Bool <- chk("setup", code(s0) ++ out(s0), "0")
    r1 : Bool <- IO.bind(Bool, Bool, G.Git.is_repo(repo), b => chk("is_repo", Bool.show(b), "True"))
    r2 : Bool <- IO.bind(Bool, Bool, G.Git.is_repo(root), b => chk("is_repo outside", Bool.show(b), "False"))
    rt : String <- G.Git.root(repo)
    r3 : Bool <- chk("root", rt, repo)
    br : String <- G.Git.branch(repo)
    r4 : Bool <- chk("branch", br, "main")
    head0 : String <- git(repo, ["rev-parse", "HEAD"])
    +k0 : String <- G.Git.checkpoint.capture(repo, "thread-1", 0n)
    r5 : Bool <- chk("capture 0", len(k0), "40")
    s1 : P.Ran <- sh(repo, edit1())
    +k1 : String <- G.Git.checkpoint.capture(repo, "thread-1", 1n)
    r6 : Bool <- chk("capture 1", len(k1), "40")
    p1 : String <- git(repo, ["rev-parse", k1 ++ "^"])
    r7 : Bool <- chk("capture 1 parent", p1, k0)
    +d01 : String <- G.Git.checkpoint.diff(repo, "thread-1", 0n, 1n)
    r8 : Bool <- chk("diff 0..1", summary(d01), "4 files, +3 −3")
    r9 : Bool <- chk("diff 0..1 files", files(d01), "modified:a.txt deleted:b.txt added:c.txt modified:u.txt")
    d11 : String <- G.Git.checkpoint.diff(repo, "thread-1", 1n, 1n)
    q1 : Bool <- chk("diff same", d11, "")
    dx : String <- G.Git.checkpoint.diff(repo, "thread-1", 1n, 7n)
    q2 : Bool <- chk("diff missing", Bool.show(G.Git.failed(dx)), "True")
    s2 : P.Ran <- sh(repo, edit2())
    +dw : String <- G.Git.checkpoint.diff_worktree(repo, "thread-1", 1n)
    q3 : Bool <- chk("diff worktree", files(dw), "modified:a.txt added:sub/d.txt")
    q4 : Bool <- chk("diff worktree stats", summary(dw), "2 files, +2 −1")
    rv : String <- G.Git.checkpoint.revert(repo, "thread-1", 0n)
    q5 : Bool <- chk("revert", rv, "")
    fa : String <- cat(repo ++ "/a.txt")
    q6 : Bool <- chk("revert a.txt", fa, "one")
    fb : String <- cat(repo ++ "/b.txt")
    q7 : Bool <- chk("revert b.txt back", fb, "keep")
    fu : String <- cat(repo ++ "/u.txt")
    q8 : Bool <- chk("revert untracked u.txt", fu, "u1")
    ec : String <- exists(repo ++ "/c.txt")
    q9 : Bool <- chk("revert removes c.txt", ec, "False")
    ed : String <- exists(repo ++ "/sub/d.txt")
    w1 : Bool <- chk("revert removes sub/d.txt", ed, "False")
    ei : String <- exists(repo ++ "/ig.log")
    w2 : Bool <- chk("revert keeps ignored", ei, "True")
    head1 : String <- git(repo, ["rev-parse", "HEAD"])
    w3 : Bool <- chk("revert keeps HEAD", head1, head0)
    st : String <- G.Git.status(repo)
    w4 : Bool <- chk("status after revert", status.show(G.Git.Status.parse(st)), "main +0 -0 1 u.txt")
    rv1 : String <- G.Git.checkpoint.revert(repo, "thread-1", 1n)
    fa1 : String <- cat(repo ++ "/a.txt")
    w5 : Bool <- chk("revert forward", rv1 ++ fa1, "two")
    refs : String <- git(repo, ["for-each-ref", "--format=%(refname)", "refs/backplane"])
    w6 : Bool <- chk("refs", refs, "refs/backplane/checkpoints/dGhyZWFkLTE/turn/0\nrefs/backplane/checkpoints/dGhyZWFkLTE/turn/1")
    lg : String <- git(repo, ["rev-list", "--count", "HEAD"])
    w7 : Bool <- chk("no commits on branch", lg, "1")
    rvx : String <- G.Git.checkpoint.revert(repo, "thread-1", 9n)
    w8 : Bool <- chk("revert missing fails", rvx, "error: fatal: Needed a single revision")
    fa2 : String <- cat(repo ++ "/a.txt")
    w9 : Bool <- chk("revert missing leaves files", fa2, "two")
    return all([c0, r1, r2, r3, r4, r5, r6, r7, r8, r9, q1, q2, q3, q4, q5, q6, q7, q8, q9, w1, w2, w3, w4, w5, w6, w7, w8, w9])

def worktree_checks(+root: String) -> IO(Bool):
  +repo = root ++ "/repo"
  +base = root ++ "/base"
  +wt = root ++ "/base/worktrees/repo/feature-add-thing"
  do IO<Bool>:
    p : String <- G.Git.worktree.create(repo, base, "repo", "Feature/Add Thing!", "")
    t1 : Bool <- chk("worktree create", p, wt)
    fa : String <- cat(wt ++ "/a.txt")
    t2 : Bool <- chk("worktree files", fa, "one")
    b : String <- G.Git.branch(wt)
    t3 : Bool <- chk("worktree branch", b, "feature/add-thing")
    k : String <- G.Git.checkpoint.capture(wt, "wt-thread", 0n)
    t4 : Bool <- chk("worktree capture", len(k), "40")
    rn : String <- G.Git.branch.rename(wt, "feature/add-thing", "Better Name")
    t5 : Bool <- chk("rename", rn, "better-name")
    b2 : String <- G.Git.branch(wt)
    t6 : Bool <- chk("renamed branch", b2, "better-name")
    dup : String <- G.Git.worktree.create(repo, base, "repo", "better-name", "")
    t7 : Bool <- chk("create clash fails", Bool.show(G.Git.failed(dup)), "True")
    +tp : String <- G.Git.worktree.create(repo, base, "repo", G.Git.branch.temp(7n), "main")
    t8 : Bool <- chk("temp worktree", tp, G.Git.worktree.path(base, "repo", G.Git.branch.temp(7n)))
    rm : String <- G.Git.worktree.remove(repo, wt)
    t9 : Bool <- chk("worktree remove", rm, "")
    ew : String <- exists(wt)
    u1 : Bool <- chk("worktree gone", ew, "False")
    s : P.Ran <- sh(tp, "printf 'dirty\\n' > a.txt")
    rm2 : String <- G.Git.worktree.remove(repo, tp)
    u2 : Bool <- chk("remove dirty refused", Bool.show(G.Git.failed(rm2)), "True")
    rm3 : String <- G.Git.worktree.remove_force(repo, tp)
    u3 : Bool <- chk("remove forced", rm3, "")
    return all([t1, t2, t3, t4, t5, t6, t7, t8, t9, u1, u2, u3])

# thread worktrees in <repo>/.backplane: a KiCad project in a subfolder,
# its uncommitted work carried over, the folder never seeing them, removal
# after a checkpoint, and making one again from the kept branch
def thread_wt_checks(+root: String) -> IO(Bool):
  +repo = root ++ "/tw"
  +hw = root ++ "/tw/hw"
  +wt = root ++ "/tw/.backplane/worktrees/backplane-abc"
  do IO<Bool>:
    s0 : P.Ran <- sh(root, "mkdir -p tw/hw plain unborn && cd tw && git init -q -b main && git config user.email t@t && git config user.name t && printf 'p\\n' > hw/board.kicad_pro && printf 'v1\\n' > hw/board.kicad_pcb && printf 'old\\n' > hw/old.txt && git add -A && git commit -q -m one && printf 'v2\\n' > hw/board.kicad_pcb && printf 'sym\\n' > hw/new.kicad_sym && mkdir -p hw/board-backups && printf 'z' > hw/board-backups/b.zip && printf 'l' > hw/~board.kicad_pcb.lck && rm hw/old.txt && cd ../unborn && git init -q")
    +p : String <- G.Git.thread_wt(hw, True{}, "backplane/abc")
    a1 : Bool <- chk("thread worktree", p, wt ++ "/hw")
    pc : String <- cat(wt ++ "/hw/board.kicad_pcb")
    a2 : Bool <- chk("carries changes", pc, "v2")
    ns : String <- exists(wt ++ "/hw/new.kicad_sym")
    a3 : Bool <- chk("carries new files", ns, "True")
    bk : String <- exists(wt ++ "/hw/board-backups")
    a4 : Bool <- chk("leaves backups", bk, "False")
    lk : String <- exists(wt ++ "/hw/~board.kicad_pcb.lck")
    a5 : Bool <- chk("leaves locks", lk, "False")
    od : String <- exists(wt ++ "/hw/old.txt")
    a6 : Bool <- chk("carries deletions", od, "False")
    st : String <- git(repo, ["status", "--porcelain", "-uall"])
    a7 : Bool <- chk("folder never sees it", Bool.show(String.contains(st, ".backplane")), "False")
    x2 : String <- G.Git.exclude(hw)
    ex : P.Ran <- sh(repo, "grep -c -x /.backplane/ .git/info/exclude")
    a8 : Bool <- chk("excluded once", out(ex), "1")
    pl : String <- G.Git.thread_wt(root ++ "/plain", False{}, "backplane/p")
    a9 : Bool <- chk("no repository", pl, "skip: not a git repository")
    ub : String <- G.Git.thread_wt(root ++ "/unborn", False{}, "backplane/u")
    b1 : Bool <- chk("no commit", ub, "skip: no commit yet")
    +sw : String <- G.Git.thread_wt(repo, False{}, "backplane/sw")
    sp : String <- cat(sw ++ "/hw/board.kicad_pcb")
    b2 : Bool <- chk("software starts clean", sp, "v1")
    s1 : P.Ran <- sh(wt ++ "/hw", "printf 'v3\\n' > board.kicad_pcb")
    k : String <- G.Git.checkpoint.capture(p, "tw-thread", 0n)
    b3 : Bool <- chk("checkpoint before removal", len(k), "40")
    rm : String <- G.Git.worktree.remove_force(hw, wt)
    ew : String <- exists(wt)
    b4 : Bool <- chk("removed", rm ++ ew, "False")
    br : String <- git(repo, ["branch", "--list", "backplane/abc"])
    b5 : Bool <- chk("branch kept", br, "backplane/abc")
    +p2 : String <- G.Git.thread_wt.again(hw, "backplane/abc")
    b6 : Bool <- chk("made again", p2, wt ++ "/hw")
    rv : String <- G.Git.checkpoint.revert(p2, "tw-thread", 0n)
    p3 : String <- cat(p2 ++ "/board.kicad_pcb")
    b7 : Bool <- chk("state restored", rv ++ p3, "v3")
    s2 : P.Ran <- sh(root, "git init -q --bare -b main twr.git && git -C tw remote add origin ../twr.git && git -C tw push -q origin main && git clone -q twr.git twc && cd twc && git config user.email t@t && git config user.name t && printf 'u\\n' > up.txt && git add up.txt && git commit -q -m up && git push -q origin main")
    +up : String <- G.Git.thread_wt(repo, False{}, "backplane/up")
    uf : String <- exists(up ++ "/up.txt")
    c1 : Bool <- chk("starts from upstream main", uf, "True")
    ut : P.Ran <- sh(up, "git rev-parse -q --verify @{u} || echo none")
    c2 : Bool <- chk("tracks nothing", out(ut), "none")
    +hc : String <- G.Git.thread_wt(hw, True{}, "backplane/hc")
    hf : String <- exists(hc ++ "/../up.txt")
    c3 : Bool <- chk("carried work stays on HEAD", hf, "False")
    return all([a1, a2, a3, a4, a5, a6, a7, a8, a9, b1, b2, b3, b4, b5, b6, b7, c1, c2, c3])

# A project added only on the checked-out branch must exist in the new
# thread too. A newer origin/main remains preferred when it has that
# directory; a same-named blob is not a project directory.
def prefix_setup() -> String:
  "git init -q --bare -b main prefix-origin.git && git init -q -b main prefix && cd prefix && git config user.email t@t && git config user.name t && git config commit.gpgsign false && mkdir existing && printf 'main\\n' > existing/board.kicad_pro && printf 'blob\\n' > collision && git add -A && git commit -qm base && git remote add origin ../prefix-origin.git && git push -qu origin main && git checkout -qb local-feature && mkdir 'feature only' 'design $(touch injected); pieces' && printf 'feature\\n' > 'feature only/board.kicad_pro' && printf 'special\\n' > 'design $(touch injected); pieces/board.kicad_pro' && rm collision && mkdir collision && printf 'directory\\n' > collision/board.kicad_pro && git add -A && git commit -qm projects && git checkout -qb upstream-source main && printf 'upstream\\n' > existing/board.kicad_pro && git add -A && git commit -qm upstream && git push -q origin HEAD:main && git checkout -q local-feature"

def thread_prefix_checks(+root: String) -> IO(Bool):
  +repo = root ++ "/prefix"
  do IO<Bool>:
    +s0 : P.Ran <- sh(root, prefix_setup())
    a0 : Bool <- chk("prefix fixture setup", code(s0) ++ out(s0), "0")
    +head : String <- git(repo, ["rev-parse", "HEAD"])
    +upstream : String <- git(repo, ["rev-parse", "origin/main"])
    a1 : Bool <- chk("prefix fixture commits differ", Bool.show(String.eq(head, upstream)), "False")
    +fresh : String <- G.Git.thread_wt(repo ++ "/feature only", False{}, "backplane/prefix-new")
    a2 : Bool <- IO.bind(String, Bool, exists(fresh), v => chk("feature-only project directory exists", v, "True"))
    a3 : Bool <- IO.bind(String, Bool, cat(fresh ++ "/board.kicad_pro"), v => chk("feature-only project files", v, "feature"))
    a4 : Bool <- IO.bind(String, Bool, git(fresh, ["rev-parse", "HEAD"]), v => chk("feature-only project starts at HEAD", v, head))
    a5 : Bool <- IO.bind(String, Bool, git(fresh, ["config", "--get", "branch.backplane/prefix-new.remote"]), v => chk("fallback branch tracks nothing", v, ""))
    +blob : String <- G.Git.thread_wt(repo ++ "/collision", False{}, "backplane/prefix-blob")
    b1 : Bool <- IO.bind(String, Bool, cat(blob ++ "/board.kicad_pro"), v => chk("upstream file does not replace project directory", v, "directory"))
    b2 : Bool <- IO.bind(String, Bool, git(blob, ["rev-parse", "HEAD"]), v => chk("file collision starts at HEAD", v, head))
    +special : String <- G.Git.thread_wt(repo ++ "/design $(touch injected); pieces", False{}, "backplane/prefix-special")
    b3 : Bool <- IO.bind(String, Bool, cat(special ++ "/board.kicad_pro"), v => chk("project path keeps spaces and shell characters", v, "special"))
    b4 : Bool <- IO.bind(String, Bool, exists(repo ++ "/injected"), v => chk("project path is not evaluated by a shell", v, "False"))
    +normal : String <- G.Git.thread_wt(repo ++ "/existing", False{}, "backplane/prefix-normal")
    c1 : Bool <- IO.bind(String, Bool, cat(normal ++ "/board.kicad_pro"), v => chk("existing project uses upstream files", v, "upstream"))
    c2 : Bool <- IO.bind(String, Bool, git(normal, ["rev-parse", "HEAD"]), v => chk("existing project keeps origin base", v, upstream))
    c3 : Bool <- IO.bind(String, Bool, git(normal, ["config", "--get", "branch.backplane/prefix-normal.remote"]), v => chk("upstream-based branch tracks nothing", v, ""))
    s1 : P.Ran <- sh(repo, "mkdir untracked-only && printf 'new\\n' > untracked-only/board.kicad_pro")
    +absent : String <- G.Git.thread_wt(repo ++ "/untracked-only", False{}, "backplane/prefix-untracked")
    d1 : Bool <- chk("uncommitted-only project has a useful error", Bool.show(Bool.and(G.Git.failed(absent), String.contains(absent, "untracked-only"))), "True")
    d2 : Bool <- IO.bind(String, Bool, exists(G.Git.thread_wt.path(repo, "backplane/prefix-untracked")), v => chk("missing project does not create a worktree", v, "False"))
    d3 : Bool <- IO.bind(String, Bool, git(repo, ["branch", "--list", "backplane/prefix-untracked"]), v => chk("missing project does not create a branch", v, ""))
    +untracked : String <- G.Git.thread_wt(repo ++ "/untracked-only", True{}, "backplane/prefix-untracked-carry")
    d4 : Bool <- IO.bind(String, Bool, cat(untracked ++ "/board.kicad_pro"), v => chk("uncommitted-only project can be carried", untracked ++ "\n" ++ v, G.Git.thread_wt.path(repo, "backplane/prefix-untracked-carry") ++ "/untracked-only\nnew"))
    d5 : Bool <- IO.bind(String, Bool, git(untracked, ["rev-parse", "HEAD"]), v => chk("carried uncommitted-only project keeps HEAD base", v, head))
    s2 : P.Ran <- sh(repo, "printf 'dirty\\n' > existing/board.kicad_pro")
    +carried : String <- G.Git.thread_wt(repo ++ "/existing", True{}, "backplane/prefix-carry")
    e1 : Bool <- IO.bind(String, Bool, cat(carried ++ "/board.kicad_pro"), v => chk("carried project keeps uncommitted edits", v, "dirty"))
    e2 : Bool <- IO.bind(String, Bool, git(carried, ["rev-parse", "HEAD"]), v => chk("carried project keeps HEAD base", v, head))
    s3 : P.Ran <- sh(repo, "git remote remove origin")
    +local : String <- G.Git.thread_wt(repo ++ "/feature only", False{}, "backplane/prefix-local")
    f1 : Bool <- IO.bind(String, Bool, cat(local ++ "/board.kicad_pro"), v => chk("project without origin stays usable", v, "feature"))
    f2 : Bool <- IO.bind(String, Bool, git(local, ["rev-parse", "HEAD"]), v => chk("project without origin keeps HEAD base", v, head))
    return all([a0, a1, a2, a3, a4, a5, b1, b2, b3, b4, c1, c2, c3, d1, d2, d3, d4, d5, e1, e2, f1, f2])

def act_checks(+root: String) -> IO(Bool):
  +repo = root ++ "/repo"
  do IO<Bool>:
    s : P.Ran <- sh(root, "git init -q --bare remote.git && cd repo && git remote add origin ../remote.git")
    a : String <- G.Git.act(repo, "commit_push", "Add u", "", "")
    h : String <- git(repo, ["log", "-1", "--format=%h"])
    v1 : Bool <- chk("commit_push", a, "commit: " ++ h ++ " Add u\npush: main -> origin/main")
    rh : String <- git(repo, ["ls-remote", "origin", "refs/heads/main"])
    lh : String <- git(repo, ["rev-parse", "HEAD"])
    v2 : Bool <- chk("pushed", rh, lh ++ "\trefs/heads/main")
    n : String <- G.Git.act(repo, "commit", "x", "", "")
    v3 : Bool <- chk("nothing to commit", n, "commit: nothing to commit")
    s2 : P.Ran <- sh(repo, "printf 'more\\n' >> a.txt")
    a2 : String <- G.Git.act(repo, "commit_push", "", "", "")
    h2 : String <- git(repo, ["log", "-1", "--format=%h"])
    v4 : Bool <- chk("commit_push upstream", a2, "commit: " ++ h2 ++ " Update from Backplane\npush: main")
    u : String <- G.Git.act(repo, "bogus", "", "", "")
    v5 : Bool <- chk("unknown action", u, "error: unknown action bogus")
    +pr : String <- G.Git.act(repo, "commit_push_pr", "", "T", "B")
    v6 : Bool <- chk("pr fails without github", Bool.show(Bool.and(G.Git.failed(pr), String.contains(pr, "\nerror: pr: "))), "True")
    IO.print("  (pr answer: " ++ pr ++ ")")
    return all([v1, v2, v3, v4, v5, v6])

def main() -> IO(Unit):
  do IO<Unit>:
    t : U32 <- H.Clock.epoch()
    +root : String = "/tmp/bp-git-test-" ++ U32.show(t)
    m : P.Ran <- P.Proc.run("mkdir", ["-p", root], "")
    a : Bool <- pure_checks()
    b : Bool <- checkpoint_checks(root)
    c : Bool <- worktree_checks(root)
    d : Bool <- act_checks(root)
    e : Bool <- thread_wt_checks(root)
    f : Bool <- thread_prefix_checks(root)
    x : P.Ran <- P.Proc.run("rm", ["-rf", root], "")
    finish(all([a, b, c, d, e, f]))
