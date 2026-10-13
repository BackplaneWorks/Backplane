import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/bot.bend as BT
import ../src/core/bothub.bend as BH
import ../src/core/mcp.bend as MC
import ../src/core/ledger.bend as LG
import ../src/core/told.bend as TD
import ../src/core/tboard.bend as TB
import ../src/core/tbdata.bend as TT

# The task board (tboard.bend), and the ledger tools every agent has
# (ledger.bend): told_check, told_mark and task_board_*.

def yes(+b: Bool) -> String:
  match b:
    case True{}:
      "yes"
    case False{}:
      "no"

def has(+s: String, +part: String) -> String:
  yes(String.contains(s, part))

def run(+s: M.State, +st: B.Step) -> M.State:
  match st:
    case B.Step{cs, _}:
      M.State.replay(cs, s)

def more(+s: M.State, +cs: List<&2, M.Change>) -> M.State:
  M.State.replay(cs, s)

def req(+m: String, +p: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"id\":1,\"m\":\"" ++ m ++ "\",\"p\":" ++ p ++ "}"), J.Null{})

def made(+s: M.State, +now: Nat, +name: String) -> M.State:
  run(s, BH.Bots.request(s, now, 1, req("bots.create", "{\"name\":\"" ++ name ++ "\",\"provider\":\"claude\"}")))

def th.of(+s: M.State, +name: String) -> String:
  BT.Bot.thread(Maybe.default(&2, BT.Bot, BT.World.find(M.State.bots(s), name), BT.Bot.blank("?")))

def id.of(+s: M.State, +name: String) -> String:
  BT.Bot.id(Maybe.default(&2, BT.Bot, BT.World.find(M.State.bots(s), name), BT.Bot.blank("?")))

def n(+xs: List<&2, TT.TbItem>) -> String:
  Nat.show(List.length(&2, TT.TbItem, xs))

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "Power board", "/tmp/power", 900n}, M.ProjectCreated{"p2", "Backplane", "/tmp/bp", 901n}, M.ThreadCreated{"t-pb", "p1", "Rev C review", M.Local{}, "", "", "claude", "", 1000n}],
    made(made(M.State.empty(), 1000n, "Miso"), 1001n, "Tofu"))

def titles(+xs: List<&2, TT.TbItem>) -> String:
  match xs:
    case Nil{}:
      ""
    case t <> r:
      TB.Tb.title(t) ++ "|" ++ titles(r)

def blank() -> TT.TbItem:
  TB.Tb.blank("?", 0n)

def by.title.go(+xs: List<&2, TT.TbItem>, +title: String) -> TT.TbItem:
  match xs:
    case Nil{}:
      blank()
    case +t <> r:
      Bool.pick(TT.TbItem, String.eq(TB.Tb.title(t), title), t, by.title.go(r, title))

def by.title(+s: M.State, +title: String) -> TT.TbItem:
  by.title.go(TB.Tb.all(s), title)

def line(+t: TT.TbItem) -> String:
  TB.Tb.status(t) ++ "/" ++ TB.Tb.group(t) ++ "/" ++ TB.Tb.due(t) ++ "/" ++ TB.Tb.project(t) ++ "/" ++ TB.Tb.owner(t)

def imp.cs(+r: TB.TbImp) -> List<&2, M.Change>:
  match r:
    case TB.TbImp{_, _, _, cs, _}:
      cs

def imp.added(+r: TB.TbImp) -> Nat:
  match r:
    case TB.TbImp{a, _, _, _, _}:
      a

def imp.dups(+r: TB.TbImp) -> Nat:
  match r:
    case TB.TbImp{_, d, _, _, _}:
      d

def imp.skipped(+r: TB.TbImp) -> String:
  match r:
    case TB.TbImp{_, _, sk, _, _}:
      String.join(sk, " / ")

def imp.err(+r: TB.TbImp) -> String:
  match r:
    case TB.TbImp{_, _, _, _, e}:
      e

def imported(+s: M.State, +now: Nat, +md: String) -> M.State:
  more(s, imp.cs(TB.Tb.import(s, now, "person", md, "", "")))

def ok.of(+r: TB.TbR) -> String:
  match r:
    case TB.TbOk{id, _}:
      "ok"
    case TB.TbErr{m}:
      m

def cs.of(+r: TB.TbR) -> List<&2, M.Change>:
  match r:
    case TB.TbOk{_, cs}:
      cs
    case TB.TbErr{_}:
      Nil{}

def id.of.r(+r: TB.TbR) -> String:
  match r:
    case TB.TbOk{i, _}:
      i
    case TB.TbErr{_}:
      ""

def pj(+t: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{})

def added(+s: M.State, +now: Nat, +o: String) -> M.State:
  more(s, cs.of(TB.Tb.add(s, now, "person", pj(o))))

# a tool call as the agent of `thread` makes it: the reply, the answer's
# JSON text, and the changes
def mcp(+s: M.State, +thread: String, +tool: String, +args: String) -> MC.McpOut:
  LG.Ledger.call(s, 5000n, thread, J.Num{"7"}, pj("{\"name\":\"" ++ tool ++ "\",\"arguments\":" ++ args ++ "}"))

def reply(+s: M.State, +thread: String, +tool: String, +args: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(MC.McpOut.reply(mcp(s, thread, tool, args))), J.Null{})

def failed(+s: M.State, +thread: String, +tool: String, +args: String) -> Bool:
  J.Json.get_bool(J.Json.get_obj(reply(s, thread, tool, args), "result"), "isError", False{})

def text(+s: M.State, +thread: String, +tool: String, +args: String) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(reply(s, thread, tool, args), "result"), "content"))), J.Null{}), "text", "")

def ans(+s: M.State, +thread: String, +tool: String, +args: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text(s, thread, tool, args)), J.Null{})

def step.cs(+o: MC.McpOut) -> List<&2, M.Change>:
  match o:
    case MC.McpOut{_, B.Step{cs, _}, _}:
      cs

def after(+s: M.State, +thread: String, +tool: String, +args: String) -> M.State:
  more(s, step.cs(mcp(s, thread, tool, args)))

def sample() -> String:
  "# Todo board (Bub, chief of staff)\nImported 2026-09-26 ...\n## Work / hardware\n- [ ] PoE in PVC pipe (existing Backplane thread / PoE_LoRa_Tube project)\n- [~] FioMet CAD / BiometCat (Mon 10am reminder)\n## Outbound (email / invoices / admin)\n- [x] Invoices: June–Aug re-sent to Scott 10/12\n- [ ] Kane Hsieh: lunch in ATL Wed 10/28 12–1 ET (due 2026-10-28)\n## Never nag (email)\n- rsync.net account de4835\n## Done recently\nAaron @ Atmospheric Intelligence: Kostas scheduled it"

def rep(+st: B.Step) -> String:
  match st:
    case B.Step{_, B.Send{_, text} <> _}:
      text
    case _:
      "none"

def many(+k: Nat, +s: M.State) -> M.State:
  match k:
    case 0n:
      s
    case 1n+j:
      many(j, M.State.replay([M.SettingSet{"board.m" ++ Nat.show(j), "{\"title\":\"t" ++ Nat.show(j) ++ "\",\"status\":\"todo\",\"due\":\"2026-10-" ++ Nat.show(Nat.add(10n, Nat.mod(j, 18n))) ++ "\",\"at\":" ++ Nat.show(j) ++ "}", 1n}], s))

def main() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = base()
    +miso : String = th.of(s, "Miso")
    +tofu : String = th.of(s, "Tofu")
    # due dates
    C.Check.str("due ok", yes(TB.Tb.date("2026-10-28")), "yes")
    C.Check.str("due 10/28", yes(TB.Tb.date("10/28")), "no")
    C.Check.str("due month 13", yes(TB.Tb.date("2026-13-01")), "no")
    C.Check.str("due day 00", yes(TB.Tb.date("2026-10-00")), "no")
    C.Check.str("due letters", yes(TB.Tb.date("2026-1a-28")), "no")
    C.Check.str("due long", yes(TB.Tb.date("2026-10-281")), "no")
    # the import from the spec
    +i1 : M.State = imported(s, 2000n, "## Errands\n- [ ] Peach Pass\n- [x] floss\n- plain")
    C.Check.str("import: two items", n(TB.Tb.all(i1)), "2")
    C.Check.str("import: todo", line(by.title(i1, "Peach Pass")), "todo/Errands///")
    C.Check.str("import: done", line(by.title(i1, "floss")), "done/Errands///")
    C.Check.str("import: skipped", imp.skipped(TB.Tb.import(s, 2000n, "person", "## Errands\n- [ ] Peach Pass\n- [x] floss\n- plain", "", "")), "- plain")
    C.Check.str("import: counts", Nat.show(imp.added(TB.Tb.import(s, 2000n, "person", "## Errands\n- [ ] Peach Pass\n- [x] floss\n- plain", "", ""))), "2")
    # idempotent
    +again : TB.TbImp = TB.Tb.import(i1, 2100n, "person", "## Errands\n- [ ] Peach Pass\n- [x] floss\n- plain", "", "")
    C.Check.str("idempotent: nothing added", Nat.show(imp.added(again)), "0")
    C.Check.str("idempotent: duplicates", Nat.show(imp.dups(again)), "2")
    C.Check.str("idempotent: no changes", Nat.show(List.length(&2, M.Change, imp.cs(again))), "0")
    C.Check.str("idempotent: still two", n(TB.Tb.all(more(i1, imp.cs(again)))), "2")
    C.Check.str("duplicate by case and spaces", Nat.show(imp.dups(TB.Tb.import(i1, 2100n, "person", "## Errands\n- [ ]   peach   PASS", "", ""))), "1")
    C.Check.str("same title in another group is new", Nat.show(imp.added(TB.Tb.import(i1, 2100n, "person", "## Other\n- [ ] Peach Pass", "", ""))), "1")
    C.Check.str("duplicate within one import", Nat.show(imp.dups(TB.Tb.import(s, 2100n, "person", "- [ ] a thing\n- [ ] A thing", "", ""))), "1")
    # the real file's excerpt
    +real : M.State = imported(s, 2000n, sample())
    C.Check.str("sample: four items", n(TB.Tb.all(real)), "4")
    C.Check.str("sample: todo", line(by.title(real, "PoE in PVC pipe (existing Backplane thread / PoE_LoRa_Tube project)")), "todo/Work / hardware///")
    C.Check.str("sample: doing", line(by.title(real, "FioMet CAD / BiometCat (Mon 10am reminder)")), "doing/Work / hardware///")
    C.Check.str("sample: done, non-ascii", line(by.title(real, "Invoices: June–Aug re-sent to Scott 10/12")), "done/Outbound (email / invoices / admin)///")
    C.Check.str("sample: due taken out of the title", line(by.title(real, "Kane Hsieh: lunch in ATL Wed 10/28 12–1 ET")), "todo/Outbound (email / invoices / admin)/2026-10-28//")
    C.Check.str("sample: skipped three", imp.skipped(TB.Tb.import(s, 2000n, "person", sample(), "", "")), "Imported 2026-09-26 ... / - rsync.net account de4835 / Aaron @ Atmospheric Intelligence: Kostas scheduled it")
    C.Check.str("sample: again adds nothing", Nat.show(imp.added(TB.Tb.import(real, 2200n, "person", sample(), "", ""))), "0")
    # checkbox kinds
    +kinds : M.State = imported(s, 2000n, "- [ ] a1\n* [x] a2\n+ [X] a3\n1. [~] a4\n12. [/] a5\n- [!] a6\n- [-] a7\n- [?] a8\n-[ ] a9\n- [ ]\n- [ ]x")
    C.Check.str("kinds: seven", n(TB.Tb.all(kinds)), "7")
    C.Check.str("kind todo", TB.Tb.status(by.title(kinds, "a1")), "todo")
    C.Check.str("kind done *", TB.Tb.status(by.title(kinds, "a2")), "done")
    C.Check.str("kind done + X", TB.Tb.status(by.title(kinds, "a3")), "done")
    C.Check.str("kind doing numbered", TB.Tb.status(by.title(kinds, "a4")), "doing")
    C.Check.str("kind doing /", TB.Tb.status(by.title(kinds, "a5")), "doing")
    C.Check.str("kind blocked", TB.Tb.status(by.title(kinds, "a6")), "blocked")
    C.Check.str("kind dropped", TB.Tb.status(by.title(kinds, "a7")), "dropped")
    C.Check.str("kinds skipped", imp.skipped(TB.Tb.import(s, 2000n, "person", "- [ ] a1\n- [?] a8\n-[ ] a9\n- [ ]\n- [ ]x", "", "")), "- [?] a8 / -[ ] a9 / - [ ] / - [ ]x")
    # due and notes
    +dn : M.State = imported(s, 2000n, "- [ ] pay rent due:2026-11-01\n  autopay is off\n    check the card\n- [ ] file taxes (due 2027-04-15)\n- [ ] odd due 2026-13-45\n\n  stray line\npara\n  after para")
    C.Check.str("due: colon form", TB.Tb.due(by.title(dn, "pay rent")), "2026-11-01")
    C.Check.str("due: paren form", TB.Tb.due(by.title(dn, "file taxes")), "2027-04-15")
    C.Check.str("due: bad date stays in the title", TB.Tb.due(by.title(dn, "odd due 2026-13-45")), "")
    C.Check.str("notes: indented lines", TB.Tb.notes(by.title(dn, "pay rent")), "autopay is off\ncheck the card")
    C.Check.str("notes: not after a paragraph", imp.skipped(TB.Tb.import(s, 2000n, "person", "- [ ] x\n\npara\n  after para", "", "")), "para / after para")
    C.Check.str("notes: after a blank line still attach", TB.Tb.notes(by.title(imported(s, 2000n, "- [ ] x\n\n  more about x"), "x")), "more about x")
    # headings and projects
    +hp : M.State = imported(s, 2000n, "# Power board\n- [ ] route clocks\n## Backplane\n- [ ] ship it\n### Misc\n- [ ] loose end")
    C.Check.str("heading names a project", TB.Tb.project(by.title(hp, "route clocks")), "p1")
    C.Check.str("next heading names another", TB.Tb.project(by.title(hp, "ship it")), "p2")
    C.Check.str("a heading naming none clears it", TB.Tb.project(by.title(hp, "loose end")), "")
    C.Check.str("group is the heading", TB.Tb.group(by.title(hp, "loose end")), "Misc")
    +fp : M.State = more(s, imp.cs(TB.Tb.import(s, 2000n, "person", "# Backplane\n- [ ] ship it", "Power board", "Tofu")))
    C.Check.str("a given project wins", TB.Tb.project(by.title(fp, "ship it")), "p1")
    C.Check.str("a given owner", TB.Tb.owner(by.title(fp, "ship it")), id.of(s, "Tofu"))
    C.Check.str("unknown project in an import", imp.err(TB.Tb.import(s, 2000n, "person", "- [ ] x", "Nope", "")), "no project \"Nope\"")
    C.Check.str("unknown owner in an import", imp.err(TB.Tb.import(s, 2000n, "person", "- [ ] x", "", "Ghost")), "no bot \"Ghost\" here")
    C.Check.str("an error imports nothing", Nat.show(List.length(&2, M.Change, imp.cs(TB.Tb.import(s, 2000n, "person", "- [ ] x", "Nope", "")))), "0")
    # order
    +o : M.State = added(added(added(added(s, 3000n, "{\"title\":\"c\",\"status\":\"done\"}"), 3001n, "{\"title\":\"b\",\"due\":\"2026-12-01\"}"), 3002n, "{\"title\":\"a\"}"), 3003n, "{\"title\":\"d\",\"due\":\"2026-11-01\"}")
    +o2 : M.State = added(added(o, 3004n, "{\"title\":\"e\",\"status\":\"doing\"}"), 3005n, "{\"title\":\"f\",\"status\":\"dropped\"}")
    C.Check.str("order: doing, due, no due, done, dropped", titles(TB.Tb.order(TB.Tb.all(o2))), "e|d|b|a|c|f|")
    C.Check.str("order: open before closed", yes(String.starts_with(titles(TB.Tb.order(TB.Tb.all(o))), "d|b|a|c")), "yes")
    C.Check.str("open", titles(TB.Tb.order(TB.Tb.open(TB.Tb.all(o2)))), "e|d|b|a|")
    C.Check.str("filter status", titles(TB.Tb.order(TB.Tb.filter(TB.Tb.all(o2), "done, dropped", "", "", ""))), "c|f|")
    C.Check.str("filter none", n(TB.Tb.filter(TB.Tb.all(o2), "", "", "", "")), "6")
    C.Check.str("filter group", n(TB.Tb.filter(TB.Tb.all(more(o2, imp.cs(TB.Tb.import(o2, 3100n, "person", "## G\n- [ ] g1", "", "")))), "", "", "", "G")), "1")
    C.Check.str("filter person", n(TB.Tb.filter(TB.Tb.all(o2), "", "person", "", "")), "6")
    C.Check.str("filter an owner", n(TB.Tb.filter(TB.Tb.all(o2), "", id.of(s, "Miso"), "", "")), "0")
    # a merge sort: a few hundred items
    C.Check.str("order: many", Nat.show(List.length(&2, TT.TbItem, TB.Tb.order(TB.Tb.all(many(300n, s))))), "300")
    # update merges
    +u0 : M.State = added(s, 3000n, "{\"title\":\"Fix it\",\"notes\":\"a note\",\"group\":\"G\",\"links\":\"http://x http://y,/p\",\"due\":\"2026-11-02\"}")
    +uid : String = TB.Tb.id(by.title(u0, "Fix it"))
    +u1 : M.State = more(u0, cs.of(TB.Tb.update(u0, 3100n, "person", uid, pj("{\"status\":\"doing\"}"))))
    C.Check.str("update: status changed", TB.Tb.status(by.title(u1, "Fix it")), "doing")
    C.Check.str("update: title kept", TB.Tb.title(by.title(u1, "Fix it")), "Fix it")
    C.Check.str("update: notes kept", TB.Tb.notes(by.title(u1, "Fix it")), "a note")
    C.Check.str("update: group kept", TB.Tb.group(by.title(u1, "Fix it")), "G")
    C.Check.str("update: due kept", TB.Tb.due(by.title(u1, "Fix it")), "2026-11-02")
    C.Check.str("update: links kept", String.join(TB.Tb.links(by.title(u1, "Fix it")), " "), "http://x http://y /p")
    C.Check.str("update: created kept, updated moved", Nat.show(TB.Tb.at(by.title(u1, "Fix it"))) ++ " " ++ Nat.show(TB.Tb.updated(by.title(u1, "Fix it"))), "3000 3100")
    C.Check.str("update: append", TB.Tb.notes(by.title(more(u1, cs.of(TB.Tb.update(u1, 3200n, "person", uid, pj("{\"notes_append\":\"second\"}")))), "Fix it")), "a note\nsecond")
    C.Check.str("update: notes replaced", TB.Tb.notes(by.title(more(u1, cs.of(TB.Tb.update(u1, 3200n, "person", uid, pj("{\"notes\":\"new\"}")))), "Fix it")), "new")
    C.Check.str("update: clear due", TB.Tb.due(by.title(more(u1, cs.of(TB.Tb.update(u1, 3200n, "person", uid, pj("{\"due\":\"\"}")))), "Fix it")), "")
    C.Check.str("update: unknown", ok.of(TB.Tb.update(u1, 3200n, "person", "zz", pj("{}"))), "no item zz")
    C.Check.str("update: bad due", ok.of(TB.Tb.update(u1, 3200n, "person", uid, pj("{\"due\":\"10/28\"}"))), "a due date is YYYY-MM-DD")
    C.Check.str("update: owner by name", TB.Tb.owner(by.title(more(u1, cs.of(TB.Tb.update(u1, 3200n, "person", uid, pj("{\"owner\":\"tofu\"}")))), "Fix it")), id.of(s, "Tofu"))
    C.Check.str("update: owner person", TB.Tb.owner(by.title(more(u1, cs.of(TB.Tb.update(u1, 3200n, "person", uid, pj("{\"owner\":\"person\"}")))), "Fix it")), "")
    C.Check.str("update: bad owner", ok.of(TB.Tb.update(u1, 3200n, "person", uid, pj("{\"owner\":\"Ghost\"}"))), "no bot \"Ghost\" here")
    C.Check.str("update: project by name", TB.Tb.project(by.title(more(u1, cs.of(TB.Tb.update(u1, 3200n, "person", uid, pj("{\"project\":\"backplane\"}")))), "Fix it")), "p2")
    C.Check.str("update: unknown status is todo", TB.Tb.status(by.title(more(u1, cs.of(TB.Tb.update(u1, 3200n, "person", uid, pj("{\"status\":\"whatever\"}")))), "Fix it")), "todo")
    # add
    C.Check.str("add: title required", ok.of(TB.Tb.add(s, 3000n, "person", pj("{\"notes\":\"x\"}"))), "an item needs a title")
    C.Check.str("add: title length", ok.of(TB.Tb.add(s, 3000n, "person", pj("{\"title\":\"" ++ String.repeat("x", 301n) ++ "\"}"))), "a title is at most 300 characters")
    C.Check.str("add: notes length", ok.of(TB.Tb.add(s, 3000n, "person", pj("{\"title\":\"x\",\"notes\":\"" ++ String.repeat("x", 4001n) ++ "\"}"))), "notes are at most 4000 characters")
    C.Check.str("add: links capped at 20", Nat.show(List.length(&2, String, TB.Tb.links(by.title(added(s, 3000n, "{\"title\":\"x\",\"links\":\"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22\"}"), "x")))), "20")
    C.Check.str("add: links array", String.join(TB.Tb.links(by.title(added(s, 3000n, "{\"title\":\"x\",\"links\":[\"a\",\" b \",\"\"]}"), "x")), " "), "a b")
    C.Check.str("add: line", line(by.title(added(s, 3000n, "{\"title\":\"x\",\"status\":\"Blocked\",\"group\":\" G \",\"project\":\"Power board\",\"owner\":\"Miso\"}"), "x")), "blocked/G//p1/" ++ id.of(s, "Miso"))
    C.Check.str("add: by", TB.Tb.by(by.title(added(s, 3000n, "{\"title\":\"x\"}"), "x")), "person")
    C.Check.str("add: unknown project", ok.of(TB.Tb.add(s, 3000n, "person", pj("{\"title\":\"x\",\"project\":\"Nope\"}"))), "no project \"Nope\"")
    # remove
    C.Check.str("remove", n(TB.Tb.all(more(u1, cs.of(TB.Tb.remove(u1, 3300n, uid))))), "0")
    C.Check.str("remove: unknown", ok.of(TB.Tb.remove(u1, 3300n, "zz")), "no item zz")
    # text and json
    +tx : M.State = added(s, 3000n, "{\"title\":\"Order stencils\",\"owner\":\"Miso\",\"project\":\"Power board\",\"due\":\"2026-11-03\",\"group\":\"Fab\"}")
    C.Check.str("text", TB.Tb.text(TB.Tb.all(tx), tx), "[todo] Order stencils (" ++ TB.Tb.id(by.title(tx, "Order stencils")) ++ "; @Miso; %Power board; due 2026-11-03; Fab)")
    C.Check.str("text: bare", TB.Tb.text(TB.Tb.all(added(s, 3000n, "{\"title\":\"x\"}")), added(s, 3000n, "{\"title\":\"x\"}")), "[todo] x (" ++ TB.Tb.id(by.title(added(s, 3000n, "{\"title\":\"x\"}"), "x")) ++ ")")
    C.Check.str("json names", has(J.Json.show(TB.Tb.json(by.title(tx, "Order stencils"), tx)), "\"ownerName\":\"Miso\",\"projectName\":\"Power board\""), "yes")
    C.Check.str("json person", has(J.Json.show(TB.Tb.json(by.title(added(s, 3000n, "{\"title\":\"x\"}"), "x"), s)), "\"ownerName\":\"person\""), "yes")
    # ledger: task_board_list
    C.Check.str("list: empty", J.Json.show(ans(s, miso, "task_board_list", "{}")), "{\"text\":\"\",\"items\":[]}")
    +bd : M.State = after(after(after(s, miso, "task_board_add", "{\"title\":\"Mine\",\"owner\":\"me\",\"due\":\"2026-11-05\"}"), tofu, "task_board_add", "{\"title\":\"Theirs\",\"owner\":\"me\"}"),
      "t-pb", "task_board_add", "{\"title\":\"Persons\",\"status\":\"done\",\"project\":\"Power board\"}")
    C.Check.str("add: me is the calling bot", TB.Tb.owner(by.title(bd, "Mine")), id.of(s, "Miso"))
    C.Check.str("add: by is the bot", TB.Tb.by(by.title(bd, "Mine")), id.of(s, "Miso"))
    C.Check.str("add: by a plain thread", TB.Tb.by(by.title(bd, "Persons")), "thread:t-pb")
    C.Check.str("list: open by default", has(text(bd, miso, "task_board_list", "{}"), "Persons"), "no")
    C.Check.str("list: all", has(text(bd, miso, "task_board_list", "{\"all\":true}"), "Persons"), "yes")
    C.Check.str("list: me", has(text(bd, miso, "task_board_list", "{\"owner\":\"me\"}"), "Theirs"), "no")
    C.Check.str("list: me finds mine", has(text(bd, miso, "task_board_list", "{\"owner\":\"me\"}"), "Mine"), "yes")
    C.Check.str("list: a bot by name", has(text(bd, miso, "task_board_list", "{\"owner\":\"Tofu\"}"), "Theirs"), "yes")
    C.Check.str("list: person", has(text(bd, miso, "task_board_list", "{\"owner\":\"person\",\"all\":true}"), "Persons"), "yes")
    C.Check.str("list: me from a plain thread", yes(failed(bd, "t-pb", "task_board_list", "{\"owner\":\"me\"}")), "yes")
    C.Check.str("list: unknown owner", yes(failed(bd, miso, "task_board_list", "{\"owner\":\"Ghost\"}")), "yes")
    C.Check.str("list: status list", has(text(bd, miso, "task_board_list", "{\"status\":\"done,doing\"}"), "Persons"), "yes")
    C.Check.str("list: project", has(text(bd, miso, "task_board_list", "{\"project\":\"power board\",\"all\":true}"), "Mine"), "no")
    C.Check.str("list: shape", has(J.Json.show(ans(bd, miso, "task_board_list", "{}")), "{\"text\":\"[todo] Mine ("), "yes")
    C.Check.str("list: items carry names", has(J.Json.show(ans(bd, miso, "task_board_list", "{}")), "\"ownerName\":\"Miso\""), "yes")
    C.Check.str("list: ordered by due", has(text(bd, miso, "task_board_list", "{}"), "Mine"), "yes")
    # ledger: add, update, remove, import
    C.Check.str("tool add: answer", has(J.Json.show(ans(s, miso, "task_board_add", "{\"title\":\"x\"}")), "{\"id\":\"b5000-"), "yes")
    C.Check.str("tool add: one change", Nat.show(List.length(&2, M.Change, step.cs(mcp(s, miso, "task_board_add", "{\"title\":\"x\"}")))), "1")
    C.Check.str("tool add: no title", yes(failed(s, miso, "task_board_add", "{}")), "yes")
    C.Check.str("tool add: error says why", has(text(s, miso, "task_board_add", "{}"), "an item needs a title"), "yes")
    C.Check.str("tool add: error changes nothing", Nat.show(List.length(&2, M.Change, step.cs(mcp(s, miso, "task_board_add", "{}")))), "0")
    +mid : String = TB.Tb.id(by.title(bd, "Mine"))
    C.Check.str("tool update: answer", J.Json.show(ans(bd, miso, "task_board_update", "{\"id\":\"" ++ mid ++ "\",\"status\":\"done\"}")), "{\"id\":\"" ++ mid ++ "\"}")
    C.Check.str("tool update: applied", TB.Tb.status(by.title(after(bd, miso, "task_board_update", "{\"id\":\"" ++ mid ++ "\",\"status\":\"done\"}"), "Mine")), "done")
    C.Check.str("tool update: no id", yes(failed(bd, miso, "task_board_update", "{\"status\":\"done\"}")), "yes")
    C.Check.str("tool update: unknown id", has(text(bd, miso, "task_board_update", "{\"id\":\"zz\"}"), "no item zz"), "yes")
    C.Check.str("tool update: owner me", TB.Tb.owner(by.title(after(bd, tofu, "task_board_update", "{\"id\":\"" ++ mid ++ "\",\"owner\":\"me\"}"), "Mine")), id.of(s, "Tofu"))
    C.Check.str("tool remove: answer", J.Json.show(ans(bd, miso, "task_board_remove", "{\"id\":\"" ++ mid ++ "\"}")), "{\"id\":\"" ++ mid ++ "\"}")
    C.Check.str("tool remove: applied", n(TB.Tb.all(after(bd, miso, "task_board_remove", "{\"id\":\"" ++ mid ++ "\"}"))), "2")
    C.Check.str("tool remove: no id", yes(failed(bd, miso, "task_board_remove", "{}")), "yes")
    C.Check.str("tool remove: unknown", yes(failed(bd, miso, "task_board_remove", "{\"id\":\"zz\"}")), "yes")
    C.Check.str("tool import: answer", J.Json.show(ans(s, miso, "task_board_import", "{\"markdown\":\"## Errands\\n- [ ] Peach Pass\\n- [x] floss\\n- plain\"}")), "{\"added\":2,\"duplicates\":0,\"skipped\":[\"- plain\"]}")
    C.Check.str("tool import: applied", n(TB.Tb.all(after(s, miso, "task_board_import", "{\"markdown\":\"## Errands\\n- [ ] Peach Pass\\n- [x] floss\\n- plain\"}"))), "2")
    C.Check.str("tool import: twice", J.Json.show(ans(after(s, miso, "task_board_import", "{\"markdown\":\"- [ ] a\\n- [ ] b\"}"), miso, "task_board_import", "{\"markdown\":\"- [ ] a\\n- [ ] b\"}")), "{\"added\":0,\"duplicates\":2,\"skipped\":[]}")
    C.Check.str("tool import: owner me", TB.Tb.owner(by.title(after(s, miso, "task_board_import", "{\"markdown\":\"- [ ] a\",\"owner\":\"me\"}"), "a")), id.of(s, "Miso"))
    C.Check.str("tool import: no markdown", yes(failed(s, miso, "task_board_import", "{}")), "yes")
    C.Check.str("tool import: unknown project", has(text(s, miso, "task_board_import", "{\"markdown\":\"- [ ] a\",\"project\":\"Nope\"}"), "no project"), "yes")
    # ledger: told_mark and told_check
    C.Check.str("mark: answer", J.Json.show(ans(s, miso, "told_mark", "{\"key\":\"Lunch Kane\",\"text\":\"Kane lunch Wed\",\"channel\":\"digest\"}")), "{\"key\":\"lunch-kane\"}")
    C.Check.str("mark: key from text", J.Json.show(ans(s, miso, "told_mark", "{\"text\":\"Kane lunch in ATL Wed\"}")), "{\"key\":\"kane-lunch-atl-wed\"}")
    C.Check.str("mark: needs text", yes(failed(s, miso, "told_mark", "{\"key\":\"k\"}")), "yes")
    C.Check.str("mark: one change", Nat.show(List.length(&2, M.Change, step.cs(mcp(s, miso, "told_mark", "{\"text\":\"x y z\"}")))), "1")
    +tm : M.State = after(s, miso, "told_mark", "{\"key\":\"lunch-kane\",\"text\":\"Kane lunch in ATL Wed 10/28\",\"channel\":\"digest\"}")
    C.Check.str("mark: the bot", TD.Told.bot(Maybe.default(&2, TD.Told, List.head(&2, TD.Told, TD.Told.all(tm)), TD.Told{"", "", "", "", "", 0n})), id.of(s, "Miso"))
    C.Check.str("mark: from a plain thread no bot", TD.Told.bot(Maybe.default(&2, TD.Told, List.head(&2, TD.Told, TD.Told.all(after(s, "t-pb", "told_mark", "{\"text\":\"abc def\"}"))), TD.Told{"", "", "", "", "", 0n})), "")
    C.Check.str("check: found by key", has(J.Json.show(ans(tm, tofu, "told_check", "{\"key\":\"lunch-kane\"}")), "{\"told\":true,\"matches\":[{\"key\":\"lunch-kane\",\"text\":\"Kane lunch in ATL Wed 10/28\",\"channel\":\"digest\",\"bot\":\"Miso\","), "yes")
    C.Check.str("check: ago", has(J.Json.show(ans(tm, tofu, "told_check", "{\"key\":\"lunch-kane\"}")), "\"ago\":\"just now\""), "yes")
    C.Check.str("check: found by text", has(J.Json.show(ans(tm, tofu, "told_check", "{\"text\":\"lunch with Kane in ATL on 10/28\"}")), "\"told\":true"), "yes")
    C.Check.str("check: nothing", J.Json.show(ans(tm, tofu, "told_check", "{\"key\":\"dentist\"}")), "{\"told\":false,\"matches\":[]}")
    C.Check.str("check: needs one", yes(failed(tm, tofu, "told_check", "{}")), "yes")
    C.Check.str("check: days", has(J.Json.show(ans(tm, tofu, "told_check", "{\"key\":\"lunch-kane\",\"days\":1}")), "\"told\":true"), "yes")
    C.Check.str("check: no changes", Nat.show(List.length(&2, M.Change, step.cs(mcp(tm, tofu, "told_check", "{\"key\":\"lunch-kane\"}")))), "0")
    # routing and definitions
    C.Check.str("is: a ledger call", yes(LG.Ledger.is(pj("{\"method\":\"tools/call\",\"params\":{\"name\":\"told_check\"}}"))), "yes")
    C.Check.str("is: another tool", yes(LG.Ledger.is(pj("{\"method\":\"tools/call\",\"params\":{\"name\":\"bot_send\"}}"))), "no")
    C.Check.str("is: a list", yes(LG.Ledger.is(pj("{\"method\":\"tools/list\"}"))), "no")
    C.Check.str("tools: seven", Nat.show(List.length(&2, J.Json, LG.Ledger.tools())), "7")
    C.Check.str("routed from a bot", has(MC.McpOut.reply(BH.Bots.mcp(s, 5000n, miso, J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"told_mark\",\"arguments\":{\"text\":\"abc def\"}}}")), "abc-def"), "yes")
    C.Check.str("routed from a plain thread", has(MC.McpOut.reply(BH.Bots.mcp(s, 5000n, "t-pb", J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"told_mark\",\"arguments\":{\"text\":\"abc def\"}}}")), "abc-def"), "yes")
    C.Check.str("listed for a bot", has(MC.McpOut.reply(BH.Bots.mcp(s, 5000n, miso, J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/list\"}")), "task_board_import"), "yes")
    C.Check.str("listed for a plain thread", has(MC.McpOut.reply(BH.Bots.mcp(s, 5000n, "t-pb", J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/list\"}")), "task_board_import"), "yes")
    C.Check.str("plain thread still has the rest", has(MC.McpOut.reply(BH.Bots.mcp(s, 5000n, "t-pb", J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/list\"}")), "delegate_task"), "yes")
    C.Check.str("bot keeps its own", has(MC.McpOut.reply(BH.Bots.mcp(s, 5000n, miso, J.Null{}, "t", J.Json.arr(Nil{}), Nil{}, "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/list\"}")), "memory_save"), "yes")
    # the person's requests
    C.Check.str("board.set adds", n(TB.Tb.all(run(s, LG.Ledger.request(s, 6000n, 1, req("board.set", "{\"item\":{\"title\":\"Call mom\"}}"))))), "1")
    C.Check.str("board.set by the person", TB.Tb.by(by.title(run(s, LG.Ledger.request(s, 6000n, 1, req("board.set", "{\"item\":{\"title\":\"Call mom\"}}"))), "Call mom")), "person")
    C.Check.str("board.set updates", TB.Tb.status(by.title(run(bd, LG.Ledger.request(bd, 6000n, 1, req("board.set", "{\"item\":{\"id\":\"" ++ mid ++ "\",\"status\":\"blocked\"}}"))), "Mine")), "blocked")
    C.Check.str("board.set error replies", has(rep(LG.Ledger.request(s, 6000n, 1, req("board.set", "{\"item\":{\"notes\":\"x\"}}"))), "\"ok\":false,\"error\":\"an item needs a title\""), "yes")
    C.Check.str("board.set ok replies", rep(LG.Ledger.request(s, 6000n, 1, req("board.set", "{\"item\":{\"title\":\"x\"}}"))), "{\"t\":\"reply\",\"id\":1,\"ok\":true}")
    C.Check.str("board.drop", n(TB.Tb.all(run(bd, LG.Ledger.request(bd, 6000n, 1, req("board.drop", "{\"id\":\"" ++ mid ++ "\"}"))))), "2")
    C.Check.str("board.drop unknown", has(rep(LG.Ledger.request(bd, 6000n, 1, req("board.drop", "{\"id\":\"zz\"}"))), "no item zz"), "yes")
    C.Check.str("methods", yes(Bool.and(LG.Ledger.method("rules.set"), Bool.not(LG.Ledger.method("rules.other")))), "yes")
