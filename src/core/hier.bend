import Base
import ./sexpr.bend as S
import ./schematic.bend as SC
import ./diff.bend as DF

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

# a line's row ahead of the rest (a blank line has none); the rest is read
# once and passed in, as Bool.pick would read it in both of its arms
def Hier.read.keep(empty: Bool, +l: String, rest: List<&2, Row>) -> List<&2, Row>:
  match empty:
    case True{}:
      rest
    case False{}:
      Hier.read.row(String.split(l, '\t')) <> rest

def Hier.read.go(ls: List<&2, String>) -> List<&2, Row>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> +t:
      Hier.read.keep(String.is_empty(l), l, Hier.read.go(t))

def Hier.read(+s: String) -> List<&2, Row>:
  # Base's split recurses once per character on the JS target. The shared
  # tail-recursive line reader also handles large sheet tables on phones.
  Hier.read.go(DF.Diff.lines(s))

def Hier.kids.sheets(k: Kids) -> List<&2, Sheet>:
  match k:
    case Kids{_, ss}:
      ss
