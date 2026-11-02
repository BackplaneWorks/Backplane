import Base
import ../src/core/fold.bend as F
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/lazy-tools.bend as L
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/core/json.bend as J

def changes() -> List<&2, M.Change>:
  [M.MessagePosted{"t", "m1", M.Assistant{}, "Visible message", 1n},
   M.ActivityLogged{"t", "a", "tool", "shell", "Inspect saved design\nsecret A", 2n},
   M.MessagePosted{"t", "blank", M.Assistant{}, " \n\t", 3n},
   M.ActivityLogged{"t", "b", "tool", "edit", "secret B", 4n},
   M.MessagePosted{"t", "m2", M.Assistant{}, "Another visible message", 5n},
   M.ActivityLogged{"t", "c", "tool", "read", "secret C", 6n}]

def state() -> M.State:
  M.State.replay(changes(), M.State.empty())

def request(+action: String, +value: String, +info: J.Json) -> J.Json:
  J.Json.obj([J.KV{"action", J.Str{action}}, J.KV{"value", J.Str{value}}, J.KV{"session", J.Json.obj([J.KV{"info", info}])}])

def allowed(id: String, p: J.Json) -> String:
  +st = state()
  +index = T.Index.of(st)
  Bool.pick(String, L.Tools.found(Map.get(Bool, False{}, L.Tools.allowed(st, index, "t", T.Index.page(index, "t", 0n, 32n), p), id)), "yes", "no")

def payload(+id: String, ids: List<&2, String>, +index: T.Index) -> String:
  match ids:
    case Nil{}: "missing"
    case ordinal <> rest:
      +e = T.Row.entry(T.Index.row(index, M.Switch.num(ordinal, 0n)))
      Bool.pick(String, String.eq(M.Idx.id(e), id), T.Entry.text(e), payload(id, rest, index))

def masked(+id: String, p: J.Json) -> String:
  +st = state()
  +index = T.Index.of(st)
  +ids = T.Index.page(index, "t", 0n, 32n)
  payload(id, ids, L.Tools.mask(st, index, "t", ids, p))

def edge(id: String, +before: Nat, p: J.Json) -> String:
  +st = state()
  +index = T.Index.of(st)
  Bool.pick(String, L.Tools.found(Map.get(Bool, False{}, L.Tools.allowed(st, index, "t", T.Index.page(index, "t", before, 1n), p), id)), "yes", "no")

def label.original(+kind: String, +text: String) -> String:
  F.Fold.label.live(I.En{}, [M.Act{"label", "t", "tool", kind, text, 1n}])

def label.metadata(+kind: String, +text: String) -> String:
  F.Fold.label.live(I.En{}, [T.Row.entry(L.Tools.metadata.row(False{}, T.Row{1n, M.Act{"label", "t", "tool", kind, text, 1n}, Map.new(&2, Bool), T.Blocks.of(text)}))])

def metadata.entries(es: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match es:
    case Nil{}: Nil{}
    case +e <> rest:
      T.Row.entry(L.Tools.metadata.row(Bool.not(L.Tools.tool(e)), T.Row{1n, e, Map.new(&2, Bool), T.Blocks.of(T.Entry.text(e))})) <> metadata.entries(rest)

def role.label(r: F.Role) -> String:
  match r:
    case F.RShow{}: "show"
    case F.RIn{open}: "in:" ++ Bool.show(open)
    case F.RHead{id, text, open, live}: id ++ ":" ++ text ++ ":" ++ Bool.show(open) ++ ":" ++ Bool.show(live)
    case F.RWork{id, text, open, inner}: id ++ ":" ++ text ++ ":" ++ Bool.show(open) ++ ":" ++ role.label(inner)

def roles.labels(rs: List<&2, F.Role>) -> String:
  match rs:
    case Nil{}: ""
    case r <> rest: role.label(r) ++ "|" ++ roles.labels(rest)

def entries.labels(+es: List<&2, M.Entry>, +info: Map<&2, String>, +running: Bool) -> String:
  roles.labels(F.Work.zip(F.Work.roles.go(I.En{}, F.Work.segs(es), info, running, 20n), F.Fold.roles.all(I.En{}, F.Around.none(), es, info, running)))

def label.turn() -> List<&2, M.Entry>:
  [M.Msg{"u", "t", M.User{}, "Inspect the design", 1n},
   M.Act{"tool-a", "t", "tool", "shell", "  kicad-cli pcb drc /private/path/design.kicad_pcb\nprivate body", 2n},
   M.Act{"tool-b", "t", "tool", "read", "Inspect saved schematic\nprivate body", 3n},
   M.Msg{"mid", "t", M.Assistant{}, "Committed explanation", 4n},
   M.Act{"tool-c", "t", "tool", "edit", "Apply corrected pad geometry\nprivate body", 5n}]

def main() -> IO(Unit):
  do IO<Unit>:
    +closed : J.Json = request("", "", J.Json.obj(Nil{}))
    C.Check.str("default fold materializes no tool detail", allowed("a", closed), "no")
    C.Check.str("the second closed run materializes no tool detail", allowed("c", closed), "no")
    C.Check.str("message text remains available immediately", allowed("m1", closed), "yes")
    +opening : J.Json = request("fold", "1a", J.Json.obj(Nil{}))
    C.Check.str("explicit expansion reads its first tool", allowed("a", opening), "yes")
    C.Check.str("blank assistant commits do not split the opened run", allowed("b", opening), "yes")
    C.Check.str("expanding one run does not read another", allowed("c", opening), "no")
    C.Check.str("collapse defeats an older persisted open flag", allowed("a", request("fold", "0a", J.Json.obj([J.KV{"fold:a", J.Str{"1"}}]))), "no")
    C.Check.str("reconnect preserves the explicit open choice", allowed("a", request("", "", J.Json.obj([J.KV{"fold:a", J.Str{"1"}}]))), "yes")
    C.Check.str("arbitrary other fold IDs do not reveal a body", allowed("a", request("fold", "1other", J.Json.obj(Nil{}))), "no")
    C.Check.str("closed masking keeps only the visible title", masked("a", closed), "Inspect saved design")
    C.Check.str("masking keeps committed messages", masked("m1", closed), "Visible message")
    C.Check.str("explicit expansion keeps its selected payload", masked("a", opening), "Inspect saved design\nsecret A")
    C.Check.str("an interior page uses the off-page real first call", edge("b", 5n, opening), "yes")
    C.Check.str("an interior page stays closed by default", edge("b", 5n, closed), "no")
    C.Check.str("a fabricated interior first ID cannot open the actual run", edge("b", 5n, request("fold", "1b", J.Json.obj(Nil{}))), "no")
    C.Check.str("closed live label keeps the summarized command", label.metadata("shell", "  make drc\nprivate output"), label.original("shell", "  make drc\nprivate output"))
    C.Check.str("closed live label keeps the translated todo kind", label.metadata("TodoWrite", "1 of 3 done\nprivate output"), label.original("TodoWrite", "1 of 3 done\nprivate output"))
    C.Check.str("closed live label keeps the empty-summary fallback", label.metadata("mcp__backplane__viewer_show", ""), label.original("mcp__backplane__viewer_show", ""))
    C.Check.str("closed metadata has no detail blocks", Nat.show(T.Blocks.n(T.Row.blocks(L.Tools.metadata.row(False{}, T.Row{1n, M.Act{"label", "t", "tool", "shell", "Title\nprivate output", 1n}, Map.new(&2, Bool), T.Blocks.of("Title\nprivate output")})))), "1")
    C.Check.str("closed live label keeps a path and multiline boundary", label.metadata("shell", "  kicad-cli pcb drc /private/path/design.kicad_pcb\nprivate body"), label.original("shell", "  kicad-cli pcb drc /private/path/design.kicad_pcb\nprivate body"))
    C.Check.str("closed live label matches the 160-scalar truncation", label.metadata("shell", String.repeat("測", 180n) ++ "\nprivate body"), label.original("shell", String.repeat("測", 180n) ++ "\nprivate body"))
    C.Check.str("ordinary closed fold labels and counts stay exact", entries.labels(metadata.entries(label.turn()), Map.new(&2, String), False{}), entries.labels(label.turn(), Map.new(&2, String), False{}))
    C.Check.str("working and nested run labels stay exact", entries.labels(metadata.entries(label.turn()), Map.set(&2, String, Map.new(&2, String), "fold:work:u", "1"), True{}), entries.labels(label.turn(), Map.set(&2, String, Map.new(&2, String), "fold:work:u", "1"), True{}))
    C.Check.str("closed live label matches CRLF trimming", label.metadata("shell", "  make drc\r\nprivate body"), label.original("shell", "  make drc\r\nprivate body"))
