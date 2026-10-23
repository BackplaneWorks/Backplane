import Base
import ./check.bend as C
import ../src/core/hier.bend as H
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/utf8.bend as T
import ../src/core/sexpr.bend as S2
import ../src/server/plot.bend as PV

# Hierarchical schematic navigation by sheet instance (client.bend's Sch,
# hier.bend's places, boxes and children). Synthetic, read-only fixtures:
# /p/top has Power and two instances of one file, chan (Ch1, Ch2), each
# holding Amp; so chan and amp are each two instances.

def tab() -> List<&2, H.Kids>:
  [H.Kids{"/p/top.kicad_sch", [H.Sheet{"Power", "/p/power.kicad_sch"}, H.Sheet{"Ch1", "/p/chan.kicad_sch"}, H.Sheet{"Ch2", "/p/chan.kicad_sch"}]},
   H.Kids{"/p/chan.kicad_sch", [H.Sheet{"Amp", "/p/amp.kicad_sch"}]}]

def rows() -> List<&2, H.Row>:
  H.Hier.rows(tab(), "/p/top.kicad_sch")

# Ch2 renamed: the same instances, new names
def renamed() -> List<&2, H.Row>:
  H.Hier.rows([H.Kids{"/p/top.kicad_sch", [H.Sheet{"Power", "/p/power.kicad_sch"}, H.Sheet{"Ch1", "/p/chan.kicad_sch"}, H.Sheet{"Chan2", "/p/chan.kicad_sch"}]},
    H.Kids{"/p/chan.kicad_sch", [H.Sheet{"Amp", "/p/amp.kicad_sch"}]}], "/p/top.kicad_sch")

# each file's sheets' uuids and pages (KiCad 7+ pairs from the root R), and
# the root's own uuid and pages
def ids() -> String:
  "/p/top.kicad_sch\tU1|\tU2|/R=2\tU3|/R=3\n/p/chan.kicad_sch\tA1|/R/U2=4|/R/U3=5"

def top() -> String:
  "R|/=1"

def ids.of(rs: List<&2, H.Row>) -> List<&2, List<&2, String>>:
  [H.Hier.boxes.of(ids(), "/p/top.kicad_sch"), Nil{}, H.Hier.boxes.of(ids(), "/p/chan.kicad_sch"), Nil{}, H.Hier.boxes.of(ids(), "/p/chan.kicad_sch"), Nil{}]

def boxes() -> String:
  "/p/top.kicad_sch\t1 1 2 2\t3 3 4 4\t5 5 6 6\n/p/chan.kicad_sch\t7 7 8 8"

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def json(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def on(u: U.Ui, +id: String) -> U.Ui:
  ui(U.Ui.act(u, "select", id))

def act(u: U.Ui, +a: String, +v: String) -> U.Ui:
  ui(U.Ui.act(u, a, v))

def sheets(+u: U.Ui) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"sheets"}}, J.KV{"root", J.Str{U.Ui.root(u)}}, J.KV{"rows", J.Str{H.Hier.show(rows())}}, J.KV{"boxes", J.Str{boxes()}}])

# a reply as the plot actor sends it for a root (gen: its message count,
# server/plot.bend's Pv.tree.gen)
def tree.at(+root: String, +rs: List<&2, H.Row>, +slot: String, +gen: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"sheets"}}, J.KV{"root", J.Str{root}}, J.KV{"slot", J.Str{slot}}, J.KV{"gen", J.Str{gen}},
    J.KV{"rows", J.Str{H.Hier.show(rs)}}, J.KV{"boxes", J.Str{boxes()}}, J.KV{"ids", J.Str{ids()}}, J.KV{"top", J.Str{top()}}])

# thread a with uuids known (generation 5)

def tree.json(+u: U.Ui, +rs: List<&2, H.Row>, +slot: String, +gen: String) -> J.Json:
  tree.at(U.Ui.root(u), rs, slot, gen)

# the generation the plot actor gives its n-th message (server/plot.bend's
# Pv.tree.gen, the real constructor)
def gen(+n: Nat) -> String:
  PV.Pv.tree.gen(n)

# a hub's join (full from 0, or a resume from since) from origin
def join(+origin: String, +since: String) -> J.Json:
  json("{\"t\":\"log\",\"since\":" ++ since ++ ",\"origin\":\"" ++ origin ++ "\",\"info\":{},\"items\":[]}")

def keyed() -> U.Ui:
  U.Sch.got(on(U.Ui.init(0n), "a"), tree.json(on(U.Ui.init(0n), "a"), rows(), "", "5"))

def page.v(v: U.SchView) -> String:
  match v:
    case U.SchView{_, _, _, _, _, _, _, _, _, _, _, _, p}:
      p

def page(+u: U.Ui) -> String:
  page.v(U.Sch.view(u))

def rowtext(+u: U.Ui) -> String:
  H.Hier.show(U.Sch.rows(u))

# thread a, its project's sheets known
def start() -> U.Ui:
  U.Sch.got(on(U.Ui.init(0n), "a"), sheets(on(U.Ui.init(0n), "a")))

def Show.nats(xs: List<&2, Nat>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> t:
      Nat.show(x) ++ " " ++ Show.nats(t)

def Show.strs(xs: List<&2, String>) -> String:
  String.join(xs, " ")

def Show.ids(rs: List<&2, U.SchRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.SchRow{i, _, _, _, st, _, _, _} <> t:
      Nat.show(i) ++ ":" ++ Nat.show(st) ++ " " ++ Show.ids(t)

def at.v(v: U.SchView) -> String:
  match v:
    case U.SchView{_, _, _, a, _, _, _, _, o, _, _, _, _}:
      Nat.show(a) ++ Bool.pick(String, o, " out", "")

def at(+u: U.Ui) -> String:
  at.v(U.Sch.view(u))

def trail.v(v: U.SchView) -> String:
  match v:
    case U.SchView{_, t, _, _, _, _, _, _, _, _, _, _, _}:
      Show.ids(t)

def trail(+u: U.Ui) -> String:
  trail.v(U.Sch.view(u))

def tree.v(v: U.SchView) -> String:
  match v:
    case U.SchView{r, _, _, _, _, _, _, _, _, _, _, _, _}:
      Show.ids(r)

def tree(+u: U.Ui) -> String:
  tree.v(U.Sch.view(u))

def here.v(v: U.SchView) -> String:
  match v:
    case U.SchView{_, _, h, _, _, _, _, _, _, _, _, _, _}:
      Show.ids(h)

def here(+u: U.Ui) -> String:
  here.v(U.Sch.view(u))

def hist.v(v: U.SchView) -> String:
  match v:
    case U.SchView{_, _, _, _, _, _, b, f, _, _, _, _, _}:
      Bool.pick(String, b, "back", "-") ++ " " ++ Bool.pick(String, f, "fwd", "-")

def hist(+u: U.Ui) -> String:
  hist.v(U.Sch.view(u))

def note.v(v: U.SchView) -> String:
  match v:
    case U.SchView{_, _, _, _, _, _, _, _, _, _, _, n, _}:
      n

def note(+u: U.Ui) -> String:
  note.v(U.Sch.view(u))

def shown.v(v: U.SchView) -> String:
  match v:
    case U.SchView{_, _, _, _, _, _, _, _, _, s, _, _, _}:
      Bool.pick(String, s, "tree", "no tree")

def shown(+u: U.Ui) -> String:
  shown.v(U.Sch.view(u))

def path(+u: U.Ui) -> String:
  U.Ui.view_get(u, "view.path")

def main() -> IO(Unit):
  +rs = rows()
  +u0 = start()
  +ch2 = act(u0, "sch-go", "4\t/Ch2/")
  +amp2 = act(ch2, "sch-go", "5\t/Ch2/Amp/")
  +up = act(amp2, "sch-up", "")
  +back = act(up, "sch-back", "")
  +fwd = act(back, "sch-fwd", "")
  +atRoot = act(fwd, "sch-root", "")
  +k0 = keyed()
  +kch2 = act(k0, "sch-go", "4\t/Ch2/\t/U3/")
  +kren = U.Sch.got(kch2, tree.json(kch2, renamed(), "", "6"))
  +kstale = U.Sch.got(kren, tree.json(kren, rows(), "", "4"))
  +kver = U.Sch.got(kren, tree.json(kren, rows(), "/g\tc1", "9"))
  # two watches in one second (the actor's messages 7
  # and 8), their replies delivered in reverse order, then in order
  +s1 = U.Sch.got(on(U.Ui.init(0n), "a"), tree.json(on(U.Ui.init(0n), "a"), renamed(), "", gen(8n)))
  +s2 = U.Sch.got(s1, tree.json(s1, rows(), "", gen(7n)))
  +s3 = U.Sch.got(U.Sch.got(on(U.Ui.init(0n), "a"), tree.json(on(U.Ui.init(0n), "a"), rows(), "", gen(7n))), tree.json(on(U.Ui.init(0n), "a"), renamed(), "", gen(8n)))
  # a restarted hub counts from 0 again; its socket's join comes first
  +s4 = U.Sch.got(U.Ui.recv(s1, join("O", "4")), tree.json(s1, rows(), "", gen(0n)))
  # another project's tree on the same client
  +s5 = U.Sch.got(act(s1, "sch-go", "4\t/Chan2/\t/U3/"), tree.at("/elsewhere", rows(), "", gen(9n)))
  # the cache belongs to the connection: a join drops it, so a new hub's
  # replies are taken, its count starting again
  +j1 = U.Ui.recv(act(s1, "sch-go", "4\t/Chan2/\t/U3/"), join("O", "4"))
  +j2 = U.Sch.got(j1, tree.json(j1, rows(), "", gen(3n)))
  +other = U.Ui.recv(amp2, json("{\"t\":\"view\",\"thread\":\"a\",\"path\":\"/q/other.kicad_sch\",\"kind\":\"schematic\"}"))
  do IO<Unit>:
    C.Check.str("places", Show.strs(H.Hier.paths(rs)), "/ /Power/ /Ch1/ /Ch1/Amp/ /Ch2/ /Ch2/Amp/")
    C.Check.str("place of a repeated instance", H.Hier.path(rs, 5n), "/Ch2/Amp/")
    C.Check.str("no row, no place", H.Hier.path(rs, 9n), "")
    C.Check.str("seek the second instance", Nat.show(Maybe.default(&2, Nat, H.Hier.seek(rs, "/Ch2/Amp/"), 99n)), "5")
    C.Check.str("under the root", Show.nats(H.Hier.under(rs, 0n)), "1 2 4 ")
    C.Check.str("under Ch2", Show.nats(H.Hier.under(rs, 4n)), "5 ")
    C.Check.str("under a leaf", Show.nats(H.Hier.under(rs, 5n)), "")
    C.Check.str("boxes of a file", Show.strs(H.Hier.boxes.of(boxes(), "/p/chan.kicad_sch")), "7 7 8 8")
    C.Check.str("boxes: a key is a whole file", Show.strs(H.Hier.boxes.of(boxes(), "/p/chan")), "")
    C.Check.str("boxes read from sheet forms", Nat.show(List.length(&2, String, H.Hier.boxes(T.Utf8.encode("(kicad_sch (sheet (at 10 10) (size 20 20) (property \"Sheetname\" \"A\") (property \"Sheetfile\" \"a.kicad_sch\")) (sheet (at 40 10) (size 5 5) (property \"Sheetname\" \"B\") (property \"Sheetfile\" \"b.kicad_sch\")))")))), "2")
    C.Check.str("starts at the root", at(u0), "0")
    C.Check.str("the root's sheets here", here(u0), "1:0 2:0 4:0 ")
    C.Check.str("Ch2's sheet symbols", U.Sch.boxes(ch2), "5\t/Ch2/Amp/\t\t7 7 8 8")
    C.Check.str("the root's sheet symbols", U.Sch.boxes(atRoot), "1\t/Power/\t\t1 1 2 2\n2\t/Ch1/\t\t3 3 4 4\n4\t/Ch2/\t\t5 5 6 6")
    C.Check.str("a repeated instance is itself, not the first of its file", at(ch2), "4")
    C.Check.str("its file on show", path(ch2), "/p/chan.kicad_sch")
    C.Check.str("trail to it", trail(amp2), "0:1 4:1 5:2 ")
    C.Check.str("tree marks the path", tree(amp2), "0:1 1:0 2:0 3:0 4:1 5:2 ")
    C.Check.str("up goes to its own parent", at(up), "4")
    C.Check.str("back returns", at(back), "5")
    C.Check.str("back leaves forward", hist(back), "back fwd")
    C.Check.str("forward", at(fwd), "4")
    C.Check.str("root", at(atRoot) ++ "|" ++ path(atRoot), "0|")
    C.Check.str("up at the root stays", at(act(atRoot, "sch-up", "")), "0")
    C.Check.str("back from the root", at(act(atRoot, "sch-back", "")), "4")
    C.Check.str("a new place clears forward", hist(act(back, "sch-go", "1\t/Power/")), "back -")
    C.Check.str("going where it is keeps history", hist(act(u0, "sch-go", "0\t/")), "- -")
    C.Check.str("a stale row goes by its place", at(act(u0, "sch-go", "3\t/Ch2/Amp/")), "5")
    C.Check.str("a place that is gone changes nothing", at(act(ch2, "sch-go", "9\t/Gone/")) ++ "|" ++ note(act(ch2, "sch-go", "9\t/Gone/")), "4|gone")
    C.Check.str("the note clears on the next sheet", note(act(act(ch2, "sch-go", "9\t/Gone/"), "sch-root", "")), "")
    C.Check.str("a file outside the hierarchy", at(other), "6 out")
    C.Check.str("up from outside goes to the root", at(act(other, "sch-up", "")), "0")
    C.Check.str("a folded branch hides its rows", tree(act(amp2, "sch-fold", "/Ch1/")), "0:1 1:0 2:0 4:1 5:2 ")
    C.Check.str("the path on show never folds", tree(act(amp2, "sch-fold", "/Ch2/")), "0:1 1:0 2:0 3:0 4:1 5:2 ")
    C.Check.str("fold twice opens", tree(act(act(amp2, "sch-fold", "/Ch1/"), "sch-fold", "/Ch1/")), "0:1 1:0 2:0 3:0 4:1 5:2 ")
    C.Check.str("the tree puts away", shown(act(u0, "sch-tree", "")), "no tree")
    C.Check.str("each thread keeps its own instance", at(on(amp2, "b")) ++ "|" ++ hist(on(amp2, "b")), "0|- -")
    C.Check.str("and gets it back", at(on(on(amp2, "b"), "a")), "5")
    C.Check.str("the phones' menu picks a row", at(act(u0, "view-sheet", "#4")), "4")
    C.Check.str("a file picks its first instance", at(act(u0, "view-sheet", "/p/chan.kicad_sch")), "2")
    C.Check.str("no boxes on a board", U.Sch.boxes(U.Ui.view_pick(ch2, "board")), "")
    C.Check.str("uuid paths and pages", String.join(H.Hier.uids(rs, ids.of(rs), top()), ","), "/\t1,/U1/\t,/U2/\t2,/U2/A1/\t4,/U3/\t3,/U3/A1/\t5")
    C.Check.str("no uuids: no instance paths", String.join(H.Hier.uids(rs, Nil{}, ""), ","), "/\t,\t,\t,\t,\t,\t")
    C.Check.str("a sheet form's uuid and pages", H.Hier.uid.form(S2.Sx.parse(T.Utf8.encode("(sheet (at 1 1) (size 2 2) (uuid \"U2\") (instances (project \"p\" (path \"/R\" (page \"2\"))) (project \"q\" (path \"/S\" (page \"7\")))))"))), "U2|/R=2|/S=7")
    C.Check.str("the root's uuid and pages", H.Hier.top(T.Utf8.encode("(kicad_sch (version 1) (uuid \"R\") (sheet_instances (path \"/\" (page \"1\")) (path \"/U1\" (page \"9\"))))")), "R|/=1|/U1=9")
    C.Check.str("an entry keeps its uuid path", U.Ui.scratch(kch2, "@sch:a"), "4\t/Ch2/\t/U3/")
    C.Check.str("the page on show is KiCad's", page(kch2), "3")
    C.Check.str("a rename keeps the instance by uuid", at(kren) ++ "|" ++ path(kren), "4|/p/chan.kicad_sch")
    C.Check.str("a uuid beats a stale row and place", at(act(kren, "sch-go", "3\t/Gone/\t/U3/A1/")), "5")
    C.Check.str("folds key by uuid", tree(act(act(kren, "sch-go", "5\t\t/U3/A1/"), "sch-fold", "/U2/")), "0:1 1:0 2:0 4:1 5:2 ")
    C.Check.str("an older reply is not taken", rowtext(kstale), H.Hier.show(renamed()))
    C.Check.str("and the sheet on show stays", at(kstale), "4")
    C.Check.str("a version's tree is kept apart", rowtext(kver), H.Hier.show(renamed()))
    C.Check.str("its generation is its own", U.Ui.info(kver, "sheetgen:" ++ U.Ui.root(kver)) ++ "|" ++ U.Ui.info(kver, "sheetgen:" ++ U.Ui.root(kver) ++ "\t/g\tc1"), "6|9")
    C.Check.str("same second, reversed: the later request's reply stays", rowtext(s2), H.Hier.show(renamed()))
    C.Check.str("same second, reversed: its generation stays", U.Ui.info(s2, "sheetgen:" ++ U.Ui.root(s2)), gen(8n))
    C.Check.str("same second, in order: the later is taken", rowtext(s3), H.Hier.show(renamed()))
    C.Check.str("a restarted hub's first reply (count 0) is taken after its join", rowtext(s4), H.Hier.show(rows()))
    C.Check.str("a same reply again changes nothing", rowtext(U.Sch.got(s1, tree.json(s1, rows(), "", gen(8n)))), H.Hier.show(renamed()))
    C.Check.str("another root's tree leaves this one", rowtext(s5) ++ "|" ++ at(s5), H.Hier.show(renamed()) ++ "|4")
    C.Check.str("and is kept under its own root", U.Ui.info(s5, "sheetgen:/elsewhere"), gen(9n))
    C.Check.str("a thread switch reads its own instance of the same tree", at(on(s5, "b")) ++ "|" ++ at(on(on(s5, "b"), "a")), "0|4")
    C.Check.str("generation: the actor's message count, no clock", gen(7n), "7")
    C.Check.str("generation: at a real epoch's magnitude it stays a plain number", gen(1792779015n), "1792779015")
    C.Check.str("generation: and orders there", Bool.show(U.Sch.newer(gen(1792779016n), gen(1792779015n))), "True")
    C.Check.str("generation: a count past 9 orders as a number, not text", Bool.show(U.Sch.newer(gen(10n), gen(9n))), "True")
    C.Check.str("generation: the next message is newer in the same second", Bool.show(U.Sch.newer(gen(8n), gen(7n))), "True")
    C.Check.str("generation: an earlier message is not newer", Bool.show(U.Sch.newer(gen(7n), gen(8n))), "False")
    C.Check.str("generation: the same is not newer", Bool.show(U.Sch.newer(gen(8n), gen(8n))), "False")
    C.Check.str("generation: anything is newer than none held", Bool.show(U.Sch.newer(gen(0n), "")), "True")
    C.Check.str("a join drops the sheets and their generation", U.Ui.info(j1, "sheets:" ++ U.Ui.root(j1)) ++ "|" ++ U.Ui.info(j1, "sheetgen:" ++ U.Ui.root(j1)), "|")
    C.Check.str("a join from another hub drops them too", U.Ui.info(U.Ui.recv(s1, join("P", "0")), "sheetgen:" ++ U.Ui.root(s1)), "")
    C.Check.str("after a join any reply is taken, whatever its clock", rowtext(j2), H.Hier.show(rows()))
    C.Check.str("a join keeps the instance on show", U.Ui.scratch(j1, "@sch:a"), "4\t/Chan2/\t/U3/")
