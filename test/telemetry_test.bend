import Base
import ../src/core/i18n.bend as I
import ../src/core/model.bend as M
import ./check.bend as C
import ./telemetry_cases.bend as F
import ../src/core/telemetry.bend as T
import ../src/core/client.bend as U
import ../src/core/onboard.bend as O
import ../src/core/sign.bend as S

def check(name: String, b: Bool) -> IO(Unit):
  C.Check.str(name, Bool.pick(String, b, "yes", "no"), "yes")
def count(st: T.TmState) -> String:
  Nat.show(List.length(&2, T.TmEv, T.TmState.queue(st)))
def held() -> T.TmState:
  F.Cases.unasked()
def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("first install held", count(held()), "1")
    C.Check.str("unasked ready empty", Nat.show(List.length(&2, T.TmEv, T.Tm.ready(T.TmState.queue(held()), 2n))), "0")
    check("unasked no id", Bool.not(String.contains(F.Cases.body(held()), "\"id\"")))
    check("opt-out body no id", Bool.not(String.contains(F.Cases.body(F.Cases.off()), "\"id\"")))
    C.Check.str("opt-out drops active", count(F.Cases.off()), "2")
    C.Check.str("day rollover once", count(F.Cases.twice()), "1")
    C.Check.str("update", count(F.Cases.run(F.Cases.st(Nil{}), F.Cases.view(True{}, True{}, "0", "0.11.4", "2026-10-14", 2n))), "1")
    C.Check.str("re-enable rotates", T.TmState.id(F.Cases.run(F.Cases.off(), F.Cases.view(True{}, True{}, "0", "0.11.3", "2026-10-14", 3n))), F.Cases.id2())
    C.Check.str("reset rotates", T.TmState.id(F.Cases.run(F.Cases.st([F.Cases.ev("active", "0.11.3", True{})]), F.Cases.view(True{}, True{}, "1", "0.11.3", "2026-10-14", 3n))), F.Cases.id2())
    C.Check.str("3-day hold releases", Nat.show(List.length(&2, T.TmEv, T.Tm.ready(T.TmState.queue(F.Cases.run(held(), F.Cases.view(True{}, False{}, "0", "0.11.3", "2026-10-14", 259201n))), 259201n))), "1")
    C.Check.str("7-day prune", count(F.Cases.run(held(), F.Cases.view(True{}, False{}, "0", "0.11.3", "2026-10-14", 604802n))), "0")
    C.Check.str("queue cap", Nat.show(List.length(&2, T.TmEv, T.Tm.queue.limit(F.Cases.repeat(201n)))), "200")
    C.Check.str("planner queue cap", count(F.Cases.run(F.Cases.st(F.Cases.updates(201n)), F.Cases.view(True{}, True{}, "0", "0.11.3", "2026-10-14", 2n))), "200")
    C.Check.str("batch cap", Nat.show(List.length(&2, T.TmEv, T.Tm.batch(F.Cases.repeat(200n)))), "50")
    C.Check.str("400 drops", count(T.Tm.sent(F.Cases.st([F.Cases.ev("install", "0.11.3", True{})]), [F.Cases.ev("install", "0.11.3", True{})], 400n, 0n, 2n)), "0")
    C.Check.str("202 drops", count(T.Tm.sent(F.Cases.st([F.Cases.ev("install", "0.11.3", True{})]), [F.Cases.ev("install", "0.11.3", True{})], 202n, 0n, 2n)), "0")
    C.Check.str("500 backoff", Nat.show(T.TmState.next(T.Tm.sent(F.Cases.st([F.Cases.ev("install", "0.11.3", True{})]), [F.Cases.ev("install", "0.11.3", True{})], 500n, 0n, 2n))), "3602")
    C.Check.str("429 Retry-After", Nat.show(T.TmState.next(T.Tm.sent(F.Cases.st(Nil{}), Nil{}, 429n, 7200n, 2n))), "7202")
    C.Check.str("state roundtrip", T.Tm.state.json(T.Tm.state.parse(T.Tm.state.json(held()))), T.Tm.state.json(held()))
    C.Check.str("corrupt fresh", T.Tm.state.json(T.Tm.state.parse("{\"seen\":true}")), T.Tm.state.json(T.Tm.fresh()))
    check("sandbox closed", Bool.not(T.Tm.gate(T.TmEnv{True{}, "", "", "", "", "http://stub", "dev-1", "key"})))
    check("no key closed", Bool.not(T.Tm.gate(T.TmEnv{False{}, "", "", "", "", "http://stub", "dev-1", ""})))
    check("path scrubbed", Bool.not(String.contains(F.Cases.bad("/home/h/x"), "/home/h/x")))
    check("email scrubbed", Bool.not(String.contains(F.Cases.bad("a@b.c"), "a@b.c")))
    check("name scrubbed", Bool.not(String.contains(F.Cases.bad("private project"), "private project")))
    check("UUIDv4", T.Tm.uuid.ok(T.Tm.uuid("0123456789abcdef0123456789abcdef")))
    check("pow vector", T.Tm.pow.ok(F.Cases.id(), "186d7", 18n))
    C.Check.str("HMAC known vector", S.Sign.header("0000000000000000000000000000000000000000000000000000000000000000", 1n, "{}"), "sha256=a07ac7cfc802ea7629f0933ebf00ef447832d56dcdcab022d121f329249e4ac3")
    C.Check.str("body bytes", T.Tm.body(1791990000n, "9f2c4e1ab07d33e5", [T.TmEv{"install", "2026-10-14", False{}, "", "0.11.3", "", "release", "darwin", "arm64", "", 1n, False{}}]), "{\"v\":1,\"ts\":1791990000,\"nonce\":\"9f2c4e1ab07d33e5\",\"events\":[{\"e\":\"install\",\"day\":\"2026-10-14\",\"share\":false,\"ver\":\"0.11.3\",\"ch\":\"release\",\"os\":\"darwin\",\"arch\":\"arm64\"}]}")
    C.Check.str("share on asked", F.Cases.setting(U.Ui.act(U.Ui.init(1n), "share", "on"), "telemetry.asked"), "1")
    C.Check.str("share off", F.Cases.setting(U.Ui.act(U.Ui.init(1n), "share", "off"), "telemetry.share"), "off")
    C.Check.str("share reset", F.Cases.setting(U.Ui.act(U.Ui.init(1n), "share-reset", ""), "telemetry.reset"), "1")
    C.Check.str("existing asks share first", Nat.show(U.Tour.at(F.Cases.version(F.Cases.old()))), Nat.show(O.Ob.share()))
    C.Check.str("then uber", Nat.show(U.Tour.at(F.Cases.ui(U.Ui.act(F.Cases.version(F.Cases.old()), "share", "on")))), Nat.show(O.Ob.uber()))
    C.Check.str("privacy leaves step", Nat.show(U.Tour.at(F.Cases.ui(U.Ui.act(F.Cases.version(F.Cases.old()), "open-url", U.Share.privacy())))), Nat.show(O.Ob.share()))
    check("beta on existing prompt", String.contains(U.Tour.body(1n, F.Cases.version(F.Cases.old())), O.Ob.beta()))
    C.Check.str("Settings id text", U.Share.note.id(U.Share.id(U.Ui.info_set(U.Ui.init(1n), "telemetry.id", F.Cases.id()))), U.Share.note() ++ " Install id: " ++ F.Cases.id())
    C.Check.str("opt-out hides cached id", U.Share.id(F.Cases.ui(U.Ui.act(U.Ui.info_set(U.Ui.init(1n), "telemetry.id", F.Cases.id()), "share", "off"))), "")
    check("welcome beta", String.contains(O.Ob.body(I.En{}, 1n, "7", 0n), O.Ob.beta()))
    C.Check.str("Next answers standalone share", Nat.show(U.Tour.at(F.Cases.ui(U.Ui.act(F.Cases.version(F.Cases.old()), "ob-next", "")))), Nat.show(O.Ob.uber()))
    C.Check.str("Skip answers standalone share", Nat.show(U.Tour.at(F.Cases.ui(U.Ui.act(F.Cases.version(F.Cases.old()), "ob-skip", "")))), Nat.show(O.Ob.uber()))
    C.Check.str("issue link leaves standalone share", Nat.show(U.Tour.at(F.Cases.ui(U.Ui.act(F.Cases.version(F.Cases.old()), "open-url", U.Share.bug(F.Cases.version(F.Cases.old())))))), Nat.show(O.Ob.share()))
    C.Check.str("full share next marks asked", F.Cases.setting(U.Ui.act(U.Ui.scratch_set(F.Cases.version(U.Ui.scratch_set(U.Ui.adopt.raw(U.Ui.init(1n), M.State.replay([M.ProjectCreated{"p", "p", "/p", 1n}], M.State.empty())), "@sync", "1")), "@tour", "s9"), "ob-next", ""), "telemetry.asked"), "1")
    C.Check.str("bug exact URL", T.Bug.url("0.11.3", "linux", "x64"), "https://github.com/i2cjak/Backplane/issues/new?body=Version%3A%200.11.3%0AOS%3A%20linux%20x64%0A%0AWhat%20happened%3A%0A")
    check("bad os dropped", Bool.not(T.Tm.clean(T.TmEv{"active", "2026-10-14", True{}, F.Cases.id(), "0.11.3", "", "release", "/home/h/x", "x64", "", 1n, False{}})))
    check("bad prev dropped", Bool.not(T.Tm.clean(T.TmEv{"update", "2026-10-14", False{}, "", "0.11.3", "a@b.c", "release", "linux", "x64", "", 1n, False{}})))
    check("batch size bound", Nat.is_le(String.length(T.Tm.body(1791990000n, "0123456789abcdef", F.Cases.repeat(200n))), 16384n))
