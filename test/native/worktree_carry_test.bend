import Base
import ../check.bend as C
import ../../src/server/proc.bend as P
import ../../src/server/git.bend as G
import ../../src/server/host.bend as H

# Native Git regression, with only temporary repositories and a local bare
# remote. The source's index must not change; the destination gets the
# working files' current contents, whether an edit was staged or not.
# scripts/build-app.sh test/native/worktree_carry_test.bend build/worktree_carry_test
# build/worktree_carry_test

def out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, text}: String.trim(text)

def code(r: P.Ran) -> String:
  match r:
    case P.Ran{c, _}: U32.show(c)

def sh(cwd: String, script: String) -> IO(P.Ran):
  P.Proc.run("sh", ["-c", script], cwd)

def git(cwd: String, args: List<&2, String>) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("git", args, cwd), r => IO.pure(String, out(r)))

def cat(path: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("cat", [path], "/"), r => IO.pure(String, out(r)))

def exists(path: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("test", ["-e", path], "/"), r => IO.pure(String, Bool.show(String.eq(code(r), "0"))))

def chk(name: String, +got: String, +want: String) -> IO(Bool):
  do IO<Bool>:
    IO.print(C.Check.pick(String.eq(got, want), name, got, want))
    return String.eq(got, want)

def all(bs: List<&2, Bool>) -> Bool:
  match bs:
    case Nil{}: True{}
    case b <> t: Bool.and(b, all(t))

def finish(ok: Bool) -> IO(Unit):
  match ok:
    case True{}: IO.print("all ok")
    case False{}: IO.die(Unit, 1, "worktree_carry_test failed")

def setup() -> String:
  String.join([
    "set -eu",
    "git init -q -b main source",
    "cd source",
    "git config user.email carry@example.invalid",
    "git config user.name 'Carry test'",
    "git config commit.gpgsign false",
    "printf '*.log\\n' > .gitignore",
    "printf 'board\\n' > board.kicad_pcb",
    "for f in 'staged edit.txt' 'staged delete.txt' 'rename old.txt' mixed.txt cancelled.txt unstaged.txt recreated.txt recreated-link.txt; do printf 'old\\n' > \"$f\"; done",
    "printf 'old tracked\\n' > tracked.log",
    "git add -A",
    "git add -f tracked.log",
    "git commit -qm initial",
    "git init -q --bare -b main ../remote.git",
    "git remote add origin ../remote.git",
    "git push -q -u origin main",
    "git clone -q ../remote.git ../staged-only",
    "git clone -q ../remote.git ../peer",
    "git -C ../peer config user.email carry@example.invalid",
    "git -C ../peer config user.name 'Carry test'",
    "git -C ../peer config commit.gpgsign false",
    "printf 'upstream\\n' > ../peer/remote-only.txt",
    "git -C ../peer add remote-only.txt",
    "git -C ../peer commit -qm upstream",
    "git -C ../peer push -q origin main",
    "git worktree add -q -b carry-target ../carried",
    "printf 'staged edit\\n' > 'staged edit.txt'",
    "printf 'staged addition\\n' > 'staged add.kicad_sch'",
    "git rm -q 'staged delete.txt'",
    "git rm -q recreated.txt recreated-link.txt",
    "git mv 'rename old.txt' 'rename new.txt'",
    "printf 'staged intermediate\\n' > mixed.txt",
    "printf 'staged intermediate\\n' > cancelled.txt",
    "printf 'new tracked\\n' > tracked.log",
    "n=$(printf 'line\\nbreak.kicad_sch'); printf 'newline name\\n' > \"$n\"",
    "printf 'dash name\\n' > ./-option.kicad_sch",
    "mkdir board-backups",
    "printf 'backup\\n' > board-backups/staged.zip",
    "printf 'lock\\n' > staged.lck",
    "printf 'autosave\\n' > _autosave-staged.kicad_pcb",
    "printf 'cache\\n' > fp-info-cache",
    "git add -A",
    "printf 'working mixed\\n' > mixed.txt",
    "printf 'old\\n' > cancelled.txt",
    "printf 'unstaged edit\\n' > unstaged.txt",
    "printf 'untracked\\n' > 'untracked file.kicad_sym'",
    "printf 'recreated\\n' > recreated.txt",
    "ln -s missing-target recreated-link.txt",
    "printf 'ignored\\n' > ignored.log",
    "cd ../staged-only",
    "printf 'only staged edit\\n' > 'staged edit.txt'",
    "printf 'only staged add\\n' > only-added.txt",
    "git rm -q 'staged delete.txt'",
    "git add -A"
  ], "\n")

def checks(+root: String) -> IO(Bool):
  +source = root ++ "/source"
  +dest = root ++ "/carried"
  +only = root ++ "/staged-only"
  do IO<Bool>:
    s : P.Ran <- sh(root, setup())
    a0 : Bool <- chk("isolated staged carry repositories initialize", code(s), "0")
    +index : String <- git(source, ["diff", "--cached", "--binary"])
    +work : String <- git(source, ["diff", "--binary"])
    +head : String <- git(source, ["rev-parse", "HEAD"])
    result : String <- G.Git.carry(source, dest)
    a1 : Bool <- chk("staged carry reports success", result, "")
    modified : String <- cat(dest ++ "/staged edit.txt")
    a2 : Bool <- chk("carries fully staged modified contents", modified, "staged edit")
    added : String <- cat(dest ++ "/staged add.kicad_sch")
    a3 : Bool <- chk("carries fully staged added file", added, "staged addition")
    deleted : String <- exists(dest ++ "/staged delete.txt")
    a4 : Bool <- chk("carries fully staged deletion", deleted, "False")
    renamed : String <- cat(dest ++ "/rename new.txt")
    a5 : Bool <- chk("carries fully staged rename destination", renamed, "old")
    old_name : String <- exists(dest ++ "/rename old.txt")
    a6 : Bool <- chk("removes fully staged rename source", old_name, "False")
    mixed : String <- cat(dest ++ "/mixed.txt")
    a7 : Bool <- chk("staged plus unstaged edit carries working contents", mixed, "working mixed")
    cancelled : String <- cat(dest ++ "/cancelled.txt")
    a8 : Bool <- chk("unstaged reversal of staged edit carries working contents", cancelled, "old")
    unstage : String <- cat(dest ++ "/unstaged.txt")
    a9 : Bool <- chk("still carries unstaged modified contents", unstage, "unstaged edit")
    untracked : String <- cat(dest ++ "/untracked file.kicad_sym")
    b0 : Bool <- chk("still carries untracked files", untracked, "untracked")
    tracked : String <- cat(dest ++ "/tracked.log")
    b1 : Bool <- chk("staged tracked ignored file still carries", tracked, "new tracked")
    newline : String <- cat(dest ++ "/line\nbreak.kicad_sch")
    b2 : Bool <- chk("staged newline filename is preserved", newline, "newline name")
    dash : String <- cat(dest ++ "/-option.kicad_sch")
    b3 : Bool <- chk("staged leading dash filename is preserved", dash, "dash name")
    recreated : String <- cat(dest ++ "/recreated.txt")
    r0 : Bool <- chk("staged deletion with recreated untracked file keeps its contents", recreated, "recreated")
    recreated_link : P.Ran <- P.Proc.run("readlink", [dest ++ "/recreated-link.txt"], "/")
    r1 : Bool <- chk("staged deletion with recreated broken symlink keeps its target", out(recreated_link), "missing-target")
    ignored : String <- exists(dest ++ "/ignored.log")
    b4 : Bool <- chk("still leaves ignored untracked files behind", ignored, "False")
    backups : String <- exists(dest ++ "/board-backups")
    b5 : Bool <- chk("staged KiCad backups stay behind", backups, "False")
    lock : String <- exists(dest ++ "/staged.lck")
    b6 : Bool <- chk("staged KiCad locks stay behind", lock, "False")
    autosave : String <- exists(dest ++ "/_autosave-staged.kicad_pcb")
    b7 : Bool <- chk("staged KiCad autosaves stay behind", autosave, "False")
    cache : String <- exists(dest ++ "/fp-info-cache")
    b8 : Bool <- chk("staged KiCad footprint cache stays behind", cache, "False")
    index_after : String <- git(source, ["diff", "--cached", "--binary"])
    b9 : Bool <- chk("carry never changes the source index", index_after, index)
    work_after : String <- git(source, ["diff", "--binary"])
    c0 : Bool <- chk("carry never changes the source working files", work_after, work)
    source_head : String <- git(source, ["rev-parse", "HEAD"])
    c1 : Bool <- chk("carry never changes source HEAD", source_head, head)
    dest_index : String <- git(dest, ["diff", "--cached", "--name-only"])
    c2 : Bool <- chk("carry leaves destination index at its base", dest_index, "")
    again : String <- G.Git.carry(source, dest)
    c3 : Bool <- chk("repeated carry succeeds", again, "")
    repeated : String <- cat(dest ++ "/mixed.txt")
    c4 : Bool <- chk("repeated carry keeps working contents", repeated, "working mixed")
    +only_index : String <- git(only, ["diff", "--cached", "--binary"])
    +only_head : String <- git(only, ["rev-parse", "HEAD"])
    +base : String <- G.Git.upstream(True{}, only)
    d0 : Bool <- chk("fully staged work selects HEAD instead of newer upstream", base, "")
    +wt : String <- G.Git.thread_wt(only, True{}, "backplane/staged")
    d1 : Bool <- chk("fully staged worktree creation succeeds", Bool.show(G.Git.failed(wt)), "False")
    wt_head : String <- git(wt, ["rev-parse", "HEAD"])
    d2 : Bool <- chk("fully staged worktree actually starts at source HEAD", wt_head, only_head)
    upstream_file : String <- exists(wt ++ "/remote-only.txt")
    d3 : Bool <- chk("fully staged work never mixes with newer upstream files", upstream_file, "False")
    wt_edit : String <- cat(wt ++ "/staged edit.txt")
    d4 : Bool <- chk("thread worktree includes fully staged edit", wt_edit, "only staged edit")
    wt_add : String <- cat(wt ++ "/only-added.txt")
    d5 : Bool <- chk("thread worktree includes fully staged addition", wt_add, "only staged add")
    wt_del : String <- exists(wt ++ "/staged delete.txt")
    d6 : Bool <- chk("thread worktree includes fully staged deletion", wt_del, "False")
    only_after : String <- git(only, ["diff", "--cached", "--binary"])
    d7 : Bool <- chk("thread worktree creation keeps source staging intact", only_after, only_index)
    clean_base : String <- G.Git.upstream(False{}, only)
    d8 : Bool <- chk("non-carrying worktree still selects upstream", clean_base, "origin/main")
    return all([a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, b0, b1, b2, b3, r0, r1, b4, b5, b6, b7, b8, b9, c0, c1, c2, c3, c4, d0, d1, d2, d3, d4, d5, d6, d7, d8])

def run(+root: String) -> IO(Unit):
  do IO<Unit>:
    directories : P.Ran <- P.Proc.run("mkdir", ["-p", root ++ "/user", root ++ "/empty-templates"], "/tmp")
    H.Env.put("HOME", root ++ "/user")
    H.Env.put("BACKPLANE_HOME", root ++ "/backplane")
    H.Env.put("GIT_CONFIG_NOSYSTEM", "1")
    H.Env.put("GIT_CONFIG_GLOBAL", "/dev/null")
    H.Env.put("GIT_CONFIG_COUNT", "0")
    H.Env.put("GIT_CONFIG_PARAMETERS", "")
    H.Env.put("GIT_TEMPLATE_DIR", root ++ "/empty-templates")
    H.Env.put("GIT_TERMINAL_PROMPT", "0")
    # Link host.c's existing CBOR buffer in this small native program, as
    # native/mailstore_test.bend does. This opens no socket or hub.
    frame : List<&2, U32> <- H.Json.cbor_frame("null", "", "")
    linked : Bool <- chk("host effect linkage frame", Nat.show(List.length(&2, U32, frame)), "3")
    ok : Bool <- checks(root)
    removed : P.Ran <- P.Proc.run("rm", ["-rf", root], "/tmp")
    finish(Bool.and(linked, ok))

def created(r: P.Ran) -> IO(Unit):
  match r:
    case P.Ran{0, root}: run(String.trim(root))
    case P.Ran{_, error}: IO.die(Unit, 1, "cannot create test directory: " ++ error)

def main() -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("mktemp", ["-d", "/tmp/bp-worktree-carry.XXXXXX"], "/tmp"), created)
