import Base
import ./model.bend as M
import ./hub.bend as H
import ./rview.bend as RV
import ./hist.bend as HI
import ./fm.bend as FM
import ./folder.bend as FP
import ./far.bend as FR

# What kicad.watch and kicad.warm may name (pure; the server adds the parts
# that need the disk, git identity of a version, in server/git.bend's
# Git.ident). The request names a folder (root), optionally the thread it is
# for, and sources: the live file, a version in a commit, or a comparison
# (core/hist.bend). Nothing here looks at the filesystem, so a file inside
# the folder that is a link out of it is not caught here.
#
# Folders. A thread given: exactly the folder this hub has for it (its
# worktree, else its project's; a deleted or removed thread has none), or
# for a thread of another machine the cache this hub keeps of its files
# (<home>/farfs/<link>/, the link live). No thread: exactly a live project's
# root or a live thread's folder. Never an ancestor or a descendant of one.
#
# Sources. A live file: inside the folder, or exactly the file the thread's
# agent showed with viewer_show (the setting viewer.src:<thread>, written by
# the hub when it answers the tool). A version: a commit hash in a git
# directory that the server then proves to be the folder's own repository,
# of a design file. A comparison: each of its sides, an empty side being the
# live file.

def Wp.grant(+s: M.State, +thread: String) -> String:
  Bool.pick(String, String.is_empty(thread), "", M.Setting.get(s, "viewer.src:" ++ thread, ""))

def Wp.exact.projects(ps: List<&2, M.Project>, +root: String) -> Bool:
  match ps:
    case Nil{}: False{}
    case M.Project{_, _, +r, _, False{}} <> t: Bool.or(String.eq(r, root), Wp.exact.projects(t, root))
    case _ <> t: Wp.exact.projects(t, root)

def Wp.exact.threads(ts: List<&2, M.Thread>, +s: M.State, +root: String) -> Bool:
  match ts:
    case Nil{}: False{}
    case +th <> t: Bool.or(String.eq(H.Hub.cwd(s, th), root), Wp.exact.threads(t, s, root))

def Wp.far.root(+home: String, +thread: String, +root: String, live: Bool) -> Bool:
  Bool.and(live, Bool.and(Bool.not(String.is_empty(home)), Bool.not(String.is_empty(RV.Rview.far.root(home, FR.Fr.link(thread), root)))))

def Wp.local.root(+s: M.State, +thread: String, +root: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(root)), String.eq(Maybe.default(&2, String, H.Hub.root_for(s, thread), ""), root))

def Wp.any.root(+s: M.State, +root: String) -> Bool:
  Bool.and(FP.Folder.absolute(root), Bool.or(Wp.exact.projects(H.Hub.projects(s), root),
    Wp.exact.threads(Map.values(&2, M.Thread, FR.Fr.st.threads(s)), s, root)))

def Wp.root.k(far: Bool, +s: M.State, +home: String, +thread: String, +root: String, live: Bool) -> Bool:
  match far:
    case True{}: Wp.far.root(home, thread, root, live)
    case False{}: Wp.local.root(s, thread, root)

# live: whether the link of a thread of another machine is live
def Wp.root(+s: M.State, +home: String, +thread: String, +root: String, live: Bool) -> Bool:
  Bool.pick(Bool, String.is_empty(thread), Wp.any.root(s, root), Wp.root.k(FR.Fr.is(thread), s, home, thread, root, live))

# a design file's extension
def Wp.design(+rel: String) -> Bool:
  Bool.or(String.ends_with(rel, ".kicad_pcb"), String.ends_with(rel, ".kicad_sch"))

# a version's shape: absolute git directory without a climbing part, a
# commit hash, a design file of the commit that does not climb
def Wp.version(+p: String) -> Bool:
  Bool.and(Bool.and(String.starts_with(HI.Hist.git.dir(p), "/"), FM.Fm.safe(String.drop(HI.Hist.git.dir(p), 1n))),
    Bool.and(RV.Rview.hex(HI.Hist.git.commit(p)),
      Bool.and(Bool.not(String.is_empty(HI.Hist.git.rel(p))), Bool.and(FM.Fm.safe(HI.Hist.git.rel(p)), Wp.design(HI.Hist.git.rel(p))))))

def Wp.live(+root: String, +grant: String, +p: String) -> Bool:
  Bool.and(FP.Folder.absolute(p), Bool.or(Bool.and(String.starts_with(p, root ++ "/"), FM.Fm.safe(FM.Fm.rel(root, p))),
    Bool.and(Bool.not(String.is_empty(grant)), String.eq(p, grant))))

def Wp.side(+root: String, +grant: String, +p: String) -> Bool:
  Bool.or(String.is_empty(p), Bool.pick(Bool, Nat.is_eq(HI.Hist.kind(p), 1n), Wp.version(p),
    Bool.and(Nat.is_eq(HI.Hist.kind(p), 0n), Wp.live(root, grant, p))))

# the sides of a source ("" for none): a comparison's old and new
def Wp.sides(+p: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, Nat.is_eq(HI.Hist.kind(p), 2n), [HI.Hist.old(p), HI.Hist.new(p)], [p])

def Wp.sides.ok(xs: List<&2, String>, +root: String, +grant: String) -> Bool:
  match xs:
    case Nil{}: True{}
    case +x <> t: Bool.and(Wp.side(root, grant, x), Wp.sides.ok(t, root, grant))

def Wp.source(+root: String, +grant: String, +p: String) -> Bool:
  Wp.sides.ok(Wp.sides(p), root, grant)

# the versions among sources (their git directories are proven by the server)
def Wp.keep(git: Bool, +x: String, rest: List<&2, String>) -> List<&2, String>:
  match git:
    case True{}: x <> rest
    case False{}: rest

def Wp.gits.of(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case +x <> t: Wp.keep(Nat.is_eq(HI.Hist.kind(x), 1n), x, Wp.gits.of(t))

def Wp.gits(+ps: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}: Nil{}
    case +p <> t: List.append(&2, String, Wp.gits.of(Wp.sides(p)), Wp.gits(t))

# the request as far as it can be judged without the disk: a stop
# (kind "") is always allowed
def Wp.ok(+s: M.State, +home: String, +kind: String, +thread: String, +root: String, +path: String, +tree: String, +sheet: String, live: Bool) -> Bool:
  Bool.or(String.is_empty(kind), Bool.and(Wp.root(s, home, thread, root, live),
    Bool.and(Wp.source(root, Wp.grant(s, thread), path), Bool.and(Wp.source(root, Wp.grant(s, thread), tree), Wp.source(root, Wp.grant(s, thread), sheet)))))
