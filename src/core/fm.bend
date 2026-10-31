import Base
import ./diff.bend as DF

# The viewer's Files tab (pure): a thread's project folder listed a folder
# at a time (the hub's files.dir: "d|f <tab> size <tab> name" lines, folders
# first, hidden ones left out), what each file opens as in the viewer, and
# paths from the project's root that never leave it (laws fm_*).
#
# A board or schematic opens in the board viewer, a STEP or GLB in 3D, an
# image or a PDF in a view of its own; project notes open as read-only text.
# Anything else is listed only.

type FEnt is Data:
  FEnt{dir: Bool, size: Nat, name: String}

def Fm.digits.go(s: String, +n: Nat) -> Nat:
  match s:
    case SNil{}:
      n
    case SCon{Chr{+c}, t}:
      Fm.digits.go(t, Bool.pick(Nat, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Nat.add(Nat.mul(n, 10n), U32.to_nat((c - 48 : U32))), n))

# a size the listing wrote (digits; anything else is 0)
def Fm.num(s: String) -> Nat:
  Fm.digits.go(s, 0n)

def Fm.ent.put(none: Bool, e: FEnt, rest: List<&2, FEnt>) -> List<&2, FEnt>:
  match none:
    case True{}:
      rest
    case False{}:
      e <> rest

def Fm.ent.k(ps: List<&2, String>, rest: List<&2, FEnt>) -> List<&2, FEnt>:
  match ps:
    case +k <> +n <> +name:
      Fm.ent.put(List.is_empty(&2, String, name), FEnt{String.eq(k, "d"), Fm.num(n), String.join(name, "\t")}, rest)
    case _:
      rest

def Fm.parse.go(ls: List<&2, String>) -> List<&2, FEnt>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Fm.ent.k(String.split(l, '\t'), Fm.parse.go(t))

# the listing's entries, in its order
def Fm.parse(+text: String) -> List<&2, FEnt>:
  Fm.parse.go(DF.Diff.lines(text))

def Fm.ends(xs: List<&2, String>, +p: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.ends_with(p, x), Fm.ends(t, p))

def Fm.lower.c(+c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), (c + 32 : U32), c)

def Fm.lower.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      SCon{Chr{Fm.lower.c(c)}, Fm.lower.go(t)}

# a name's extension, lower case (a name is short: its last 12 characters)
def Fm.ext(+name: String) -> String:
  Fm.lower.go(String.drop(name, Nat.sub(String.length(name), 12n)))

def Fm.kind.k(+e: String) -> String:
  Bool.pick(String, String.ends_with(e, ".kicad_pcb"), "board",
    Bool.pick(String, String.ends_with(e, ".kicad_sch"), "schematic",
      Bool.pick(String, Fm.ends([".step", ".stp", ".glb"], e), "3d",
        Bool.pick(String, Fm.ends([".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"], e), "image",
          Bool.pick(String, String.ends_with(e, ".pdf"), "pdf", "")))))

# what a file opens as in the viewer: "board", "schematic", "3d", "image",
# "pdf", or "" (no design viewer)
def Fm.kind(+name: String) -> String:
  Fm.kind.k(Fm.ext(name))

# Files can read project notes as well as opening design viewers. Keep
# that extra preview separate from the design-file classification.
def Fm.preview(+name: String) -> String:
  Bool.pick(String, Fm.ends([".txt", ".md", ".markdown", ".csv", ".json", ".log", ".yaml", ".yml"], Fm.ext(name)), "text", Fm.kind(name))

# a path from the root: a folder's entry, the folder itself when "" is the root
def Fm.join(+dir: String, +name: String) -> String:
  Bool.pick(String, String.is_empty(dir), name, dir ++ "/" ++ name)

def Fm.last(+p: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, String.split(p, '/')), p)

# the folder above ("" above a folder at the root, and above the root)
def Fm.parent(+dir: String) -> String:
  String.take(dir, Nat.sub(Nat.sub(String.length(dir), String.length(Fm.last(dir))), Bool.pick(Nat, String.contains(dir, "/"), 1n, 0n)))

# a path the hub may list or serve from the project: from its root, no ".."
# part, never absolute
def Fm.safe.go(ps: List<&2, String>) -> Bool:
  match ps:
    case Nil{}:
      True{}
    case +p <> t:
      Bool.and(Bool.not(String.eq(p, "..")), Fm.safe.go(t))

def Fm.safe(+rel: String) -> Bool:
  Bool.and(Bool.not(String.starts_with(rel, "/")), Fm.safe.go(String.split(rel, '/')))

# a path from the root made absolute
def Fm.abs(+root: String, +rel: String) -> String:
  Bool.pick(String, String.is_empty(rel), root, root ++ "/" ++ rel)

# an absolute path back to one from the root ("" when it is not under it)
def Fm.rel(+root: String, +p: String) -> String:
  Bool.pick(String, String.starts_with(p, root ++ "/"), String.drop(p, 1n+String.length(root)), "")

def Fm.tenths(+n: Nat, +unit: Nat, +name: String) -> String:
  Nat.show(Nat.div(n, unit)) ++ "." ++ Nat.show(Nat.div(Nat.mod(n, unit), Nat.div(unit, 10n))) ++ " " ++ name

# a file's size as a person reads it
def Fm.size(+n: Nat) -> String:
  Bool.pick(String, Nat.is_lt(n, 1024n), Nat.show(n) ++ " B",
    Bool.pick(String, Nat.is_lt(n, 1048576n), Nat.show(Nat.div(Nat.add(n, 512n), 1024n)) ++ " KB",
      Bool.pick(String, Nat.is_lt(n, 1073741824n), Fm.tenths(n, 1048576n, "MB"), Fm.tenths(n, 1073741824n, "GB"))))

# the folder's name as a trail shows it ("Project" for the root)
def Fm.title(+dir: String) -> String:
  Bool.pick(String, String.is_empty(dir), "Project", Fm.last(dir))
