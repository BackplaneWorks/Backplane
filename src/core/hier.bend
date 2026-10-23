import Base
import ./sexpr.bend as S
import ./schematic.bend as SC

# A hierarchical schematic as a tree of sheets. Each .kicad_sch lists its
# child sheets (name and file, the file relative to the parent's folder).
# The window reads every file the root reaches (IO, main.bend's
# Watch.tree) into a table, and everything else happens here: the tree in
# pre-order as rows with a depth, where each row sits (its chain from the
# root), a sheet's parent and children. A sheet that includes one of its
# own ancestors is listed but never expanded (law hier_loop_listed_once),
# so a recursive design still gives a finite tree.

# a sheet as its parent names it
type Sheet is Data:
  Sheet{name: String, file: String}

# the sheets one file lists, its file resolved
type Kids is Data:
  Kids{file: String, sheets: List<&2, Sheet>}

# a tree row: depth (the root is 0), name, resolved file, and whether it
# loops back to an ancestor (then it has no rows under it)
type Row is Data:
  Row{depth: Nat, name: String, file: String, loop: Bool}

# Paths
# -----

def Hier.dir.go(s: String, +cur: String, +acc: String) -> String:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Hier.dir.go(t, Bool.pick(String, U32.is_eq(c, 47), "", cur ++ SCon{Chr{c}, SNil{}}),
        Bool.pick(String, U32.is_eq(c, 47), acc ++ cur ++ "/", acc))

# the folder of a path, with its trailing slash ("" for a bare name)
def Hier.dir(+path: String) -> String:
  Hier.dir.go(path, "", "")

def Hier.resolve.k(abs: Bool, +parent: String, +file: String) -> String:
  match abs:
    case True{}:
      file
    case False{}:
      Hier.dir(parent) ++ file

# a sheet's file as a path: absolute as written, else beside its parent
def Hier.resolve(+parent: String, +file: String) -> String:
  Hier.resolve.k(String.starts_with(file, "/"), parent, file)

# Reading sheets
# --------------

def Hier.sheet.k(hit: Bool, +bs: List<&2, U32>, rest: List<&2, Sheet>) -> List<&2, Sheet>:
  match hit:
    case True{}:
      +x = S.Sx.parse(bs)
      Sheet{SC.Sheet.prop(x, "Sheetname", "Sheet name"), SC.Sheet.prop(x, "Sheetfile", "Sheet file")} <> rest
    case False{}:
      rest

def Hier.sheets.go(fs: List<&2, List<&2, U32>>) -> List<&2, Sheet>:
  match fs:
    case Nil{}:
      Nil{}
    case +f <> t:
      Hier.sheet.k(String.eq(SC.Sch.head(f), "sheet"), f, Hier.sheets.go(t))

def Hier.sheets.k(m: Maybe<&2, List<&2, List<&2, U32>>>) -> List<&2, Sheet>:
  match m:
    case Some{fs}:
      Hier.sheets.go(fs)
    case None{}:
      Nil{}

# the child sheets a schematic file lists (only sheet forms are parsed)
def Hier.sheets(bs: List<&2, U32>) -> List<&2, Sheet>:
  Hier.sheets.k(S.Sx.forms(bs))

def Hier.abs(ss: List<&2, Sheet>, +parent: String) -> List<&2, Sheet>:
  match ss:
    case Nil{}:
      Nil{}
    case Sheet{n, +f} <> t:
      Sheet{n, Hier.resolve(parent, f)} <> Hier.abs(t, parent)

# a file's table entry: its sheets with their files resolved
def Hier.kids(+file: String, bs: List<&2, U32>) -> Kids:
  Kids{file, Hier.abs(Hier.sheets(bs), file)}

# Loading
# -------
# The IO side keeps a to-do list of files; each file read adds the files
# its sheets name that are neither read nor waiting.

def Hier.known(tab: List<&2, Kids>, +f: String) -> Bool:
  match tab:
    case Nil{}:
      False{}
    case Kids{+k, _} <> t:
      Bool.or(String.eq(k, f), Hier.known(t, f))

def Hier.has(fs: List<&2, String>, +f: String) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.eq(x, f), Hier.has(t, f))

def Hier.todo.k(fresh: Bool, +f: String, todo: List<&2, String>) -> List<&2, String>:
  match fresh:
    case True{}:
      List.append(&2, String, todo, [f])
    case False{}:
      todo

# the to-do list once a file's sheets are known
def Hier.todo(ss: List<&2, Sheet>, +tab: List<&2, Kids>, +todo: List<&2, String>) -> List<&2, String>:
  match ss:
    case Nil{}:
      todo
    case Sheet{_, +f} <> t:
      Hier.todo(t, tab, Hier.todo.k(Bool.not(Bool.or(Hier.known(tab, f), Hier.has(todo, f))), f, todo))

def Hier.find(tab: List<&2, Kids>, +f: String) -> List<&2, Sheet>:
  match tab:
    case Nil{}:
      Nil{}
    case Kids{+k, ss} <> t:
      Bool.pick(List<&2, Sheet>, String.eq(k, f), ss, Hier.find(t, f))

# Rows
# ----
# Rows come from a stack of sheets still to visit, each with its depth
# and the files above it: pop one, list it, push its children in front.

type Todo is Data:
  Todo{depth: Nat, name: String, file: String, above: List<&2, String>}

def Hier.push(ss: List<&2, Sheet>, +d: Nat, +above: List<&2, String>, rest: List<&2, Todo>) -> List<&2, Todo>:
  match ss:
    case Nil{}:
      rest
    case Sheet{n, f} <> t:
      Todo{d, n, f, above} <> Hier.push(t, d, above, rest)

def Hier.visit(loop: Bool, +tab: List<&2, Kids>, +d: Nat, +f: String, +above: List<&2, String>, rest: List<&2, Todo>) -> List<&2, Todo>:
  match loop:
    case True{}:
      rest
    case False{}:
      Hier.push(Hier.find(tab, f), 1n+d, f <> above, rest)

# pre-order rows, at most `fuel` of them
def Hier.walk(fuel: Nat, st: List<&2, Todo>, +tab: List<&2, Kids>) -> List<&2, Row>:
  match fuel st:
    case 0n _:
      Nil{}
    case _ Nil{}:
      Nil{}
    case 1n+k Todo{+d, +n, +f, +above} <> t:
      Row{d, n, f, Hier.has(above, f)} <> Hier.walk(k, Hier.visit(Hier.has(above, f), tab, d, f, above, t), tab)

def Hier.base(+f: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, String.split(f, '/')), f)

# the whole tree from the root file (the root is row 0, named by its file)
def Hier.rows(+tab: List<&2, Kids>, +root: String) -> List<&2, Row>:
  Hier.walk(4096n, [Todo{0n, Hier.base(root), root, Nil{}}], tab)

# Where a row sits
# ----------------

type At is Data:
  At{depth: Nat, i: Nat}

def Hier.pop(st: List<&2, At>, +d: Nat) -> List<&2, At>:
  match st:
    case Nil{}:
      Nil{}
    case At{+e, +j} <> +t:
      Bool.pick(List<&2, At>, Nat.is_ge(e, d), Hier.pop(t, d), At{e, j} <> t)

def Hier.idx(st: List<&2, At>, acc: List<&2, Nat>) -> List<&2, Nat>:
  match st:
    case Nil{}:
      acc
    case At{_, +j} <> t:
      Hier.idx(t, j <> acc)

def Hier.chain.go(rs: List<&2, Row>, +j: Nat, +i: Nat, +st: List<&2, At>) -> List<&2, Nat>:
  match rs:
    case Nil{}:
      Nil{}
    case Row{+d, _, _, _} <> t:
      Bool.pick(List<&2, Nat>, Nat.is_eq(j, i), Hier.idx(At{d, j} <> Hier.pop(st, d), Nil{}),
        Hier.chain.go(t, 1n+j, i, At{d, j} <> Hier.pop(st, d)))

# the rows from the root down to row i (empty when there is no row i)
def Hier.chain(+rs: List<&2, Row>, +i: Nat) -> List<&2, Nat>:
  Hier.chain.go(rs, 0n, i, Nil{})

# row i's parent (the root is its own)
def Hier.parent(+rs: List<&2, Row>, +i: Nat) -> Nat:
  Maybe.default(&2, Nat, List.last(&2, Nat, List.take(&2, Nat, Hier.chain(rs, i), Nat.sub(List.length(&2, Nat, Hier.chain(rs, i)), 1n))), 0n)

def Hier.row(+rs: List<&2, Row>, +i: Nat) -> Maybe<&2, Row>:
  List.head(&2, Row, List.drop(&2, Row, rs, i))

def Hier.depth.k(m: Maybe<&2, Row>) -> Nat:
  match m:
    case Some{Row{d, _, _, _}}:
      d
    case None{}:
      0n

def Hier.depth(+rs: List<&2, Row>, +i: Nat) -> Nat:
  Hier.depth.k(Hier.row(rs, i))

def Hier.file.k(m: Maybe<&2, Row>) -> String:
  match m:
    case Some{Row{_, _, f, _}}:
      f
    case None{}:
      ""

def Hier.file(+rs: List<&2, Row>, +i: Nat) -> String:
  Hier.file.k(Hier.row(rs, i))

def Hier.name.k(m: Maybe<&2, Row>) -> String:
  match m:
    case Some{Row{_, n, _, _}}:
      n
    case None{}:
      ""

def Hier.name(+rs: List<&2, Row>, +i: Nat) -> String:
  Hier.name.k(Hier.row(rs, i))

def Hier.child.go(rs: List<&2, Row>, +j: Nat, +d: Nat, +name: String) -> Maybe<&2, Nat>:
  match rs:
    case Nil{}:
      None{}
    case Row{+e, +n, _, _} <> t:
      Bool.pick(Maybe<&2, Nat>, Nat.is_le(e, d), None{},
        Bool.pick(Maybe<&2, Nat>, Bool.and(Nat.is_eq(e, 1n+d), String.eq(n, name)), Some{j}, Hier.child.go(t, 1n+j, d, name)))

# the row of the sheet called `name` directly under row i
def Hier.child(+rs: List<&2, Row>, +i: Nat, +name: String) -> Maybe<&2, Nat>:
  Hier.child.go(List.drop(&2, Row, rs, 1n+i), 1n+i, Hier.depth(rs, i), name)

def Hier.first.go(rs: List<&2, Row>, +j: Nat, +f: String) -> Nat:
  match rs:
    case Nil{}:
      0n
    case Row{_, _, +g, _} <> t:
      Bool.pick(Nat, String.eq(g, f), j, Hier.first.go(t, 1n+j, f))

def Hier.at.k(same: Bool, +rs: List<&2, Row>, +i: Nat, +file: String) -> Nat:
  match same:
    case True{}:
      i
    case False{}:
      Hier.first.go(rs, 0n, file)

# the row the viewer is on: row i while it shows row i's file, else the
# first row of the file on show (an agent may show any sheet), else the root
def Hier.at(+rs: List<&2, Row>, +i: Nat, +file: String) -> Nat:
  Hier.at.k(String.eq(Hier.file(rs, i), file), rs, i, file)

# Text
# ----
# The rows go to the window as text: a line per row, fields split by tabs.

def Hier.show.row(r: Row) -> String:
  match r:
    case Row{d, n, f, l}:
      Nat.show(d) ++ "\t" ++ Bool.pick(String, l, "1", "0") ++ "\t" ++ n ++ "\t" ++ f

def Hier.show(rs: List<&2, Row>) -> String:
  match rs:
    case Nil{}:
      ""
    case r <> Nil{}:
      Hier.show.row(r)
    case r <> t:
      Hier.show.row(r) ++ "\n" ++ Hier.show(t)

def Hier.field(+fs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, fs, i)), "")

def Hier.read.row(+fs: List<&2, String>) -> Row:
  Row{Maybe.default(&2, Nat, Nat.read(Hier.field(fs, 0n)), 0n), Hier.field(fs, 2n), Hier.field(fs, 3n), String.eq(Hier.field(fs, 1n), "1")}

def Hier.read.go(ls: List<&2, String>) -> List<&2, Row>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> +t:
      Bool.pick(List<&2, Row>, String.is_empty(l), Hier.read.go(t), Hier.read.row(String.split(l, '\t')) <> Hier.read.go(t))

def Hier.read(+s: String) -> List<&2, Row>:
  Hier.read.go(String.split(s, '\n'))

def Hier.kids.sheets(k: Kids) -> List<&2, Sheet>:
  match k:
    case Kids{_, ss}:
      ss

# Instances
# ---------
# A row's place, as KiCad names a sheet instance: the sheet names from the
# root down, "/" for the root, "/Power/LDO/" under it. A file used twice
# is two rows with two places, so the viewer keeps the instance, never
# only the file. (KiCad keeps a sheet's name unique under its parent; two
# siblings with one name are told apart by their row.)

type PAt is Data:
  PAt{depth: Nat, path: String}

def Hier.ppop(st: List<&2, PAt>, +d: Nat) -> List<&2, PAt>:
  match st:
    case Nil{}:
      Nil{}
    case PAt{+e, +q} <> +t:
      Bool.pick(List<&2, PAt>, Nat.is_ge(e, d), Hier.ppop(t, d), PAt{e, q} <> t)

def Hier.ptop(st: List<&2, PAt>) -> String:
  match st:
    case Nil{}:
      "/"
    case PAt{_, q} <> _:
      q

def Hier.place(+st: List<&2, PAt>, +d: Nat, +n: String) -> String:
  Bool.pick(String, Nat.is_eq(d, 0n), "/", Hier.ptop(Hier.ppop(st, d)) ++ n ++ "/")

def Hier.paths.go(rs: List<&2, Row>, +st: List<&2, PAt>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case Row{+d, +n, _, _} <> t:
      Hier.place(st, d, n) <> Hier.paths.go(t, PAt{d, Hier.place(st, d, n)} <> Hier.ppop(st, d))

# every row's place, in row order
def Hier.paths(rs: List<&2, Row>) -> List<&2, String>:
  Hier.paths.go(rs, Nil{})

# row i's place ("" when there is no row i)
def Hier.path(+rs: List<&2, Row>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, Hier.paths(rs), i)), "")

def Hier.seek.k(hit: Bool, +j: Nat, rest: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match hit:
    case True{}:
      Some{j}
    case False{}:
      rest

def Hier.seek.go(ps: List<&2, String>, +j: Nat, +p: String) -> Maybe<&2, Nat>:
  match ps:
    case Nil{}:
      None{}
    case q <> t:
      Hier.seek.k(String.eq(q, p), j, Hier.seek.go(t, 1n+j, p))

# the first row at a place
def Hier.seek(+rs: List<&2, Row>, +p: String) -> Maybe<&2, Nat>:
  Hier.seek.go(Hier.paths(rs), 0n, p)

def Hier.under.kid(kid: Bool, +j: Nat, rest: List<&2, Nat>) -> List<&2, Nat>:
  match kid:
    case True{}:
      j <> rest
    case False{}:
      rest

def Hier.under.k(stop: Bool, kid: Bool, +j: Nat, rest: List<&2, Nat>) -> List<&2, Nat>:
  match stop:
    case True{}:
      Nil{}
    case False{}:
      Hier.under.kid(kid, j, rest)

def Hier.under.go(rs: List<&2, Row>, +j: Nat, +d: Nat) -> List<&2, Nat>:
  match rs:
    case Nil{}:
      Nil{}
    case Row{+e, _, _, _} <> t:
      Hier.under.k(Nat.is_le(e, d), Nat.is_eq(e, 1n+d), j, Hier.under.go(t, 1n+j, d))

# the rows directly under row i, in the order its file lists their sheets
# (a row that loops back has none)
def Hier.under(+rs: List<&2, Row>, +i: Nat) -> List<&2, Nat>:
  Hier.under.go(List.drop(&2, Row, rs, 1n+i), 1n+i, Hier.depth(rs, i))

# Sheet boxes
# -----------
# Where each child sheet sits on its parent ("x0 y0 x1 y1", mm on the
# parent, as the plot's coordinates are), so a click on a sheet symbol
# opens that instance: the k-th row under a row is the k-th box of its
# file, both in the file's order of sheet forms.

def Hier.box.at(+at: S.Sx, +sz: S.Sx) -> String:
  F32.show(S.Sx.f(at, 0n)) ++ " " ++ F32.show(S.Sx.f(at, 1n)) ++ " " ++ F32.show(F32.add(S.Sx.f(at, 0n), S.Sx.f(sz, 0n))) ++ " " ++ F32.show(F32.add(S.Sx.f(at, 1n), S.Sx.f(sz, 1n)))

def Hier.box(+x: S.Sx) -> String:
  Hier.box.at(S.Sx.get(x, "at"), S.Sx.get(x, "size"))

def Hier.box.k(hit: Bool, +bs: List<&2, U32>, rest: List<&2, String>) -> List<&2, String>:
  match hit:
    case True{}:
      Hier.box(S.Sx.parse(bs)) <> rest
    case False{}:
      rest

def Hier.boxes.go(fs: List<&2, List<&2, U32>>) -> List<&2, String>:
  match fs:
    case Nil{}:
      Nil{}
    case +f <> t:
      Hier.box.k(String.eq(SC.Sch.head(f), "sheet"), f, Hier.boxes.go(t))

def Hier.boxes.k(m: Maybe<&2, List<&2, List<&2, U32>>>) -> List<&2, String>:
  match m:
    case Some{fs}:
      Hier.boxes.go(fs)
    case None{}:
      Nil{}

# the boxes of the child sheets a file lists, in its order
def Hier.boxes(bs: List<&2, U32>) -> List<&2, String>:
  Hier.boxes.k(S.Sx.forms(bs))

# a file's line: its key (the live file, core/hist.bend's family, which has
# no tabs), then each box, split by tabs
def Hier.boxes.line(+key: String, bs: List<&2, U32>) -> String:
  String.join(key <> Hier.boxes(bs), "\t")

def Hier.boxes.pick(hit: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match hit:
    case True{}:
      List.drop(&2, String, String.split(l, '\t'), 1n)
    case False{}:
      rest

def Hier.boxes.find(ls: List<&2, String>, +key: String) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Hier.boxes.pick(String.starts_with(l, key ++ "\t"), l, Hier.boxes.find(t, key))

# the boxes the lines name for a file's key, in its order
def Hier.boxes.of(+text: String, +key: String) -> List<&2, String>:
  Hier.boxes.find(String.split(text, '\n'), key)
