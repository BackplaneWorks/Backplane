import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/core/fallback.bend as F
import ../src/web/view.bend as W
import ../src/web/dom.bend as D
import ../src/mobile/screen.bend as P
import ../src/app/layout.bend as L
import ../src/app/nui.bend as N

def modelinfo() -> String:
  "{\"claude\":[\"sonnet\"],\"codex\":[\"gpt-6.1-sol\",\"test-next\"]}"

def state() -> M.State:
  M.State.replay([M.SettingSet{"fallback.chain", "claude/sonnet,codex/gpt-6.1-sol", 1n}, M.SettingSet{"agent.provider", "codex", 1n}, M.SettingSet{"agent.model", "gpt-6.1-sol", 1n}], M.State.empty())

def ui() -> U.Ui:
  U.Ui.info_set(U.Ui.far.st(U.Ui.init(10n), state()), "models", modelinfo())

def texts(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}: ""
    case D.Txt{s} <> t: s ++ " " ++ texts(t)
    case D.El{_, _, _, kids} <> t: texts(kids) ++ texts(t)

def vals(as: List<&2, D.Attr>) -> String:
  match as:
    case Nil{}: ""
    case D.At{"data-value", v} <> t: v ++ "\n" ++ vals(t)
    case _ <> t: vals(t)

def buttons(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}: ""
    case D.El{_, _, as, kids} <> t: vals(as) ++ buttons(kids) ++ buttons(t)
    case _ <> t: buttons(t)

def rowtext(rs: List<&2, U.AgRow>) -> String:
  match rs:
    case Nil{}: ""
    case U.AgRow{l, n, _, _} <> t: l ++ " " ++ n ++ "\n" ++ rowtext(t)

def hit(h: N.Hit) -> String:
  match h:
    case N.Hit{_, _, _, _, a, v}: a ++ ":" ++ v ++ "\n"

def hits(hs: List<&2, N.Hit>) -> String:
  match hs:
    case Nil{}: ""
    case h <> t: hit(h) ++ hits(t)

def window(o: L.Out) -> String:
  match o:
    case L.Out{_, hs}: hits(hs)

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = ui()
    +rs : List<&2, U.AgRow> = U.Fallbackv.rows(u)
    +web : List<&2, D.Node> = W.View.agy.rows(rs)
    +phone : String = J.Json.show(J.Json.arr(P.Mob.agy.rows(rs)))
    +win : String = window(L.Set.agy.rows(rs, L.Ty.none(), N.Nui.init(u, 1400, 1000), 20, 680, 20))
    C.Check.str("shared Settings copy explains inheritance", Bool.show(String.contains(rowtext(rs), "delegates, routines and mail runs")), "True")
    C.Check.str("shared Settings copy preserves explicit selection", Bool.show(String.contains(rowtext(rs), "Your chosen model runs first")), "True")
    C.Check.str("web render contains fallback hierarchy", Bool.show(String.contains(texts(web), "Fallback hierarchy")), "True")
    C.Check.str("web add chooses exact Codex target", Bool.show(String.contains(buttons(web), "fallback.chain=claude/sonnet,codex/gpt-6.1-sol")), "True")
    C.Check.str("web controls reorder", Bool.show(String.contains(buttons(web), "fallback.chain=codex/gpt-6.1-sol,claude/sonnet")), "True")
    C.Check.str("phone Settings render", Bool.show(String.contains(phone, "Fallback hierarchy")), "True")
    C.Check.str("phone remove control", Bool.show(String.contains(phone, "Remove")), "True")
    C.Check.str("phone enable control", Bool.show(String.contains(phone, "fallback.on=on")), "True")
    C.Check.str("window Settings hit enable", Bool.show(String.contains(win, "setting:fallback.on=on")), "True")
    C.Check.str("window Settings hit reorder", Bool.show(String.contains(win, "fallback.chain=codex/gpt-6.1-sol,claude/sonnet")), "True")
    C.Check.str("config does not overwrite migrated defaults", M.Setting.get(U.Ui.state.of(u), "agent.model", ""), "gpt-6.1-sol")
