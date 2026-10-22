import Base
import ./check.bend as C
import ../src/server/server.bend as S
import ../src/server/botnet.bend as BN
import ../src/server/kicad.bend as K
import ../src/core/store.bend as ST
import ../src/core/model.bend as M
import ../src/core/sign.bend as SG
import ../src/core/far.bend as FR
import ../src/core/codec.bend as CC

def cfg(home: String, bn: BN.Bn) -> S.Cfg:
  S.Cfg{home, "", "127.0.0.1", 0, "", Nil{}, "test",
    S.Net{False{}, False{}, "", "", "", K.Rt.empty(), None{}, None{}, None{}, "[]", None{}, bn, True{}, "", "", None{}}, Nil{}, Nil{}}

def disk() -> List<&2, String>:
  ["{\"$\":\"SettingSet\",\"key\":\"test\",\"value\":\"yes\",\"at\":1}",
   "{\"$\":\"UnknownFutureChange\",\"at\":2}",
   "{",
   "{\"$\":\"DeviceDropped\",\"token\":\"synthetic-private-token\",\"at\":3}"]

def loaded() -> ST.Loaded:
  ST.Store.load(disk(), ST.Loaded{M.State.empty(), Nil{}})

def count(name: String, cfg: S.Cfg, want: Nat) -> IO(Unit):
  C.Check.str(name, Nat.show(S.LogCount.n(cfg)), Nat.show(want))

def bn_count(name: String, bn: BN.Bn) -> IO(Unit):
  C.Check.str(name, Nat.show(BN.Bn.wire_n(bn)), "137")

def main() -> IO(Unit):
  do IO<Unit>:
    +ld : ST.Loaded = loaded()
    +wire : List<&2, String> = ST.Loaded.wire(ld)
    +c : S.Cfg = S.LogCount.set(cfg("/tmp/unused-mailboot", BN.Bn.empty()), List.length(&2, String, wire))
    C.Check.str("torn object is excluded from replay wire", Nat.show(List.length(&2, String, ST.Loaded.wire(ST.Store.load(["{"], ST.Loaded{M.State.empty(), Nil{}})))), "0")
    count("initialized count equals real replay wire", c, 3n)
    C.Check.str("unknown JSON makes seq differ from wire count", Nat.show(M.State.seq(ST.Loaded.st(ld))), "2")
    C.Check.str("private token absent from replay wire", Bool.show(String.contains(String.join(wire, ""), "synthetic-private-token")), "False")
    count("fresh wire lines increment count", S.LogCount.committed(c, ["fresh-one", "fresh-two"]), 5n)
    count("empty commit preserves count", S.LogCount.committed(c, Nil{}), 3n)
    +b : BN.Bn = BN.Bn.with_wire_n(BN.Bn.empty(), 137n)
    bn_count("directory setter preserves count", BN.Bn.with_dir(b, "[]"))
    bn_count("seen setter preserves count", BN.Bn.with_seen(b, SG.Seen.empty()))
    bn_count("work setter preserves count", BN.Bn.with_work(b, BN.Bn.work(b)))
    bn_count("far mirror setter preserves local count", BN.Bn.with_far(b, BN.Bn.far(b)))
    bn_count("live setter preserves count", BN.Bn.with_live(b, Nil{}))
    bn_count("broken setter preserves count", BN.Bn.with_broken(b, "held"))
    bn_count("far live setter preserves local count", BN.Bn.with_fl(b, Nil{}))
    bn_count("tools setter preserves count", BN.Bn.with_tools(b, ["tool"]))
    bn_count("teach setter preserves count", BN.Bn.with_tch(b, BN.Bn.tch(b)))
    +mirror : FR.In = FR.In.put(FR.In.new("synthetic-peer", "Peer"), 0n, 1n, [FR.Item{1n, CC.Change.encode(M.SettingSet{"peer.test", "yes", 1n})}])
    +far : BN.Bn = BN.Bn.with_far(b, FR.Fh.in.set(BN.Bn.far(b), mirror))
    C.Check.str("mirror fixture actually has one remote item", Nat.show(FR.In.head(FR.Fh.in(BN.Bn.far(far), "synthetic-peer", "Peer"))), "1")
    bn_count("nonempty mirror adds no local wire lines", far)
