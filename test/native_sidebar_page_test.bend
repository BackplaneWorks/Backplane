import Base
import ./check.bend as C
import ../src/core/native-sidebar.bend as N
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/presentation.bend as P

def target() -> J.Json:
  J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{"a"}}])

def request(hub: String, gen: String) -> J.Json:
  J.Json.obj([J.KV{"nativeSidebar", J.Num{"1"}}, J.KV{"hub", J.Str{hub}}, J.KV{"generation", J.Str{gen}}, J.KV{"target", target()}])

def ui() -> U.Ui:
  N.Ns.prepare(U.Ui.adopt.raw(U.Ui.init(1n), M.State.apply(M.State.empty(), M.ProjectCreated{"p", "Project", "/tmp/p", 1n})), False{}, request("h", "1"), "epoch", "view", 7n, "00112233445566778899aabbccddeeff")

def ids(+n: Nat) -> List<&2, String>:
  match n:
    case 0n: Nil{}
    case 1n+k: Nat.show(n) <> ids(k)

def page(u: U.Ui) -> N.Slice:
  N.Ns.slice(u, ids(200n), "settled", "p", 16n, "2")

def next(u: U.Ui) -> String:
  "h|" ++ J.Json.get_str(J.Json.get_obj(N.Slice.pager(page(u)), "next"), "value", "")

type Walk is Data:
  Walk{ui: U.Ui, seen: List<&2, String>, done: Bool}

def walk(fuel: Nat, state: Walk) -> List<&2, String>:
  match fuel state:
    case 0n Walk{_, seen, _}: seen
    case _ Walk{_, seen, True{}}: seen
    case 1n+k Walk{+u, seen, False{}}:
      +part = page(u)
      +pager = N.Slice.pager(part)
      +offset = J.Json.get_nat(pager, "offset", 0n)
      +ordinary = List.take(&2, String, N.Slice.ids(part), Nat.min(16n, Nat.sub(200n, offset)))
      +token = J.Json.get_str(J.Json.get_obj(pager, "next"), "value", "")
      walk(k, Walk{N.Ns.accept(u, "h|" ++ token), List.append(&2, String, seen, ordinary), String.is_empty(token)})

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def token_payload() -> J.Json:
  J.Json.get_obj(N.Ns.parse(N.Ns.raw(ui(), next(ui()))), "payload")

def changed_token(key: String, value: J.Json) -> String:
  "h|" ++ J.Json.show(N.Obj.put(N.Ns.parse(N.Ns.raw(ui(), next(ui()))), "payload", N.Obj.put(token_payload(), key, value)))

def commit(+u: U.Ui, part: N.Slice) -> U.Ui:
  N.Ns.reconcile(u, J.Json.obj([J.KV{"nativeSidebar", J.Num{"1"}}, J.KV{"projects", J.Json.arr([J.Json.obj([J.KV{"settledPage", N.Slice.pager(part)}])])}]))

def retired() -> U.Ui:
  +u = N.Ns.accept(ui(), next(ui()))
  commit(u, N.Ns.slice(u, List.reverse(&2, String, ids(200n)), "settled", "p", 16n, "2"))

def vector() -> J.Json:
  N.Ns.parse("{\"version\":1,\"hub\":\"h\",\"generation\":\"1\",\"epoch\":\"epoch\",\"viewId\":\"view\",\"target\":{\"kind\":\"thread\",\"id\":\"a\"},\"metadataRevision\":7,\"listKind\":\"settled\",\"projectOwner\":\"p\",\"choicesDigest\":\"external-vector\",\"membershipDigest\":\"ordered-ids\",\"offset\":16,\"limit\":16}")

def long_request() -> J.Json:
  J.Json.obj([J.KV{"nativeSidebar", J.Num{"1"}}, J.KV{"hub", J.Str{String.repeat("h", 1024n)}}, J.KV{"generation", J.Str{"1"}},
    J.KV{"target", J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{String.repeat("t", 1024n)}}])}])

def long_ui() -> U.Ui:
  N.Ns.prepare(U.Ui.adopt.raw(U.Ui.init(1n), M.State.apply(M.State.empty(), M.ProjectCreated{String.repeat("\\", 1024n), "Long project", "/tmp/native-long", 1n})),
    False{}, long_request(), "epoch", "view", 7n, "00112233445566778899aabbccddeeff")

def long_control() -> String:
  J.Json.get_str(J.Json.get_obj(N.Slice.pager(N.Ns.slice(long_ui(), ids(200n), "settled", String.repeat("\\", 1024n), 16n, "")), "next"), "value", "")

def ambiguous_owner_payload() -> J.Json:
  N.Ns.expand.owner(N.Ns.owner.projects([M.Project{"p", "One", "/tmp/one", 1n, False{}}, M.Project{"p", "Two", "/tmp/two", 1n, False{}}], N.Ns.owner_digest("p")), ui(), token_payload())

def missing_owner_control() -> String:
  "h|" ++ N.Ns.token(ui(), N.Ns.payload(ui(), "settled", "missing-project", N.Ns.member(ids(200n)), 16n, 16n))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("native capability defaults off", yes(N.Ns.enabled(U.Ui.init(1n))), "no")
    C.Check.str("web cannot opt into native metadata", yes(N.Ns.enabled(N.Ns.prepare(ui(), True{}, request("h", "1"), "epoch", "view", 7n, "00112233445566778899aabbccddeeff"))), "no")
    C.Check.str("first page truthful total", Nat.show(J.Json.get_nat(N.Slice.pager(page(ui())), "total", 0n)), "200")
    C.Check.str("selected exception bounded outside page", Nat.show(List.length(&2, String, N.Slice.ids(page(ui())))), "17")
    C.Check.str("selected exception is real selected id", Maybe.default(&2, String, List.get(&2, String, N.Slice.ids(page(ui())), 16n), ""), "2")
    C.Check.str("first page has no previous control", J.Json.show(J.Json.get_obj(N.Slice.pager(page(ui())), "previous")), "null")
    C.Check.str("offered sealed next accepted", yes(N.Ns.valid(ui(), next(ui()))), "yes")
    C.Check.str("next page starts exactly after prior range", Nat.show(J.Json.get_nat(N.Slice.pager(page(N.Ns.accept(ui(), next(ui())))), "offset", 0n)), "16")
    C.Check.str("forged offset rejected", yes(N.Ns.valid(ui(), changed_token("offset", J.Num{"999"}))), "no")
    C.Check.str("forged capacity rejected", yes(N.Ns.valid(ui(), changed_token("limit", J.Num{"48"}))), "no")
    C.Check.str("forged source owner rejected", yes(N.Ns.valid(ui(), changed_token("ownerDigest", J.Str{"other"}))), "no")
    C.Check.str("other hub cannot borrow control", yes(N.Ns.valid(N.Ns.prepare(ui(), False{}, request("other", "1"), "epoch", "view", 7n, "00112233445566778899aabbccddeeff"), next(ui()))), "no")
    C.Check.str("other generation cannot borrow control", yes(N.Ns.valid(N.Ns.prepare(ui(), False{}, request("h", "2"), "epoch", "view", 7n, "00112233445566778899aabbccddeeff"), next(ui()))), "no")
    C.Check.str("other view cannot borrow control", yes(N.Ns.valid(N.Ns.prepare(ui(), False{}, request("h", "1"), "epoch", "other", 7n, "00112233445566778899aabbccddeeff"), next(ui()))), "no")
    C.Check.str("stale source resets rather than adopts offset", Nat.show(J.Json.get_nat(N.Slice.pager(page(N.Ns.accept(N.Ns.prepare(ui(), False{}, request("h", "1"), "epoch", "view", 8n, "00112233445566778899aabbccddeeff"), next(ui())))), "offset", 99n)), "0")
    C.Check.str("private key never enters exported session", yes(String.contains(J.Json.show(P.Session.pack(ui())), "00112233445566778899aabbccddeeff")), "no")
    C.Check.str("private page positions never enter exported session", yes(String.contains(J.Json.show(P.Session.pack(N.Ns.accept(ui(), next(ui())))), "local.native.sidebar.pages")), "no")
    C.Check.str("all200 source identities traversed once in exact order", J.Json.show(J.Json.arr(N.Ns.ids.json(walk(20n, Walk{ui(), Nil{}, False{}})))), J.Json.show(J.Json.arr(N.Ns.ids.json(ids(200n)))))
    C.Check.str("membership change resets old offset", Nat.show(J.Json.get_nat(N.Slice.pager(N.Ns.slice(N.Ns.accept(ui(), next(ui())), List.reverse(&2, String, ids(200n)), "settled", "p", 16n, "2")), "offset", 99n)), "0")
    C.Check.str("natural ticking preserves page offset", Nat.show(J.Json.get_nat(N.Slice.pager(page(N.Ns.prepare(U.Ui.tick(N.Ns.accept(ui(), next(ui())), 2n), False{}, request("h", "1"), "epoch", "view", 7n, "00112233445566778899aabbccddeeff"))), "offset", 99n)), "16")
    C.Check.str("new private nonce rejects old control", yes(N.Ns.valid(N.Ns.prepare(ui(), False{}, request("h", "1"), "epoch", "view", 7n, "ffeeddccbbaa99887766554433221100"), next(ui()))), "no")
    C.Check.str("empty lists have no synthetic pager", J.Json.show(N.Slice.pager(N.Ns.slice(ui(), Nil{}, "settled", "p", 16n, ""))), "null")
    C.Check.str("membership reversal permanently retires old offset", Nat.show(J.Json.get_nat(N.Slice.pager(page(retired())), "offset", 99n)), "0")
    C.Check.str("empty projection retires hidden positions", U.Ui.info(N.Ns.reconcile(N.Ns.accept(ui(), next(ui())), J.Json.obj([J.KV{"nativeSidebar", J.Num{"1"}}, J.KV{"projects", J.Json.arr(Nil{})}])), "local.native.sidebar.pages"), "{}")
    C.Check.str("nonhex32 secret cannot negotiate", yes(N.Ns.enabled(N.Ns.prepare(ui(), False{}, request("h", "1"), "epoch", "view", 7n, "zz112233445566778899aabbccddeeff00"))), "no")
    C.Check.str("nonce participates in private memo ownership", yes(String.eq(N.Ns.memo_key(ui()), N.Ns.memo_key(N.Ns.prepare(ui(), False{}, request("h", "1"), "epoch", "view", 7n, "ffeeddccbbaa99887766554433221100")))), "no")
    C.Check.str("domain-separated external crypto vector", N.Ns.mac("00112233445566778899aabbccddeeff", vector()), "31b6ca450712a692a077c3b9f85e0a6387a4d18195c3ef6c1fea86dedaf36a72")
    C.Check.str("allowed long hub target escaped owner token stays bounded", yes(Nat.is_le(J.Json.bytes(J.Str{long_control()}), 2048n)), "yes")
    C.Check.str("allowed long context producer token is accepted", yes(N.Ns.valid(long_ui(), String.repeat("h", 1024n) ++ "|" ++ long_control())), "yes")
    C.Check.str("unknown full owner cannot be substituted", yes(N.Ns.valid(ui(), changed_token("ownerDigest", J.Str{String.repeat("a", 64n)}))), "no")
    C.Check.str("ambiguous canonical owner scan rejects reconstruction", J.Json.show(ambiguous_owner_payload()), "null")
    C.Check.str("otherwise valid signed missing owner is rejected", yes(N.Ns.valid(ui(), missing_owner_control())), "no")
