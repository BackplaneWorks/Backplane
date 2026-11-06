import Base
import ./utf8.bend as U
import ./model.bend as M
import ./far.bend as FR
import ./fm.bend as FM
import ./fview.bend as FV
import ./hub.bend as H
import ./hist.bend as HI

# A window on another machine's hub (pure; docs/bots.md "Viewers elsewhere",
# "A window on another hub").
#
# The native window reads the files its viewers draw from its own disk. When
# its hub is on another machine those files are not there, so the hub that
# has them (or keeps them, for a thread it only mirrors: core/fview.bend)
# lists a thread's viewer files for its clients with a digest (GET
# /view/ver, answering when the list changes: a long poll) and sends one
# file's bytes (GET /view/get), one file of a commit (GET /view/git). The
# window keeps what it fetched under <cache>/remote/<hub>, in the same
# folders as on the hub, and its viewers read that copy exactly as they read
# a folder here: a hub's path P is read at <cache>/remote/<hub>P (Rview.at).
# Same rules as for the owner's machines (Fv.file.ok): a path never leaves
# the thread's folder and never reaches a hidden file; the hub names the
# folder itself from the thread, so a client names no folder.

# Percent-encoding
# ----------------

def Rview.hexd(+n: U32) -> U32:
  Bool.pick(U32, U32.is_lt(n, 10), (48 + n : U32), (55 + n : U32))

def Rview.plain(+b: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(b, 48), U32.is_le(b, 57)), Bool.and(U32.is_ge(b, 97), U32.is_le(b, 122))),
    Bool.or(Bool.and(U32.is_ge(b, 65), U32.is_le(b, 90)), Bool.or(Bool.or(U32.is_eq(b, 45), U32.is_eq(b, 95)), Bool.or(U32.is_eq(b, 46), U32.is_eq(b, 47)))))

def Rview.enc.one(plain: Bool, +b: U32, acc: List<&2, U32>) -> List<&2, U32>:
  match plain:
    case True{}:
      b <> acc
    case False{}:
      Rview.hexd(U32.mod(b, 16)) <> Rview.hexd((b / 16 : U32)) <> 37 <> acc

def Rview.enc.go(bs: List<&2, U32>, acc: List<&2, U32>) -> List<&2, U32>:
  match bs:
    case Nil{}:
      acc
    case +b <> t:
      Rview.enc.go(t, Rview.enc.one(Rview.plain(b), b, acc))

# a query value, percent-encoded (every byte but letters, digits and - _ . /)
def Rview.enc(+s: String) -> String:
  U.Utf8.decode(List.reverse(&2, U32, Rview.enc.go(U.Utf8.encode(s), Nil{})))

# Where the copy lives
# --------------------

def Rview.key.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      SCon{Chr{Bool.pick(U32, Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122))),
        Bool.or(Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), Bool.or(U32.is_eq(c, 46), U32.is_eq(c, 45)))), c, 95)}, Rview.key.go(t)}

# a hub's folder name: its address with every odd character an underscore
# ("." alone never starts it, so it is never ".." either)
def Rview.key(+url: String) -> String:
  "h" ++ Rview.key.go(String.take(url, 200n))

# <cache>/remote/<hub>: what the window keeps of that hub's files
def Rview.pre(+cache: String, +url: String) -> String:
  cache ++ "/remote/" ++ Rview.key(url)

# a path on the hub, where it is read here: under the hub's folder, or ""
# when it is not an absolute path that stays where it says; a path that is
# not a file (a version, a comparison, "") is as it was
# (a path already in the copies stays as it is: the sheet tree read from a
# copy names its children there, and they are not copied twice)
def Rview.at(+pre: String, +path: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(pre), String.starts_with(path, pre ++ "/")), path,
    Bool.pick(String, String.starts_with(path, "/"),
      Bool.pick(String, FM.Fm.safe(String.drop(path, 1n)), pre ++ path, ""), path))

# the hub's path a path here is the copy of ("" when it is not one)
def Rview.back(+pre: String, +path: String) -> String:
  Bool.pick(String, Bool.and(Bool.not(String.is_empty(pre)), String.starts_with(path, pre ++ "/")), String.drop(path, String.length(pre)), "")

def Rview.want.k(+rel: String) -> String:
  Bool.pick(String, FV.Fv.file.ok(rel), rel, "")

# the file a source names on the hub, from its folder: the one the viewer
# was asked to show, when it is a file a viewer may read ("" when the
# project's own board or schematic is shown)
def Rview.want(+pre: String, +root: String, +path: String) -> String:
  Rview.want.k(FV.Fv.rel(Rview.back(pre, root), Rview.back(pre, path)))

# What goes to a fetching fiber: the hub, the folder of copies, the thread
# and the connection's tag, one a line (none has a line break)
def Rview.spec(+url: String, +pre: String, +thread: String, +tag: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(url), String.is_empty(pre)), "", url ++ "\n" ++ pre ++ "\n" ++ thread ++ "\n" ++ tag)

def Rview.part(+spec: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, String.split(spec, '\n'), i)), "")

# The hub
# -------

# a folder a client says holds a mirrored thread's files: the cache this hub
# keeps of that link's machine (Fv.cache), as the client was told by far.view;
# "" for any other folder
def Rview.far.k(has: Bool, +home: String, +link: String, +claimed: String) -> String:
  Bool.pick(String, has,
    Bool.pick(String, String.eq(FV.Fv.cache(home, link, String.drop(claimed, String.length(home ++ "/farfs/" ++ link))), claimed), claimed, ""), "")

def Rview.far.root(+home: String, +link: String, +claimed: String) -> String:
  Rview.far.k(String.starts_with(claimed, home ++ "/farfs/" ++ link ++ "/"), home, link, claimed)

# the thread's folder as this hub has its files: its own, or for a thread
# it mirrors (the hub's state holds none of it) the cache it keeps of the
# owner's (core/fview.bend) that the client names; "" for a thread this hub
# does not know or a folder that is not absolute
def Rview.root.k(known: Bool, far: Bool, +home: String, +link: String, +cwd: String, +claimed: String) -> String:
  Bool.pick(String, far, Rview.far.root(home, link, claimed),
    Bool.pick(String, Bool.and(known, String.starts_with(cwd, "/")), cwd, ""))

def Rview.has(r: Map<&2, M.Thread> & Bool) -> Bool:
  (_, b) = r
  b

def Rview.root(+home: String, +s: M.State, +th: String, +claimed: String) -> String:
  Rview.root.k(Rview.has(Map.has(&2, M.Thread, FR.Fr.st.threads(s), th)), FR.Fr.is(th), home, FR.Fr.link(th), Maybe.default(&2, String, H.Hub.root_for(s, th), ""), claimed)

# which request a path is: 1 the listing, 2 a file, 3 a file of a commit
def Rview.kind(+route: String) -> Nat:
  Bool.pick(Nat, String.eq(route, "/view/ver"), 1n, Bool.pick(Nat, String.eq(route, "/view/get"), 2n, Bool.pick(Nat, String.eq(route, "/view/git"), 3n, 0n)))

def Rview.hex.go(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 102))), Rview.hex.go(t))

# a commit named by its hash (never an option for git)
def Rview.hex(+s: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(s)), Bool.and(Nat.is_le(String.length(s), 64n), Rview.hex.go(s)))

# a git directory the hub's own history keeps: a repository's, a
# worktree's of it, or the folder outside git (<folder>/.backplane/history.git)
def Rview.gitdir(+g: String) -> Bool:
  Bool.and(Bool.and(String.starts_with(g, "/"), FM.Fm.safe(String.drop(g, 1n))),
    Bool.or(Bool.or(String.ends_with(g, "/.git"), String.ends_with(g, "/.backplane/history.git")), String.contains(g, "/.git/worktrees/")))

# a version of a file in a commit that a client may read
def Rview.version.ok(+p: String) -> Bool:
  Bool.and(Nat.is_eq(HI.Hist.kind(p), 1n),
    Bool.and(Bool.and(Rview.gitdir(HI.Hist.git.dir(p)), Rview.hex(HI.Hist.git.commit(p))),
      Bool.and(FM.Fm.safe(HI.Hist.git.rel(p)), Bool.not(String.is_empty(HI.Hist.git.rel(p))))))

# a thread's project folder ("" for a thread the hub does not keep itself:
# a mirrored thread's history is its own machine's)
def Rview.proot.k(known: Bool, +s: M.State, +th: String) -> String:
  Bool.pick(String, Bool.and(known, Bool.not(FR.Fr.is(th))), H.Hub.project.root(H.Hub.project(s, H.Hub.thread_project(H.Hub.thread(s, th)))), "")

def Rview.proot(+s: M.State, +th: String) -> String:
  Rview.proot.k(Bool.not(String.is_empty(Maybe.default(&2, String, H.Hub.root_for(s, th), ""))), s, th)

# a version's git directory is the thread's own: inside its folder or its
# project's (the repository, a worktree's record in it, the history kept
# outside git), and the file in it one a viewer may read (no hidden file)
def Rview.bound(+g: String, +root: String, +proot: String) -> Bool:
  Bool.or(Bool.and(Bool.not(String.is_empty(root)), String.starts_with(g, root ++ "/")),
    Bool.and(Bool.not(String.is_empty(proot)), String.starts_with(g, proot ++ "/")))

def Rview.version.mine(+p: String, +root: String, +proot: String) -> Bool:
  Bool.and(Rview.version.ok(p), Bool.and(Rview.bound(HI.Hist.git.dir(p), root, proot), FV.Fv.file.ok(HI.Hist.git.rel(p))))

# whether a request is one the hub answers: a thread it has a folder for
# and paths a viewer may read
def Rview.ask.k(+k: Nat, +root: String, +proot: String, +want: String, +path: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(root)),
    Bool.pick(Bool, Nat.is_eq(k, 1n), FV.Fv.wants.ok(want),
      Bool.pick(Bool, Nat.is_eq(k, 2n), FV.Fv.file.ok(path),
        Bool.and(Nat.is_eq(k, 3n), Rview.version.mine(path, root, proot)))))

def Rview.ask.ok(+route: String, +root: String, +proot: String, +want: String, +path: String) -> Bool:
  Rview.ask.k(Rview.kind(route), root, proot, want, path)
