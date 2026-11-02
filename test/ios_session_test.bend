import Base
import ./check.bend as C
import ./hubs_test.bend as F
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/mobile/hubs.bend as H
import ../src/mobile/session.bend as S
import ../src/core/i18n.bend as I

def canonical() -> U.Ui:
  U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.info_set(U.Ui.info_set(U.Ui.online(H.Hubs.ui(F.two(), "box:3787"), True{}), "origin", "real-origin"), "local.generation", "real-generation"), "@boot", "1"), "@sync", "1")

def presentation() -> U.Ui:
  U.Ui.info_set(U.Ui.scratch_set(U.Ui.select.k(U.Ui.init(900n), "t2"), "t2", "unsent"), "fold:m2", "1")

def malicious() -> J.Json:
  J.Json.obj([J.KV{"sel", J.Json.str("t2")}, J.KV{"seq", J.Json.nat(999999n)},
    J.KV{"info", J.Json.obj([J.KV{"origin", J.Json.str("wrong-origin")}, J.KV{"local.generation", J.Json.str("wrong-generation")}, J.KV{"fold:m2", J.Json.str("1")}])},
    J.KV{"drafts", J.Json.obj([J.KV{"@boot", J.Json.str("")}, J.KV{"@sync", J.Json.str("")}, J.KV{"t2", J.Json.str("unsent")}])}])

def adopted() -> U.Ui:
  S.Session.adopt(canonical(), malicious())

def live() -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str("live")}, J.KV{"items", J.Json.arr([])}])

def input() -> U.Ui:
  S.Session.input(U.Ui.state(canonical()), J.Json.obj([]), live(), 900n, "phone", S.Session.pack(presentation()))

def result() -> J.Json:
  S.Session.result(S.Session.act(input(), "box:3787", "phone", 900n, "draft", "new unsent"), "box:3787")

def send() -> H.HAct:
  S.Session.act(U.Ui.scratch_set(input(), "t2", "one delivery"), "box:3787", "phone", 900n, "send", "")

def cache.fixture() -> U.Ui:
  U.Ui.scratch_set(presentation(), "@img:old", "=/tmp/old.png")

def page.fixture() -> U.Ui:
  U.Ui.adopt(presentation(), M.State.apply(M.State.apply(U.Ui.state(canonical()),
    M.MessagePosted{"t2", "visible-image", M.Assistant{}, "See /tmp/visible.png", 901n}),
    M.MessagePosted{"t1", "other-image", M.Assistant{}, "See /tmp/other.png", 902n}))

def page.hubs(u: U.Ui) -> H.Hubs:
  H.Hubs{"phone", 903n, "box:3787", [H.Hub{"box:3787", u}]}

def page.oracle() -> J.Json:
  S.Session.result.shown(page.hubs(U.Ui.scratch_set(page.fixture(), "@img:visible-image", "=/tmp/visible.png")), Nil{}, "box:3787",
    H.Hubs.shown(I.En{}, H.Hubs.list(page.hubs(U.Ui.scratch_set(page.fixture(), "@img:visible-image", "=/tmp/visible.png"))), "box:3787"))

def sends(cs: List<&2, H.HCmd>, +method: String) -> Nat:
  match cs:
    case Nil{}: 0n
    case H.HCmd{_, U.Send{text}} <> t:
      Nat.add(Bool.pick(Nat, String.eq(J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), "m", ""), method), 1n, 0n), sends(t, method))
    case _ <> t: sends(t, method)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("derived image cache never crosses presentation transport", J.Json.get_str(J.Json.get_obj(S.Session.pack(cache.fixture()), "drafts"), "@img:old", "private"), "private")
    C.Check.str("canonical image cache survives compact session adoption", U.Ui.scratch(S.Session.adopt(cache.fixture(), S.Session.pack(presentation())), "@img:old"), "=/tmp/old.png")
    C.Check.str("explicit draft identity preserves selection", J.Json.get_str(S.Session.draft.at(S.Session.pack(presentation()), "other", "new other"), "sel", ""), "t2")
    C.Check.str("explicit draft identity preserves selected draft", J.Json.get_str(J.Json.get_obj(S.Session.draft.at(S.Session.pack(presentation()), "other", "new other"), "drafts"), "t2", ""), "unsent")
    C.Check.str("explicit draft identity updates original thread", J.Json.get_str(J.Json.get_obj(S.Session.draft.at(S.Session.pack(presentation()), "other", "new other"), "drafts"), "other", ""), "new other")
    C.Check.str("queued permission uses the thread visible at tap", J.Json.get_str(S.Session.prepare(S.Session.pack(U.Ui.select.k(presentation(), "other")), S.Session.pack(presentation()), "thread-permissions"), "sel", ""), "t2")
    C.Check.str("queued permission cannot restore a sent draft", J.Json.get_str(J.Json.get_obj(S.Session.prepare(S.Session.pack(U.Ui.scratch_set(U.Ui.select.k(presentation(), "other"), "t2", "")), S.Session.pack(presentation()), "thread-permissions"), "drafts"), "t2", ""), "")
    C.Check.str("compact session still carries unsent draft", J.Json.get_str(J.Json.get_obj(S.Session.pack(cache.fixture()), "drafts"), "t2", "missing"), "unsent")
    C.Check.str("compact adoption does not restore a cleared draft", U.Ui.scratch(S.Session.adopt(cache.fixture(), S.Session.pack(U.Ui.scratch_set(presentation(), "t2", ""))), "t2"), "")
    C.Check.str("fresh presentation derives visible message images", U.Ui.scratch(S.Session.images(page.fixture()), "@img:visible-image"), "=/tmp/visible.png")
    C.Check.str("fresh presentation does not scan another thread images", U.Ui.scratch(S.Session.images(page.fixture()), "@img:other-image"), "")
    C.Check.str("compact image presentation matches independently cached full screen", J.Json.show(J.Json.get_obj(S.Session.result(H.HAct{page.hubs(page.fixture()), Nil{}, Nil{}}, "box:3787"), "screen")), J.Json.show(J.Json.get_obj(page.oracle(), "screen")))
    C.Check.str("cached reading page carries old newest mark", U.Ui.scratch(S.Session.adopt(canonical(), S.Session.reading(S.Session.pack(presentation()), "t2", "cached-tail", 80n, True{})), "@away.t2"), "cached-tail")
    C.Check.str("cached reading retains enough old entries", U.Ui.scratch(S.Session.adopt(canonical(), S.Session.reading(S.Session.pack(presentation()), "t2", "cached-tail", 80n, True{})), "@more.t2"), "2")
    C.Check.str("cached reading never advances replay cursor", Nat.show(U.Ui.seq(S.Session.adopt(canonical(), S.Session.reading(S.Session.pack(presentation()), "t2", "cached-tail", 80n, True{})))), Nat.show(U.Ui.seq(canonical())))
    C.Check.str("intentional latest clears cached reading mark", U.Ui.scratch(S.Session.adopt(canonical(), S.Session.reading(S.Session.reading(S.Session.pack(presentation()), "t2", "cached-tail", 80n, True{}), "t2", "new-tail", 20n, False{})), "@away.t2"), "")
    C.Check.str("handoff never advances canonical cursor", Nat.show(U.Ui.seq(adopted())), Nat.show(U.Ui.seq(canonical())))
    C.Check.str("handoff keeps canonical origin", U.Ui.info(adopted(), "origin"), "real-origin")
    C.Check.str("handoff keeps connection generation", U.Ui.info(adopted(), "local.generation"), "real-generation")
    C.Check.str("handoff keeps boot flag", U.Ui.scratch(adopted(), "@boot"), "1")
    C.Check.str("handoff keeps sync flag", U.Ui.scratch(adopted(), "@sync"), "1")
    C.Check.str("handoff keeps person selection", U.Ui.sel(adopted()), "t2")
    C.Check.str("handoff keeps unsent draft", U.Ui.scratch(adopted(), "t2"), "unsent")
    C.Check.str("handoff keeps fold choice", U.Ui.info(adopted(), "fold:m2"), "1")
    C.Check.str("input uses current authoritative cursor", Nat.show(U.Ui.seq(input())), Nat.show(U.Ui.seq(canonical())))
    C.Check.str("server presentation input ready", U.Ui.scratch(input(), "@sync"), "1")
    C.Check.str("server input selected recent thread", U.Ui.sel(input()), "t2")
    C.Check.str("empty presentation exposes raw reading tail", J.Json.get_str(J.Json.get_obj(J.Json.get_obj(result(), "screen"), "thread"), "readingSeen", "missing"), "-")
    C.Check.str("response marked presentation", Nat.show(J.Json.get_nat(result(), "mobile", 0n)), "1")
    C.Check.str("response has selected real thread", J.Json.get_str(J.Json.get_obj(J.Json.get_obj(result(), "screen"), "thread"), "id", ""), "box:3787|t2")
    C.Check.str("person action saved in transient session", J.Json.get_str(J.Json.get_obj(J.Json.get_obj(result(), "session"), "drafts"), "t2", ""), "new unsent")
    C.Check.str("draft action never sends a message", Nat.show(sends(H.Hubs.cmds(S.Session.act(input(), "box:3787", "phone", 900n, "draft", "new unsent")), "turn.start")), "0")
    C.Check.str("send returns exactly one ordinary turn command", Nat.show(sends(H.Hubs.cmds(send()), "turn.start")), "1")
    C.Check.str("send clears serialized draft", J.Json.get_str(J.Json.get_obj(S.Session.pack(H.Hubs.ui(H.Hubs.after(send()), "box:3787")), "drafts"), "t2", "nonempty"), "")
    C.Check.str("pure rendering cannot post a message", Nat.show(List.length(&2, M.Entry, M.State.mine(U.Ui.state(H.Hubs.ui(H.Hubs.after(send()), "box:3787")), "t2"))), "0")
