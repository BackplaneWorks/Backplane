import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/ime.bend as IM
import ../src/app/nui.bend as N
import ../src/app/win.bend as W
import ../src/app/layout.bend as L

# The window's input method (nui.bend's Imx, core/ime.bend): what is being
# composed is shown at the caret and is never the draft; its commit goes in
# once, as one undo step, as plain text; the keys the method owns do not
# reach the composer; and a composition stays with its field.

def para(+i: Nat) -> String:
  "Message " ++ Nat.show(i) ++ ": the regulator's enable pin is pulled up through R12 to VBAT, so the output comes up before the MCU "
    ++ "drives it low. Moving R12 to the 3V3 rail fixes the sequencing, and C7 can stay where it is. **Next:** check the footprint "
    ++ "of U3 against the datasheet, then rerun DRC.\n\n- item one\n- item two with `code`\n"

def msg(+k: Nat, +th: String, +at: Nat) -> M.Change:
  M.MessagePosted{th, th ++ "m" ++ Nat.show(k), Bool.pick(M.Role, Nat.is_eq(Nat.mod(k, 2n), 0n), M.User{}, M.Assistant{}), para(k), Nat.add(at, k)}

def msgs(+n: Nat, +th: String, +at: Nat) -> List<&2, M.Change>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      msg(k, th, at) <> msgs(k, th, at)

def short(+k: Nat, +at: Nat) -> List<&2, M.Change>:
  M.ThreadCreated{"t" ++ Nat.show(1n+k), "p1", "Thread " ++ Nat.show(k), M.Local{}, "", "", "claude", "", at} <> msgs(20n, "t" ++ Nat.show(1n+k), at)

def threads(+n: Nat, +at: Nat) -> List<&2, M.Change>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      List.append(&2, M.Change, short(k, at), threads(k, at))

def log(+at: Nat) -> List<&2, M.Change>:
  List.concat(&2, M.Change, [[M.ProjectCreated{"p1", "board", "/tmp/board", at}, M.ThreadCreated{"t0", "p1", "Power sequencing", M.Local{}, "", "", "claude", "", at}],
    msgs(12n, "t0", at), threads(2n, at)])

def ui_sel(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}:
      hs

def with_hits(n: N.Nui, hs: List<&2, N.Hit>, +ms: Nat) -> N.Nui:
  match n:
    case N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, _, _, vw}:
      N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hs, ms, vw}

def prepared(+n: N.Nui, +ms: Nat) -> N.Nui:
  N.Lbx.stamp(N.Blink.stamp(N.Hover.stamp(N.Nui.prepare(n), ms), ms), ms)

def step_nui(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n, _, _}:
      n

def again(+ty: L.Ty, +n0: N.Nui) -> N.Nui:
  +n = prepared(n0, 0n)
  with_hits(n, hits(L.Lay.frame(ty, L.Lay.memo(ty, n, L.TL.memo.none()), n)), 0n)

def ev(+n: N.Nui, e: W.WinEv) -> N.Nui:
  step_nui(N.Nui.event(n, e))


def key(+n: N.Nui, +code: U32, +text: U32, +mods: U32) -> N.Nui:
  ev(n, W.WKey{code, text, mods, True{}, 0})

def typed(s: String, +n: N.Nui) -> N.Nui:
  match s:
    case SNil{}:
      n
    case SCon{Chr{+c}, +t}:
      typed(t, key(n, c, c, 0))

def draft(+n: N.Nui) -> String:
  U.Ui.draft_text(N.Nui.ui(n))

def cmds(s: N.Step) -> Nat:
  match s:
    case N.Step{_, cs, _}:
      List.length(&2, N.NCmd, cs)

def b(x: Bool) -> String:
  Bool.pick(String, x, "yes", "no")

def pre(+n: N.Nui, +t: String, +f: String, +caret: U32) -> N.Nui:
  ev(n, W.WPreedit{0, 0, 0, t, f, caret, False{}})

def kind(e: W.WinEv) -> String:
  match e:
    case W.WQuit{}:
      "quit"
    case W.WJunk{}:
      "junk"
    case W.WCommit{id, _}:
      "commit" ++ U32.show(id)
    case W.WPreedit{first, len, id, _, _, caret, only}:
      "pre" ++ U32.show(first) ++ "," ++ U32.show(len) ++ "," ++ U32.show(id) ++ "," ++ U32.show(caret) ++ Bool.pick(String, only, "f", "")
    case W.WPreCaret{pos}:
      "caret" ++ U32.show(pos)
    case W.WPreStart{}:
      "start"
    case W.WPreEnd{}:
      "end"
    case W.WKey{_, _, _, _, _}:
      "key"
    case _:
      "other"

def kinds(es: List<&2, W.WinEv>) -> String:
  match es:
    case Nil{}:
      ""
    case e <> t:
      kind(e) ++ " " ++ kinds(t)

def main() -> IO(Unit):
  do IO<Unit>:
    +ty : L.Ty <- L.Ty.load()
    +u : U.Ui = ui_sel(U.Ui.act(U.Ui.fold(log(1790000000n), U.Ui.init(1790000100n)), "select", "t0"))
    +n0 : N.Nui = again(ty, step_nui(N.Nui.focus(N.Nui.init(u, 1400, 900), "composer")))
    +n1 : N.Nui = typed("ab", n0)
    +n2 : N.Nui = pre(n1, "にほんご", "2222", 4)
    +n3 : N.Nui = ev(n2, W.WKey{65293, 0, 0, True{}, 0})
    +n4 : N.Nui = ev(n3, W.WKey{65307, 0, 0, True{}, 0})
    +n5 : N.Nui = ev(ev(pre(n3, "日本語", "044", 3), W.WCommit{0, "日本語"}), W.WPreEnd{})
    +n6 : N.Nui = key(n5, 122, 122, 2)
    +n7 : N.Nui = ev(n1, W.WCommit{0, "file:///tmp/x.png"})
    +n8 : N.Nui = ev(n2, W.WBlur{})
    +n9 : N.Nui = step_nui(N.Nui.focus(n2, "attach-path"))
    +ia : N.Nui = ev(pre(n1, "か", "2", 1), W.WPreCaret{0})
    +fr : L.Out = L.Lay.frame(ty, L.Lay.memo(ty, n2, L.TL.memo.none()), n2)
    do IO<Unit>:
      C.Check.str("typing before", draft(n1), "ab")
      C.Check.str("a preedit leaves the draft", draft(n2), "ab")
      C.Check.str("it is shown at the caret", N.Imx.draft(n2), "abにほんご")
      C.Check.str("the caret is inside it", Nat.show(N.Imx.at(n2)), "6")
      C.Check.str("a caret move inside it", Nat.show(N.Imx.at(ia)), "2")
      C.Check.str("it is one underlined run", Nat.show(List.length(&2, IM.Run, N.Imx.runs(n2))), "1")
      C.Check.str("it is live", b(N.Imx.live(n2)), "yes")
      C.Check.str("the frame lays out with it", b(Nat.is_gt(List.length(&2, N.Hit, hits(fr)), 0n)), "yes")
      C.Check.str("Enter during it does not send", Nat.show(cmds(N.Nui.event(n2, W.WKey{65293, 0, 0, True{}, 0}))), "0")
      C.Check.str("Enter during it leaves the draft", draft(n3), "ab")
      C.Check.str("Enter during it leaves the composition", b(N.Imx.live(n3)), "yes")
      C.Check.str("Escape does not leave the field", N.Nui.focus_of(n4), "composer")
      C.Check.str("Escape ends the composition", b(N.Imx.live(n4)), "no")
      C.Check.str("a commit is typed in", draft(n5), "ab日本語")
      C.Check.str("it ends the composition", b(N.Imx.live(n5)), "no")
      C.Check.str("one undo undoes the whole commit", draft(n6), "ab")
      C.Check.str("a path commit is text", draft(n7), "abfile:///tmp/x.png")
      C.Check.str("it attaches nothing", Nat.show(List.length(&2, String, U.Ui.atts(N.Nui.ui(n7), "t0"))), "0")
      C.Check.str("a blur cancels", b(N.Imx.live(n8)), "no")
      C.Check.str("a blur inserts nothing", draft(n8), "ab")
      C.Check.str("another field does not own it", b(N.Imx.stale(n9)), "yes")
      C.Check.str("nor does it show there", b(N.Imx.mine(n9)), "no")
      C.Check.str("the owner keeps it", b(N.Imx.stale(n2)), "no")
      C.Check.str("decoded kinds", kinds(W.Win.decode([14, 3, 0, 0, 0, 15, 1, 2, 4, 2147483651, 16, 2, 0, 0, 0, 17, 0, 0, 0, 0, 18, 0, 0, 0, 0, 6, 0, 0, 0, 0, 99, 0, 0, 0, 0])),
        "commit3 pre1,2,4,3f caret2 start end quit junk ")
