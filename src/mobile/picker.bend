import Base
import ../core/i18n.bend as I
import ../core/msgs.bend as MSG
import ../core/json.bend as J
import ../core/client.bend as U

# The project picker on a phone: one field, empty when it opens, over the
# projects (core's U.Proj.list: the hub's own, and those it once had or
# found on disk), which the field filters. A path (~/..., /...) browses
# instead: the text up to its last / is the folder listed (the hub
# resolves ~), the rest filters that folder's subfolders. A tapped folder
# fills in its path. Listing, adding and creating are core's (U.Proj);
# the field is "@mpick" in the scratch, the folder last asked for
# "@mpick.asked", home (learned from the first listing of ~/) the info
# "proj.home".

def MPick.split.k(+ps: List<&2, String>, +text: String) -> String:
  Bool.pick(String, Nat.is_le(List.length(&2, String, ps), 1n), "~/",
    String.join(List.take(&2, String, ps, Nat.sub(List.length(&2, String, ps), 1n)), "/") ++ "/")

# the folder the field names: through its last /, home without one
def MPick.dir(+text: String) -> String:
  MPick.split.k(String.split(text, '/'), text)

def MPick.last(ps: List<&2, String>) -> String:
  match ps:
    case Nil{}:
      ""
    case x <> Nil{}:
      x
    case _ <> t:
      MPick.last(t)

# what filters the listed folder: after the last / ("~" alone is home)
def MPick.filter(+text: String) -> String:
  Bool.pick(String, String.eq(text, "~"), "", MPick.last(String.split(text, '/')))

# a path under home written with ~ (as is when home is not known yet)
def MPick.tilde(+home: String, +p: String) -> String:
  Bool.pick(String, Bool.and(Bool.not(String.is_empty(home)), Bool.or(String.eq(p, home), String.starts_with(p, home ++ "/"))),
    "~" ++ String.drop(p, String.length(home)), p)

# a folder as the field shows it: ~ form, ending in /
def MPick.field(+home: String, +p: String) -> String:
  U.Proj.join(MPick.tilde(home, p), "")

def MPick.list.k(same: Bool, +u: U.Ui, +dir: String) -> U.Act:
  match same:
    case True{}:
      U.Act{u, Nil{}}
    case False{}:
      U.Proj.rpc(U.Ui.scratch_set(u, "@mpick.asked", dir), "fs.list", [J.KV{"path", J.Str{dir}}])

# the field changed (typed, or a folder tapped): a new folder is listed,
# the same one only filtered
def MPick.type(+u: U.Ui, +text: String) -> U.Act:
  MPick.list.k(String.eq(MPick.dir(text), U.Ui.scratch(u, "@mpick.asked")), U.Ui.scratch_set(u, "@mpick", text), MPick.dir(text))

def MPick.blank(a: U.Act) -> U.Act:
  match a:
    case U.Act{u, cs}:
      U.Act{U.Ui.scratch_set(u, "@mpick", ""), cs}

# opens on the projects (asking the hub which it finds on disk), home
# listed afresh behind them
def MPick.open.k(+u: U.Ui) -> U.Act:
  U.Proj.find(MPick.blank(MPick.type(U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.flag_on(U.Proj.mode(u, ""), "proj"), "@mpick.asked", ""), "@mpick.follow", ""), "~/")))

# (the picker opens on the hub this phone is focused on, whichever machine
# it worked on last, and that machine's home is not this one's)
def MPick.open.on(away: Bool, +u: U.Ui) -> U.Act:
  match away:
    case True{}:
      MPick.open.k(U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(u, "proj.on", ""), "proj.home", ""), "proj.found", ""), "proj.dir", ""))
    case False{}:
      MPick.open.k(u)

def MPick.open(+u: U.Ui) -> U.Act:
  MPick.open.on(Bool.not(String.is_empty(U.Proj.on(u))), u)

# a new folder; the field follows into it once the hub has made it
def MPick.mkdir(+u: U.Ui, +path: String) -> U.Act:
  U.Ui.act(U.Ui.scratch_set(U.Ui.scratch_set(u, "@mpick.asked", ""), "@mpick.follow", "1"), "proj-mkdir", path)

def MPick.learn(yes: Bool, +u: U.Ui) -> U.Ui:
  match yes:
    case True{}:
      U.Ui.info_set(u, "proj.home", U.Proj.dir(u))
    case False{}:
      u

def MPick.follow.k(yes: Bool, +u: U.Ui) -> U.Ui:
  match yes:
    case True{}:
      U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(u, "@mpick", MPick.field(U.Ui.info(u, "proj.home"), U.Proj.dir(u))), "@mpick.asked",
        MPick.field(U.Ui.info(u, "proj.home"), U.Proj.dir(u))), "@mpick.follow", "")
    case False{}:
      u

# nothing asked is still out, and it did not fail
def MPick.answered(+u: U.Ui) -> Bool:
  Bool.and(String.is_empty(U.Ui.scratch(u, "@proj.rid")), Bool.and(String.is_empty(U.Ui.info(u, "proj.err")), Bool.not(String.is_empty(U.Proj.dir(u)))))

def MPick.follow(+u: U.Ui) -> U.Ui:
  MPick.follow.k(Bool.and(MPick.answered(u), String.eq(U.Ui.scratch(u, "@mpick.follow"), "1")), u)

# after a hub message: home from the listing of ~/, and the field into a
# folder just made
def MPick.seen(+u: U.Ui) -> U.Ui:
  MPick.follow(MPick.learn(Bool.and(Bool.and(MPick.answered(u), String.eq(U.Ui.scratch(u, "@mpick.asked"), "~/")), String.is_empty(U.Ui.info(u, "proj.home"))), u))

def MPick.row(+label: String, +action: String, +value: String, +kind: String) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(label)}, J.KV{"action", J.Json.str(action)}, J.KV{"value", J.Json.str(value)}, J.KV{"kind", J.Json.str(kind)}])

def MPick.rows(xs: List<&2, U.Scored>, +home: String, +dir: String) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case U.Scored{_, +n} <> t:
      MPick.row(n, "picker-type", MPick.field(home, U.Proj.join(dir, n)), "dir") <> MPick.rows(t, home, dir)

# the listed folder as a project (once)
def MPick.add(+l: I.Loc, +u: U.Ui, +home: String, +dir: String) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, U.Proj.is_project(U.Proj.ps(u), dir),
    [MPick.row(MSG.T.proj.exists(l, MPick.tilde(home, dir)), "", "", "off")],
    [MPick.row(MSG.T.proj.add(l, MPick.tilde(home, dir)), "proj-add", dir, "add")])

def MPick.up(+home: String, +dir: String) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, String.eq(dir, "/"), Nil{}, [MPick.row("..", "picker-type", MPick.field(home, U.Proj.parent(dir)), "up")])

def MPick.create(+l: I.Loc, +q: String, +dir: String, +ns: List<&2, String>) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, Bool.and(U.Proj.name_ok(q), Bool.not(U.Proj.exact(ns, String.trim(q)))),
    [MPick.row(MSG.T.proj.create(l, String.trim(q)), "proj-new", U.Proj.join(dir, String.trim(q)), "new"),
     MPick.row(MSG.T.proj.mkdir(l, String.trim(q)), "picker-mkdir", U.Proj.join(dir, String.trim(q)), "mkdir")], Nil{})

# (a match, not Bool.pick: only the list shown is made)
def MPick.items.q(empty: Bool, +l: I.Loc, +u: U.Ui, +home: String, +q: String, +dir: String, +ns: List<&2, String>) -> List<&2, J.Json>:
  match empty:
    case True{}:
      List.concat(&2, J.Json, [MPick.add(l, u, home, dir), MPick.up(home, dir), MPick.rows(U.Proj.scored(ns, q), home, dir)])
    case False{}:
      List.concat(&2, J.Json, [MPick.rows(List.sort(~U.Scored, ~U.Scored.before, U.Proj.scored(ns, q)), home, dir), MPick.create(l, q, dir, ns), MPick.add(l, u, home, dir)])

# no rows before the first listing, nor while the typed folder fails
def MPick.items.k(none: Bool, +l: I.Loc, +u: U.Ui, +q: String) -> List<&2, J.Json>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      MPick.items.q(String.is_empty(q), l, u, U.Ui.info(u, "proj.home"), q, U.Proj.dir(u), U.Proj.entries(u))

# the folders. No filter: add the listed folder (once), its parent, its
# subfolders. A filter: matching subfolders best first, then creating the
# typed name (only when no folder has it), then adding
def MPick.folders(+l: I.Loc, +u: U.Ui) -> List<&2, J.Json>:
  MPick.items.k(Bool.or(String.is_empty(U.Proj.dir(u)), Bool.not(String.is_empty(U.Ui.info(u, "proj.err")))), l, u, MPick.filter(U.Ui.scratch(u, "@mpick")))

def MPick.proj.rows(ps: List<&2, U.PItem>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case U.PItem{l, +a, v} <> t:
      MPick.row(l, a, v, Bool.pick(String, String.eq(a, "proj-pick"), "dir", "add")) <> MPick.proj.rows(t)

def MPick.browse(+l: I.Loc) -> J.Json:
  MPick.row(MSG.T.proj.browse(l), "picker-type", "~/", "dir")

def MPick.projects.k(empty: Bool, +l: I.Loc, +u: U.Ui, +q: String) -> List<&2, J.Json>:
  match empty:
    case True{}:
      MPick.browse(l) <> MPick.proj.rows(U.Proj.list.loc(l, u, q))
    case False{}:
      List.append(&2, J.Json, MPick.proj.rows(U.Proj.list.loc(l, u, q)), [MPick.browse(l)])

def MPick.items.m(path: Bool, +l: I.Loc, +u: U.Ui, +text: String) -> List<&2, J.Json>:
  match path:
    case True{}:
      MPick.folders(l, u)
    case False{}:
      MPick.projects.k(String.is_empty(String.trim(text)), l, u, String.trim(text))

# a field that starts a path browses folders
def MPick.is_path(+text: String) -> Bool:
  Bool.or(String.starts_with(text, "~"), String.starts_with(text, "/"))

# what the picker offers: the projects (adding from a folder first, or
# last under a filter), or for a path the folders (MPick.folders)
def MPick.machine(+l: I.Loc, +u: U.Ui) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, List.is_empty(&2, String, U.Proj.machines(u)), Nil{},
    [MPick.row(MSG.T.proj.on(l, U.Proj.on.name(u)), "proj-on", U.Proj.next(u), "machine")])

def MPick.items(+l: I.Loc, +u: U.Ui) -> List<&2, J.Json>:
  List.append(&2, J.Json, MPick.items.m(MPick.is_path(U.Ui.scratch(u, "@mpick")), l, u, U.Ui.scratch(u, "@mpick")), MPick.machine(l, u))

def MPick.json.l(open: Bool, +l: I.Loc, +u: U.Ui) -> J.Json:
  match open:
    case True{}:
      J.Json.obj([J.KV{"text", J.Json.str(U.Ui.scratch(u, "@mpick"))}, J.KV{"hint", J.Json.str(Bool.pick(String, MPick.is_path(U.Ui.scratch(u, "@mpick")), MSG.T.picker.hint.path(l), MSG.T.picker.hint.find(l)))},
        J.KV{"error", J.Json.str(U.Ui.info(u, "proj.err"))}, J.KV{"items", J.Json.arr(MPick.items(l, u))}])
    case False{}:
      J.Null{}

def MPick.json.k(open: Bool, +u: U.Ui) -> J.Json:
  MPick.json.l(open, U.Lang.loc(u), u)

# the picker as the phone draws it (null while closed), in the screen's language
def MPick.json.loc(+l: I.Loc, +u: U.Ui) -> J.Json:
  MPick.json.l(U.Ui.flag(u, "proj"), l, u)

def MPick.json(+u: U.Ui) -> J.Json:
  MPick.json.k(U.Ui.flag(u, "proj"), u)
