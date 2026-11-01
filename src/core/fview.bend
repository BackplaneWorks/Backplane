import Base
import ./json.bend as J
import ./model.bend as M
import ./far.bend as FR
import ./fm.bend as FM
import ./mech.bend as MC

# Viewers for threads elsewhere (pure; docs/bots.md "Viewers elsewhere").
#
# A thread on a linked machine shows in the viewer panel like one here:
# its Files tab, board, schematic, 3D, Mech, Diff. The owning hub answers
# read-only requests about a thread it shares (Fv.reads, POST /far/rpc,
# signed and owner-only like every request between hubs); the hub the
# client is on keeps the files the viewers read in a cache of its own,
# <home>/farfs/<link>/<the owner's project folder>, so the viewers that read
# files (the window, the phones' plot actor, the web's) read the cache as
# they read a folder here. Nothing here can write on the owner's machine,
# and a path never leaves the thread's folder (Fv.file.ok).

# The requests the owning hub answers about one of its shared threads,
# beside Fr.methods' writes: the viewers' needs, all reads. view.ver lists
# the files the viewers want (and answers when that changes, Fvs.ver);
# view.get is one file's bytes.
def Fv.reads() -> List<&2, String>:
  ["files.dir", "files.list", "mech.list", "history.refs", "git.diff", "git.status", "view.ver", "view.get"]

def Fv.read(+m: String) -> Bool:
  FR.Fr.may.go(Fv.reads(), m)

# The owner tells its other machines which file an agent's viewer_show put
# on screen (the thread's own id, the path from its folder): they bring it
# into their cache and show it to their clients.
def Fv.show.is(+m: String) -> Bool:
  String.eq(m, "view.show")

# the jobs those run on the owning hub (the job a request becomes, B.Git):
# none writes, starts a program or opens a path; anything else is dropped
def Fv.jobs() -> List<&2, String>:
  ["dir", "files", "mech", "refs", "diff", "status"]

def Fv.job(+j: String) -> Bool:
  FR.Fr.may.go(Fv.jobs(), j)

# The requests the client's own hub answers itself, each with the owning
# hub's help: far.view keeps the cache of a thread's files up to date,
# mech.list also brings the renders it lists.
def Fv.near() -> List<&2, String>:
  ["far.view", "mech.list"]

def Fv.near.is(+m: String) -> Bool:
  FR.Fr.may.go(Fv.near(), m)

# Paths
# -----

def Fv.dot.go(ps: List<&2, String>) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case +p <> t:
      Bool.or(String.starts_with(p, "."), Fv.dot.go(t))

# a path with a hidden segment ("." and ".." too)
def Fv.dot(+rel: String) -> Bool:
  Fv.dot.go(String.split(rel, '/'))

def Fv.plain(+s: String) -> Bool:
  Bool.not(Bool.or(Bool.or(String.contains(s, "\n"), String.contains(s, "\t")), String.contains(s, "\r")))

# the one hidden file the viewers read: the project's own choice of its
# board and schematic
def Fv.pick() -> String:
  ".backplane.json"

# a file the viewers may read: from the thread's folder, never leaving it,
# no hidden segment (.git, .env, .ssh stay where they are)
def Fv.file.ok(+rel: String) -> Bool:
  Bool.or(String.eq(rel, Fv.pick()),
    Bool.and(Bool.and(Bool.not(String.is_empty(rel)), FM.Fm.safe(rel)),
      Bool.and(Bool.and(Bool.not(Fv.dot(rel)), Bool.not(String.ends_with(rel, "/"))), Fv.plain(rel))))

# a folder the Files tab lists: the root or a folder as safe as a file
def Fv.dir.ok(+rel: String) -> Bool:
  Bool.or(String.is_empty(rel), Fv.file.ok(rel))

def Fv.wants.go(ls: List<&2, String>) -> Bool:
  match ls:
    case Nil{}:
      True{}
    case +l <> t:
      Bool.and(Bool.or(String.is_empty(l), Fv.file.ok(l)), Fv.wants.go(t))

# the files a client wants, one a line: every one a file the viewers may read
def Fv.wants.ok(+text: String) -> Bool:
  Fv.wants.go(String.split(text, '\n'))

# the largest file the owner sends: it streams from disk (farview.bend's
# Fvs.get answers Stream.mark, which server.bend sends a piece at a time)
# and the client's curl writes it straight to a file; core/attach.bend's
# Att.ceiling, 4000 MB
def Fv.cap() -> Nat:
  4194304000n

# seconds a client gives one file to arrive (a large file over a slow link)
def Fv.secs() -> Nat:
  1800n

# What the owning hub takes: a read about a thread it shares whose
# paths are safe
def Fv.params.ok(+m: String, +p: J.Json) -> Bool:
  Bool.pick(Bool, String.eq(m, "view.get"), Fv.file.ok(J.Json.get_str(p, "path", "")),
    Bool.pick(Bool, String.eq(m, "view.ver"), Fv.wants.ok(J.Json.get_str(p, "want", "")),
      Bool.pick(Bool, String.eq(m, "files.dir"), Fv.dir.ok(J.Json.get_str(p, "path", "")), True{})))

def Fv.allowed(+s: M.State, +o: J.Json) -> Bool:
  Bool.and(Bool.and(Fv.read(J.Json.get_str(o, "m", "")), Fv.params.ok(J.Json.get_str(o, "m", ""), J.Json.get_obj(o, "p"))),
    FR.Fr.thread.shared(s, J.Json.get_str(J.Json.get_obj(o, "p"), "thread", "")))

# no read is also a write (law fview_reads_disjoint)
def Fv.disjoint(xs: List<&2, String>, +ys: List<&2, String>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case +x <> t:
      Bool.and(Bool.not(FR.Fr.may.go(ys, x)), Fv.disjoint(t, ys))

# The cache
# ---------

# the folder a link's thread folder is kept in on this machine: the
# owner's absolute folder under <home>/farfs/<link> ("" for a link or a
# folder that could point elsewhere)
def Fv.cache.ok(+link: String, +root: String) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(link)), Fv.plain(link)),
    Bool.and(Bool.and(Bool.not(String.contains(link, "/")), Bool.not(Fv.dot(link))), Bool.and(Bool.and(String.starts_with(root, "/"), Nat.is_gt(String.length(root), 1n)),
      Bool.and(FM.Fm.safe(String.drop(root, 1n)), Bool.not(String.ends_with(root, "/"))))))

def Fv.cache(+home: String, +link: String, +root: String) -> String:
  Bool.pick(String, Fv.cache.ok(link, root), home ++ "/farfs/" ++ link ++ root, "")

# a path of the owner's folder in the cache: the folder itself, or a file
# in it ("" when it is outside, or the link is unsafe)
def Fv.cache.of(+home: String, +link: String, +root: String, +path: String) -> String:
  Bool.pick(String, String.eq(path, root), Fv.cache(home, link, root),
    Bool.pick(String, String.starts_with(path, root ++ "/"), Fv.cache(home, link, root) ++ String.drop(path, String.length(root)), ""))

# the file's path from the owner's folder, "" when it is not in it
def Fv.rel(+root: String, +path: String) -> String:
  FM.Fm.rel(root, path)

# Replies
# -------
# What the owner answers is in its names: the thread's own id and its
# folders. Clients here keep them under the thread's id here and read the
# cache's folder instead.

def Fv.fix.thread(+link: String, +v: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(v), FR.Fr.is(v)), v, FR.Fr.id(link, v))

def Fv.fix.root(+home: String, +link: String, +v: String) -> String:
  Bool.pick(String, String.starts_with(v, home ++ "/farfs/"), v,
    Bool.pick(String, String.is_empty(Fv.cache(home, link, v)), v, Fv.cache(home, link, v)))

def Fv.fix.res(fs: J.Json, +home: String, +link: String) -> J.Json:
  match fs:
    case J.Field{+k, J.Str{+v}, t}:
      J.Field{k, J.Str{Bool.pick(String, String.eq(k, "thread"), Fv.fix.thread(link, v),
        Bool.pick(String, Bool.or(String.eq(k, "root"), String.eq(k, "fmRoot")), Fv.fix.root(home, link, v), v))}, Fv.fix.res(t, home, link)}
    case J.Field{k, v, t}:
      J.Field{k, v, Fv.fix.res(t, home, link)}
    case x:
      x

def Fv.fix.val(res: Bool, v: J.Json, +home: String, +link: String) -> J.Json:
  match res v:
    case True{} J.Obj{rs}:
      J.Obj{Fv.fix.res(rs, home, link)}
    case True{} +x:
      x
    case False{} +x:
      x

def Fv.fix.top(fs: J.Json, +home: String, +link: String) -> J.Json:
  match fs:
    case J.Field{+k, +v, t}:
      J.Field{k, Fv.fix.val(String.eq(k, "result"), v, home, link), Fv.fix.top(t, home, link)}
    case x:
      x

def Fv.fix.o(m: Maybe<&2, J.Json>, +home: String, +link: String, +body: String) -> String:
  match m:
    case Some{J.Obj{fs}}:
      J.Json.show(J.Obj{Fv.fix.top(fs, home, link)})
    case _:
      body

# an owner's reply as clients here read it
def Fv.fix(+home: String, +link: String, +body: String) -> String:
  Fv.fix.o(J.Json.parse(body), home, link, body)

# What a client asks
# ------------------
# The files the viewers want of a thread elsewhere, kept in the cache: the
# one on show (its path from the folder; a Mech page's renders come with
# its listing, Fvs.mech).
# `ver` is the owner's digest of the last list it sent; a client sends it
# back and the owner answers when the list changes (or after a while,
# "same"), so the cache follows the owner's files with no polling here.
