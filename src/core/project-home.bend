import Base
import ./json.bend as J

# A project route has a viewer/draft owner, but never a synthetic Thread.
def Home.owner(project: String) -> String:
  "project-home-" ++ project

def Home.is(+owner: String) -> Bool:
  String.starts_with(owner, "project-home-")

def Home.project(owner: String) -> String:
  String.drop(owner, 13n)

# Artifact panels are constructed only from resolved, existing file references.
# IO supplies the versioned contents; this projection has no effects or clock.
type Panel is Data:
  Panel{kind: String, path: String, title: String, summary: String}

def Home.panel.missing(missing: Bool, kind: String, path: String, title: String, summary: String) -> List<&2, Panel>:
  match missing:
    case True{}: Nil{}
    case False{}: [Panel{kind, path, title, summary}]

def Home.panel(+kind: String, +path: String, +title: String, +summary: String) -> List<&2, Panel>:
  Home.panel.missing(String.is_empty(path), kind, path, title, summary)

def Home.artifact(+row: J.Json) -> List<&2, Panel>:
  Home.panel(J.Json.get_str(row, "kind", ""), J.Json.get_str(row, "path", ""), J.Json.get_str(row, "title", ""), J.Json.get_str(row, "summary", ""))

def Home.panels.rows(rows: List<&2, J.Json>, left: Nat) -> List<&2, Panel>:
  match rows left:
    case _ 0n: Nil{}
    case Nil{} _: Nil{}
    case row <> rest 1n+n:
      List.append(&2, Panel, Home.artifact(row), Home.panels.rows(rest, n))

# the copies of a hardware project's design this page does not show (the
# hub's "outside", core/projkind.bend's Pk.panel), first, since they may
# hold the current design; none when there are none
def Home.outside(+text: String) -> List<&2, Panel>:
  Home.panel.missing(String.is_empty(text), "outside", "outside", "Design outside this folder", text)

def Home.panels(+contents: J.Json) -> List<&2, Panel>:
  List.append(&2, Panel, Home.outside(J.Json.get_str(contents, "outside", "")), Home.panels.rows(J.Json.elems(J.Json.get_obj(contents, "artifacts")), 32n))

# Exact path lookup supplies viewer actions as well as the panel plan.
def Home.find(rows: List<&2, J.Json>, +kind: String) -> String:
  match rows:
    case Nil{}: ""
    case +row <> rest:
      Bool.pick(String, String.eq(J.Json.get_str(row, "kind", ""), kind), J.Json.get_str(row, "path", ""), Home.find(rest, kind))

def Home.path(contents: J.Json, kind: String) -> String:
  Home.find(J.Json.elems(J.Json.get_obj(contents, "artifacts")), kind)

def Home.can_split(+contents: J.Json) -> Bool:
  Bool.and(Bool.not(String.is_empty(Home.path(contents, "board"))), Bool.not(String.is_empty(Home.path(contents, "schematic"))))

# A combined viewer always anchors its board and its explicit root schematic
# in this saved snapshot, even when their filenames differ.
def Home.view_path(+contents: J.Json, kind: String) -> String:
  match kind:
    case "split": Bool.pick(String, Home.can_split(contents), Home.path(contents, "board"), "")
    case "3d": Home.path(contents, "board")
    case _: Home.path(contents, kind)

# Message ids used as durable creation keys must also be safe path segments.
def Home.token.chars(s: String) -> Bool:
  match s:
    case SNil{}: True{}
    case SCon{Chr{+c}, tail}:
      Bool.and(Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))),
        Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 95)))), Home.token.chars(tail))

def Home.token(+s: String) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(s)), Nat.is_le(String.length(s), 160n)), Home.token.chars(s))

def Home.row.find(rows: List<&2, J.Json>, +path: String) -> J.Json:
  match rows:
    case Nil{}: J.Null{}
    case +row <> rest:
      Bool.pick(J.Json, String.eq(J.Json.get_str(row, "path", ""), path), row, Home.row.find(rest, path))

def Home.row(contents: J.Json, path: String) -> J.Json:
  Home.row.find(J.Json.elems(J.Json.get_obj(contents, "artifacts")), path)

def Home.has_path(+contents: J.Json, +path: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(path)), String.eq(J.Json.get_str(Home.row(contents, path), "path", ""), path))
