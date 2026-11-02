import Base
import ./check.bend as C
import ../src/core/board-catalog.bend as B
import ../src/core/view-page.bend as P
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as L
import ../src/core/presentation.bend as S
import ../src/server/thin-actor.bend as A
import ../src/server/thin-view.bend as V
import ../src/server/view-capacity.bend as CAP
import ../src/web/view.bend as W
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as N
import ../src/core/i18n.bend as I

def ids(+n: Nat) -> List<&2, String>:
  match n:
    case 0n: Nil{}
    case 1n+k: ("boards/board" ++ Nat.show(n) ++ "/main.kicad_pcb") <> ids(k)

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Board", "/tmp/compact", 1n}, M.ThreadCreated{"self", "p", "Here", M.Local{}, "", "", "codex", "", 2n}], M.State.empty())

def preferences(u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, v, rid, want, error, note, nf, ob, stores}:
      U.Ui{st, sel, Map.set(&2, String, dr, "@intent.draft:self", "borrowed draft"), live, Map.set(&2, Bool, op, "keep-me", True{}), on, now, ad, pa, v, rid, want, error, note,
        Map.set(&2, String, Map.set(&2, String, nf, "origin", "canonical-owner-origin"), "local.board.catalog.version", "1"), Map.set(&2, String, ob, "owned-command", "queued intact"), stores}

def ui(+n: Nat) -> U.Ui:
  +u = preferences(U.Ui.view_put(U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(10n), state()), "self"), "3d", "/tmp/compact/boards/board1/main.kicad_pcb"))
  U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(U.Ui.info_set(U.Ui.scratch_set(u, "self", "draft 日本語"), "local.workspace.layout", "rail-v1"), "local.view.lazy", "1"), "fab.boards:self", String.join(ids(n), "\n")), "fab.board:self", "boards/board1/main.kicad_pcb")

def options(xs: List<&2, D.Node>) -> Nat:
  match xs:
    case Nil{}: 0n
    case D.Txt{_} <> rest: options(rest)
    case D.El{_, +key, _, kids} <> rest:
      Nat.add(Bool.pick(Nat, String.starts_with(key, "boards/board"), 1n, 0n), Nat.add(options(kids), options(rest)))

def open(+u: U.Ui, query: String, offset: Nat) -> U.Ui:
  U.Act.ui(U.Bd.catalog(u, B.Catalog.command(U.Bd.catalog.source(u), True{}, query, offset)))

def result(+u: U.Ui, compact: Bool) -> J.Json:
  +actor = A.Actor.make(u, "v", V.Target.json("thread", "self"))
  A.Rendered.result(A.Actor.render.acted(A.Acted{actor, V.ActResult{u, [J.Json.obj([J.KV{"method", J.Str{"preserved"}}])]}}, True{}, state(), T.Index.empty(), J.Json.obj([J.KV{"target", V.Target.json("thread", "self")}, J.KV{"@compactChrome", J.Flag{compact}}]), "epoch", 10n, Nil{}, L.Stream.blank(), M.State.seq(state())))

def canvas(xs: List<&2, D.Node>) -> String:
  match xs:
    case Nil{}: ""
    case D.Txt{_} <> rest: canvas(rest)
    case D.El{+tag, +key, +attrs, +kids} <> rest:
      Bool.pick(String, String.eq(key, "solid-workbench"), J.Json.show(N.Node.write(D.El{tag, key, attrs, kids})), canvas(kids) ++ canvas(rest))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("catalog dedup retains source order", J.Json.show(J.Json.arr(P.Page.json(B.Catalog.unique(["b", "a", "b"])))), "[\"b\",\"a\"]")
    +small : U.Ui = ui(2n)
    +large : U.Ui = ui(2048n)
    C.Check.str("closed small catalog has no option nodes", Nat.show(options(W.View.boards.catalog(small))), "0")
    C.Check.str("closed large catalog has no option nodes", Nat.show(options(W.View.boards.catalog(large))), "0")
    C.Check.str("closed rendered size independent of catalog length", Bool.show(Nat.is_eq(J.Json.bytes(N.Node.write(D.Dom.div("", W.View.boards.catalog(small)))), J.Json.bytes(N.Node.write(D.Dom.div("", W.View.boards.catalog(large)))))), "True")
    +legacy : U.Ui = U.Ui.info_set(small, "local.board.catalog.version", "")
    C.Check.str("old loaded rail host retains working legacy selector", Nat.show(options(W.View.boards.for(True{}, legacy, "Board"))), "2")
    C.Check.str("legacy session contract still includes derived catalog", J.Json.get_str(J.Json.get_obj(V.Route.pack(legacy), "info"), "fab.boards:self", ""), U.Ui.info(legacy, "fab.boards:self"))
    C.Check.str("missing capability cannot inherit private protocol state", U.Ui.info(V.Session.renderer(True{}, small, J.Json.obj(Nil{})), "local.board.catalog.version"), "")
    C.Check.str("new host explicitly opts into catalog protocol", U.Ui.info(V.Session.renderer(True{}, legacy, J.Json.obj([J.KV{"boardCatalog", J.Num{"1"}}])), "local.board.catalog.version"), "1")
    +opened : U.Ui = open(large, "", 0n)
    C.Check.str("open options bound includes off-page selected", Nat.show(options(W.View.boards.catalog(opened))), "17")
    C.Check.str("fitted option budget precedes DOM even for long paths", Nat.show(options(W.View.boards.catalog(U.Ui.info_set(opened, "local.view.budget", "1")))), "2")
    C.Check.str("second page stays bounded", Nat.show(options(W.View.boards.catalog(open(large, "", 16n)))), "17")
    C.Check.str("search finds off-page board plus selected", Nat.show(options(W.View.boards.catalog(open(large, "board1500/", 0n)))), "2")
    +closed : U.Ui = U.Act.ui(U.Bd.catalog(opened, B.Catalog.command(U.Bd.catalog.source(opened), False{}, "", 0n)))
    C.Check.str("close removes all option nodes", Nat.show(options(W.View.boards.catalog(closed))), "0")
    C.Check.str("stale source action cannot open changed target", U.Ui.info(U.Act.ui(U.Bd.catalog(U.Ui.select.k(large, "other"), B.Catalog.command(U.Bd.catalog.source(large), True{}, "", 0n))), "local.board.catalog"), "")
    C.Check.str("catalog browse keeps desired viewer", U.Ui.view_get(opened, "view.path"), U.Ui.view_get(large, "view.path"))
    C.Check.str("catalog browse keeps draft", U.Ui.scratch(opened, "self"), "draft 日本語")
    C.Check.str("generic pack keeps canonical cache", Bool.show(String.is_empty(J.Json.get_str(J.Json.get_obj(S.Session.pack(large), "info"), "fab.boards:self", ""))), "False")
    C.Check.str("thin pack omits derived catalog only", J.Json.get_str(J.Json.get_obj(V.Route.pack(large), "info"), "fab.boards:self", "missing"), "missing")
    C.Check.str("thin pack keeps full outbox", J.Json.show(J.Json.get_obj(V.Route.pack(large), "outbox")), J.Json.show(J.Json.get_obj(S.Session.pack(large), "outbox")))
    C.Check.str("thin pack keeps all drafts", J.Json.show(J.Json.get_obj(V.Route.pack(large), "drafts")), J.Json.show(J.Json.get_obj(S.Session.pack(large), "drafts")))
    C.Check.str("thin pack keeps flags", J.Json.show(J.Json.get_obj(V.Route.pack(large), "open")), J.Json.show(J.Json.get_obj(S.Session.pack(large), "open")))
    C.Check.str("catalog actions keep canonical origin", U.Ui.info(opened, "origin"), "canonical-owner-origin")
    C.Check.str("catalog action keeps canonical full catalog", U.Ui.info(opened, "fab.boards:self"), U.Ui.info(large, "fab.boards:self"))
    C.Check.str("stale source rejects a changed catalog", U.Ui.info(U.Act.ui(U.Bd.catalog(ui(2047n), B.Catalog.command(U.Bd.catalog.source(large), True{}, "", 0n))), "local.board.catalog"), "")
    C.Check.str("request count cannot enlarge server page", Nat.show(options(W.View.boards.catalog(U.Act.ui(U.Bd.catalog(large, J.Json.show(J.Json.obj([J.KV{"source", J.Str{U.Bd.catalog.source(large)}}, J.KV{"open", J.Flag{True{}}}, J.KV{"limit", J.Num{"999999999"}}]))))))), "17")
    C.Check.str("thin pack keeps default board", J.Json.get_str(J.Json.get_obj(V.Route.pack(large), "info"), "fab.board:self", ""), "boards/board1/main.kicad_pcb")
    +moved : J.Json = result(U.Ui.select.k(small, "other"), True{})
    C.Check.str("compact changed target is not falsely ready", Bool.show(J.Json.get_bool(moved, "remoteReady", True{})), "False")
    C.Check.str("compact changed target retains normal nextTarget", J.Json.get_str(J.Json.get_obj(moved, "nextTarget"), "id", ""), "other")
    +plain : J.Json = result(large, False{})
    +compact : J.Json = result(large, True{})
    C.Check.str("compact session byte-for-byte preserved", J.Json.show(J.Json.get_obj(compact, "session")), J.Json.show(J.Json.get_obj(plain, "session")))
    C.Check.str("compact commands unchanged", J.Json.show(Maybe.default(&2, J.Json, J.Json.get(compact, "cmds"), J.Null{})), J.Json.show(Maybe.default(&2, J.Json, J.Json.get(plain, "cmds"), J.Null{})))
    C.Check.str("compact keeps complete workbench ancestor tree and canvas", canvas(W.View.solid.loc(I.En{}, U.Ui.info_set(large, "local.view.compact", "1"))), canvas(W.View.solid.loc(I.En{}, large)))
    C.Check.str("compact window unchanged", J.Json.show(J.Json.get_obj(compact, "window")), J.Json.show(J.Json.get_obj(plain, "window")))
    C.Check.str("compact diagnostic text bounded", Nat.show(String.length(W.View.compact.text(True{}, String.join(ids(100n), "\n")))), "513")
    C.Check.str("capacity distinguishes actor refusal", J.Json.get_str(CAP.Capacity.report(compact, False{}, 1n), "reason", ""), "actor-capacity")
    C.Check.str("capacity measures whole session", Nat.show(J.Json.get_nat(CAP.Capacity.report(compact, True{}, 1n), "sessionBytes", 0n)), Nat.show(J.Json.bytes(J.Json.get_obj(compact, "session"))))
    +huge : J.Json = result(U.Ui.scratch_set(large, "self", String.join(ids(12000n), "\n")), True{})
    C.Check.str("compact never deletes an oversized session", Bool.show(Nat.is_gt(J.Json.bytes(J.Json.get_obj(huge, "session")), 262144n)), "True")
    C.Check.str("component counters expose oversized session", Bool.show(Nat.is_gt(J.Json.get_nat(CAP.Capacity.report(huge, True{}, J.Json.bytes(huge)), "sessionBytes", 0n), 262144n)), "True")
    IO.print("MEASURE " ++ J.Json.show(J.Json.obj([
      J.KV{"catalogSize", J.Num{"2048"}},
      J.KV{"closedNodesBytes", J.Json.nat(J.Json.bytes(N.Node.write(D.Dom.div("", W.View.boards.catalog(large)))))},
      J.KV{"genericSessionBytes", J.Json.nat(J.Json.bytes(S.Session.pack(large)))},
      J.KV{"negotiatedSessionBytes", J.Json.nat(J.Json.bytes(V.Route.pack(large)))}])))
