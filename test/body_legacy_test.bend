import Base
import ./check.bend as C
import ./lr_whole_test.bend as L
import ../src/server/thin-body.bend as B
import ../src/server/thin-view.bend as V
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/thin.bend as T

def request(cursor: String) -> J.Json:
  J.Json.obj([J.KV{"target", L.target()}, J.KV{"entryId", J.Str{L.id()}}, J.KV{"bodyVersion", J.Str{"1"}}, J.KV{"cursor", J.Str{cursor}}])

def read.value(read: B.Read) -> J.Json:
  match read:
    case B.Read{_, value}: value

def main() -> IO(Unit):
  do IO<Unit>:
    +st : M.State = M.State.apply(L.st(), M.MessagePosted{"t0", L.id(), M.User{}, L.legacy(), 9n})
    +index : T.Index = V.State.body.index(True{}, st, L.index(), "t0", ["1"])
    +row : T.Row = T.Index.row(index, 1n)
    C.Check.str("legacy report is repaired before bounded metadata", T.Entry.text(T.Row.entry(row)), L.head() ++ L.reply())
    +part : J.Json = B.Body.part(row, "epoch", L.target())
    +result : J.Json = read.value(B.Body.row.read(row, "t0", request(J.Json.get_str(part, "cursor", "")), "epoch"))
    C.Check.str("legacy report retains its complete accessible source", J.Json.get_str(result, "text", ""), L.head() ++ L.reply())
    C.Check.str("legacy whole source completes through normal bounded reader", Bool.show(J.Json.get_bool(result, "complete", False{})), "True")
    C.Check.str("legacy source cannot be read under another conversation", J.Json.show(read.value(B.Body.row.read(row, "foreign", request(J.Json.get_str(part, "cursor", "")), "epoch"))), "null")
    +page : List<&2, String> = T.Index.page(L.index(), "t0", 0n, 32n)
    C.Check.str("pressure preserves a sole reading anchor", Nat.show(List.length(&2, String, B.Body.window(page, L.index(), L.id()))), "1")
