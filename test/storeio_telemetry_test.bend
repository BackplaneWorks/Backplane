import Base
import ./check.bend as C
import ./telemetry_usage_cases.bend as F
import ../src/core/codec.bend as CD
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/snapshot.bend as SN
import ../src/core/store.bend as ST
import ../src/core/storedb.bend as DB
import ../src/core/telemetry.bend as T
import ../src/core/utf8.bend as U
import ../src/server/db.bend as D
import ../src/server/hotboot.bend as HB
import ../src/server/storeio.bend as STO

# Exercise the commit materializer itself. The pure Sdb.Mat test cannot catch
# a missing key in storeio.Plan.change, because it uses Sdb.step directly.
def TPlan.seed() -> STO.Plan:
  STO.Plan{M.State.empty(), Map.new(&2, Nat), 0n, 0n, 1n, Nil{}}

def TPlan.line(+c: M.Change, p: STO.Plan) -> STO.Plan:
  +line = J.Json.show(CD.Change.encode(c))
  STO.Plan.change(c, line, List.length(&2, U32, U.Utf8.encode(line)), p)

def TPlan.fold(xs: List<&2, M.Change>, p: STO.Plan) -> STO.Plan:
  match xs:
    case Nil{}:
      p
    case c <> tail:
      TPlan.fold(tail, TPlan.line(c, p))

def TPlan.lines(xs: List<&2, M.Change>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case c <> tail:
      J.Json.show(CD.Change.encode(c)) <> TPlan.lines(tail)

def TPlan.items(p: STO.Plan, lines: List<&2, String>) -> List<&2, List<&2, String>>:
  +logid = DB.Sdb.log_id(U.Utf8.encode(ST.Store.text(lines)))
  List.append(&2, List<&2, String>, STO.Plan.items(p), [
    ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", "schema", "1"],
    ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", "codec", Nat.show(DB.Sdb.codec())],
    ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", "log_id", logid]])

def TFile.done(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def TFile.write.k(r: Result<&1, &1, U32 & String, File>, +s: String) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write(f, s), TFile.done)
    case Fail{(_, e)}:
      IO.die(Unit, 1, "storeio_telemetry_test: log write failed: " ++ e)

def TFile.write(path: String, s: String) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path, "w"), r => TFile.write.k(r, s))

def TFile.append(path: String, s: String) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path, "a"), r => TFile.write.k(r, s))

def first(rs: List<&2, List<&2, String>>) -> String:
  match rs:
    case Nil{}:
      ""
    case row <> _:
      SN.Snap.col(row, 0n)

def firsts(rs: List<&2, List<&2, String>>) -> String:
  match rs:
    case Nil{}:
      ""
    case row <> Nil{}:
      SN.Snap.col(row, 0n)
    case row <> tail:
      SN.Snap.col(row, 0n) ++ "|" ++ firsts(tail)

def usage(rs: List<&2, List<&2, String>>) -> String:
  SN.Snap.str.j(Maybe.default(&2, J.Json, J.Json.parse(first(rs)), J.Null{}))

def board_events() -> List<&2, M.Change>:
  [M.SettingSet{"telemetry.pcb.board-a", "{\"created\":2,\"ordered\":3,\"at\":86400}", 86400n},
   M.SettingSet{"telemetry.pcb.board-b", "{\"created\":4,\"ordered\":5,\"at\":86400}", 86400n}]

def TDir.of(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> IO(String):
  match r:
    case Done{(_, (out, _))}:
      IO.pure(String, String.trim(out))
    case Fail{(_, e)}:
      IO.die(String, 1, "storeio_telemetry_test: mktemp failed: " ++ e)

def main() -> IO(Unit):
  do IO<Unit>:
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +home : String <- TDir.of(made)
    +opened1 : String <- D.Db.open(home ++ "/store.db")
    +h1 : String = String.drop(opened1, 3n)
    +schema : String <- D.Db.exec(h1, DB.Sdb.schema())
    C.Check.str("actual SQLite schema", String.take(schema, 2n), "ok")
    +initial : List<&2, M.Change> = List.append(&2, M.Change, F.UsageCases.log(), board_events())
    +initial_lines : List<&2, String> = TPlan.lines(initial)
    +p1 : STO.Plan = TPlan.fold(initial, TPlan.seed())
    +wrote1 : String <- D.Db.script(h1, TPlan.items(p1, initial_lines))
    C.Check.str("actual Plan initial SQL", String.take(wrote1, 2n), "ok")
    +u1 : List<&2, List<&2, String>> <- D.Db.query(h1, "SELECT json FROM obj WHERE domain='usage' AND key='1970-01-02'", [], 2)
    +boards1 : List<&2, List<&2, String>> <- D.Db.query(h1, "SELECT k FROM setting WHERE k LIKE 'telemetry.pcb.%' ORDER BY k", [], 4)
    +g1rows : List<&2, List<&2, String>> <- D.Db.query(h1, "SELECT v FROM setting WHERE k='telemetry.usage.generation'", [], 2)
    +g1 : String = first(g1rows)
    +cl1 : String <- D.Db.close(h1)
    C.Check.str("actual SQLite close after initial checkpoint", String.take(cl1, 2n), "ok")
    TFile.write(home ++ "/events.jsonl", ST.Store.text(initial_lines))
    +hot1 : HB.Hot <- HB.Hot.load(home)
    +hotst1 : M.State = HB.Hot.st(hot1)
    +cold1 : M.State = M.State.replay(initial, M.State.empty())
    C.Check.str("Plan.change/Plan.items persists all twelve counters", usage(u1), "1/1/1/2/1/100/20/30/40/1/6/8")
    C.Check.str("Plan.change/Plan.items preserves board setting keys", firsts(boards1), "telemetry.pcb.board-a|telemetry.pcb.board-b")
    C.Check.str("actual file-backed hotboot initial status", HB.Hot.why(hot1), "")
    C.Check.str("actual file-backed hotboot matches cold all-twelve counters", M.Metrics.get(hotst1, "1970-01-02"), M.Metrics.get(cold1, "1970-01-02"))
    C.Check.str("actual file-backed hotboot preserves consent generation", M.Setting.get(hotst1, "telemetry.usage.generation", ""), M.Setting.get(cold1, "telemetry.usage.generation", ""))
    C.Check.str("actual file-backed hotboot preserves telemetry reset identity", T.Tm.reset(hotst1), T.Tm.reset(cold1))
    +opened2 : String <- D.Db.open(home ++ "/store.db")
    +h2 : String = String.drop(opened2, 3n)
    +reset_events : List<&2, M.Change> = [M.SettingSet{"telemetry.reset", "1", 86400n}]
    +reset_lines : List<&2, String> = TPlan.lines(reset_events)
    +all2_lines : List<&2, String> = List.append(&2, String, initial_lines, reset_lines)
    +p2 : STO.Plan = TPlan.fold(reset_events, STO.Plan.next(p1))
    +wrote2 : String <- D.Db.script(h2, TPlan.items(p2, all2_lines))
    C.Check.str("actual Plan reset SQL", String.take(wrote2, 2n), "ok")
    +u2 : List<&2, List<&2, String>> <- D.Db.query(h2, "SELECT count(*) FROM obj WHERE domain='usage'", [], 2)
    +g2rows : List<&2, List<&2, String>> <- D.Db.query(h2, "SELECT v FROM setting WHERE k='telemetry.usage.generation'", [], 2)
    +g2 : String = first(g2rows)
    +cl2 : String <- D.Db.close(h2)
    C.Check.str("actual SQLite close after reset checkpoint", String.take(cl2, 2n), "ok")
    TFile.append(home ++ "/events.jsonl", ST.Store.text(reset_lines))
    +hot2 : HB.Hot <- HB.Hot.load(home)
    +hotst2 : M.State = HB.Hot.st(hot2)
    +cold2 : M.State = M.State.replay(List.append(&2, M.Change, initial, reset_events), M.State.empty())
    C.Check.str("Plan reset deletes durable usage rows", first(u2), "0")
    C.Check.str("Plan reset persists the new privacy generation", Bool.show(Bool.not(String.eq(g1, g2))), "True")
    C.Check.str("actual reset hotboot status", HB.Hot.why(hot2), "")
    C.Check.str("actual reset hotboot matches cold projection", M.Metrics.get(hotst2, "1970-01-02"), M.Metrics.get(cold2, "1970-01-02"))
    C.Check.str("actual reset hotboot preserves generation", M.Setting.get(hotst2, "telemetry.usage.generation", ""), M.Setting.get(cold2, "telemetry.usage.generation", ""))
    C.Check.str("actual reset hotboot preserves reset identity", T.Tm.reset(hotst2), T.Tm.reset(cold2))
    +opened3 : String <- D.Db.open(home ++ "/store.db")
    +h3 : String = String.drop(opened3, 3n)
    +off_events : List<&2, M.Change> = [M.SettingSet{"telemetry.share", "off", 86400n}]
    +off_lines : List<&2, String> = TPlan.lines(off_events)
    +all3_lines : List<&2, String> = List.append(&2, String, all2_lines, off_lines)
    +p3 : STO.Plan = TPlan.fold(off_events, STO.Plan.next(p2))
    +wrote3 : String <- D.Db.script(h3, TPlan.items(p3, all3_lines))
    C.Check.str("actual Plan opt-out SQL", String.take(wrote3, 2n), "ok")
    +u3 : List<&2, List<&2, String>> <- D.Db.query(h3, "SELECT count(*) FROM obj WHERE domain='usage'", [], 2)
    +boards3 : List<&2, List<&2, String>> <- D.Db.query(h3, "SELECT k FROM setting WHERE k LIKE 'telemetry.pcb.%' ORDER BY k", [], 4)
    C.Check.str("Plan opt-out leaves no usage rows", first(u3), "0")
    C.Check.str("Plan opt-out keeps the board source keys", firsts(boards3), "telemetry.pcb.board-a|telemetry.pcb.board-b")
    +cl3 : String <- D.Db.close(h3)
    C.Check.str("actual SQLite close after opt-out checkpoint", String.take(cl3, 2n), "ok")
    TFile.append(home ++ "/events.jsonl", ST.Store.text(off_lines))
    +opened4 : String <- D.Db.open(home ++ "/store.db")
    +h4 : String = String.drop(opened4, 3n)
    +on_events : List<&2, M.Change> = [M.SettingSet{"telemetry.share", "on", 86400n}, M.BotWoke{"after-reset", 0n, 86400n}]
    +on_lines : List<&2, String> = TPlan.lines(on_events)
    +all4_lines : List<&2, String> = List.append(&2, String, all3_lines, on_lines)
    +p4 : STO.Plan = TPlan.fold(on_events, STO.Plan.next(p3))
    +wrote4 : String <- D.Db.script(h4, TPlan.items(p4, all4_lines))
    C.Check.str("actual Plan opt-in SQL", String.take(wrote4, 2n), "ok")
    +u4 : List<&2, List<&2, String>> <- D.Db.query(h4, "SELECT json FROM obj WHERE domain='usage' AND key='1970-01-02'", [], 2)
    +cold4 : M.State = M.State.replay(List.append(&2, M.Change, List.append(&2, M.Change, List.append(&2, M.Change, initial, reset_events), off_events), on_events), M.State.empty())
    +cl4 : String <- D.Db.close(h4)
    C.Check.str("actual SQLite close after opt-in checkpoint", String.take(cl4, 2n), "ok")
    TFile.append(home ++ "/events.jsonl", ST.Store.text(on_lines))
    +hot4 : HB.Hot <- HB.Hot.load(home)
    +hotst4 : M.State = HB.Hot.st(hot4)
    C.Check.str("Plan next event after opt-in is durable", usage(u4), "0/0/0/0/1/0/0/0/0/0/0/0")
    C.Check.str("actual opt-in hotboot status", HB.Hot.why(hot4), "")
    C.Check.str("actual opt-in hotboot matches cold all-twelve counters", M.Metrics.get(hotst4, "1970-01-02"), M.Metrics.get(cold4, "1970-01-02"))
    C.Check.str("actual opt-in hotboot preserves generation", M.Setting.get(hotst4, "telemetry.usage.generation", ""), M.Setting.get(cold4, "telemetry.usage.generation", ""))
    C.Check.str("actual opt-in hotboot preserves reset identity", T.Tm.reset(hotst4), T.Tm.reset(cold4))
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", home], "", 4096, 10000)
    return Unit{}
