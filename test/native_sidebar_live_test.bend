import Base
import ./check.bend as C
import ../src/server/server.bend as S
import ../src/server/botnet.bend as BN
import ../src/server/kicad.bend as K
import ../src/core/native-sidebar.bend as N
import ../src/core/thin-live.bend as L
import ../src/core/client.bend as U
import ../src/core/json.bend as J

def cfg() -> S.Cfg:
  S.Cfg{"/tmp/unused-native-sidebar", "", "127.0.0.1", 0, "", Nil{}, "test",
    S.Net{False{}, False{}, "", "", "", K.Rt.empty(), None{}, None{}, None{}, "[]", None{}, BN.Bn.empty(), True{}, "", "", None{}}, Nil{}, Nil{}}

def request() -> J.Json:
  J.Json.obj([J.KV{"nativeSidebar", J.Num{"1"}}, J.KV{"hub", J.Str{"h"}}, J.KV{"generation", J.Str{"1"}},
    J.KV{"target", J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{"a"}}])}])

def ui() -> U.Ui:
  N.Ns.prepare(U.Ui.init(1n), False{}, request(), "epoch", "view", S.Cfg.thin.signal_seq(cfg()), "00112233445566778899aabbccddeeff")

def page(u: U.Ui) -> N.Slice:
  N.Ns.slice(u, ["a", "b", "c", "d"], "active", "", 2n, "a")

def paged() -> U.Ui:
  N.Ns.accept(ui(), "h|" ++ J.Json.get_str(J.Json.get_obj(N.Slice.pager(page(ui())), "next"), "value", ""))

def after_live() -> S.Cfg:
  S.Cfg.thin.live(cfg(), Map.set(&2, L.Stream, Map.new(&2, L.Stream), "other", L.Stream.blank()), "other")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("actual unrelated live update advances picture revision", Bool.show(Nat.is_gt(S.Cfg.thin.revision(after_live()), S.Cfg.thin.revision(cfg()))), "True")
    C.Check.str("actual unrelated live update preserves metadata revision", Nat.show(S.Cfg.thin.signal_seq(after_live())), Nat.show(S.Cfg.thin.signal_seq(cfg())))
    C.Check.str("actual live-only bump preserves adopted page position", Nat.show(J.Json.get_nat(N.Slice.pager(page(N.Ns.prepare(paged(), False{}, request(), "epoch", "view", S.Cfg.thin.signal_seq(after_live()), "00112233445566778899aabbccddeeff"))), "offset", 99n)), "2")
