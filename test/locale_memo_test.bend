import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/app/nui.bend as N
import ../src/app/layout.bend as L
import ../src/app/mm.bend as MM

def selected(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def nui(+tag: String) -> N.Nui:
  N.Nui.init(U.Ui.scratch_set(selected(U.Ui.act(U.Ui.init(1000n), "select", "t")), "@lang.tag", tag), 1400, 900)

# Deliberately fake cached output makes retaining an old language visible
# even before translated labels are installed by the localization branch.
def poison(+tag: String) -> L.TLMemo:
  L.TLMemo{780, "t", [L.TLE{M.Msg{"e", "t", M.Assistant{}, "body", 1n}, L.TLPart{[L.TLine{"old language", 0, 3, "", 0}], Nil{}, Nil{}}}],
    L.TL.live.local(tag, L.LiveMemo{780, "prefix\n\n", True{}, [L.TLine{"old language", 0, 3, "", 0}]})}

def live.first(lm: L.LiveMemo) -> String:
  match lm:
    case L.LiveMemo{_, _, _, L.TLine{t, _, _, _, _} <> _}:
      t
    case L.LiveLocal{_, _, _, _, L.TLine{t, _, _, _, _} <> _}:
      t
    case _:
      ""

def main() -> IO(Unit):
  do IO<Unit>:
    +ty : L.Ty = L.Ty.none()
    +tc : MM.TouchK = MM.TouchK.none()
    +en : N.Nui = nui("en")
    +ja : N.Nui = nui("ja")
    +sux : N.Nui = nui("sux-Xsux")
    C.Check.str("missing locale is English", L.TL.locale(N.Nui.ui(nui(""))), "en")
    C.Check.str("unknown locale is English", L.TL.locale(N.Nui.ui(nui("unknown"))), "en")
    C.Check.str("cuneiform locale is preserved", L.TL.locale(N.Nui.ui(sux)), "sux-Xsux")
    C.Check.str("main key changes on Japanese switch", Bool.show(String.eq(MM.Mm.key(en, tc, Nil{}), MM.Mm.key(ja, tc, Nil{}))), "False")
    C.Check.str("sidebar key changes on Japanese switch", Bool.show(String.eq(MM.Sm.key(en, tc), MM.Sm.key(ja, tc))), "False")
    C.Check.str("main key changes on cuneiform switch", Bool.show(String.eq(MM.Mm.key(en, tc, Nil{}), MM.Mm.key(sux, tc, Nil{}))), "False")
    C.Check.str("same locale retains cached entries", Nat.show(L.TL.memo.size(L.TL.memo.locale("en", poison("en")))), "1")
    C.Check.str("same Japanese locale retains cached entries", Nat.show(L.TL.memo.size(L.TL.memo.locale("ja", poison("ja")))), "1")
    C.Check.str("same cuneiform locale retains cached entries", Nat.show(L.TL.memo.size(L.TL.memo.locale("sux-Xsux", poison("sux-Xsux")))), "1")
    C.Check.str("same Japanese locale retains cached prefix", live.first(L.TL.memo.lv(L.TL.memo.locale("ja", poison("ja")))), "old language")
    C.Check.str("Japanese switch drops English entries", Nat.show(L.TL.memo.size(L.TL.memo.locale("ja", poison("en")))), "0")
    C.Check.str("cuneiform switch drops English entries", Nat.show(L.TL.memo.size(L.TL.memo.locale("sux-Xsux", poison("en")))), "0")
    C.Check.str("Japanese switch drops English prefix", live.first(L.TL.memo.lv(L.TL.memo.locale("ja", poison("en")))), "")
    C.Check.str("English switch drops Japanese entries", Nat.show(L.TL.memo.size(L.TL.memo.locale("en", poison("ja")))), "0")
    C.Check.str("English switch drops Japanese prefix", live.first(L.TL.memo.lv(L.TL.memo.locale("en", poison("ja")))), "")
    +mj : L.TLMemo = L.Lay.memo(ty, ja, poison("en"))
    +me : L.TLMemo = L.Lay.memo(ty, en, mj)
    C.Check.str("Japanese layout records its locale", L.TL.live.tag(L.TL.memo.lv(mj)), "ja")
    C.Check.str("return to English records its locale", L.TL.live.tag(L.TL.memo.lv(me)), "en")
    C.Check.str("growing prefix retains Japanese identity", L.TL.live.tag(L.TL.live.get(ty, L.TL.memo.lv(mj), "new prefix\n\n", 780)), "ja")
