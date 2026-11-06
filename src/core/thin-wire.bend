import Base
import ./json.bend as J
import ./presentation.bend as P
import ./thin.bend as T

# Wire-only bounds and identity fences. This data never grants a command,
# installs a read model or advances the client's canonical replay cursor.
def Wire.limits() -> J.Json:
  J.Json.obj([J.KV{"replyBytes", J.Num{"262144"}}, J.KV{"entries", J.Num{"32"}}, J.KV{"recent", J.Num{"24"}}, J.KV{"searchResults", J.Num{"25"}}, J.KV{"searchCandidates", J.Num{"128"}}, J.KV{"chunkBytes", J.Num{"65536"}}])

def Wire.capabilities() -> J.Json:
  J.Json.obj([J.KV{"version", J.Num{"1"}}, J.KV{"views", J.Json.obj([J.KV{"version", J.Num{"1"}}, J.KV{"websocketQuery", J.Str{"view=1"}}, J.KV{"renderers", J.Json.arr([J.Str{"web"}, J.Str{"mobile"}])}, J.KV{"compatibility", J.Str{"explicit-only"}}])}])

# What a web page may leave to this hub: "fabSelf", the hub asks a shown
# thread's ordering card in its own render (thin-actor.bend's mail_ready),
# so the page need not send its "fab-look" action after each open (an old
# hub, without the field, still needs it). Phones get none of it.
def Wire.greeting.extra(+renderer: String) -> List<&2, J.KV>:
  match renderer:
    case "web": [J.KV{"fabSelf", J.Flag{True{}}}]
    case _: Nil{}

def Wire.greeting(+renderer: String, generation: String, epoch: String) -> J.Json:
  J.Json.obj(List.append(&2, J.KV, [J.KV{"t", J.Str{"view"}}, J.KV{"version", J.Num{"1"}}, J.KV{"renderer", J.Str{renderer}}, J.KV{"generation", J.Str{generation}}, J.KV{"epoch", J.Str{epoch}}, J.KV{"limits", Wire.limits()}], Wire.greeting.extra(renderer)))

def Wire.within.go(s: String, +left: Nat, valid: Bool) -> Bool:
  match s valid:
    case _ False{}: False{}
    case SNil{} True{}: True{}
    case SCon{Chr{+c}, tail} True{}: Wire.within.go(tail, Nat.sub(left, T.Bytes.char(c)), Nat.is_le(T.Bytes.char(c), left))

def Wire.within(text: String, limit: Nat) -> Bool:
  Wire.within.go(text, limit, True{})

def Wire.nat.j(j: J.Json, max: Nat) -> Maybe<&2, Nat>:
  match j:
    case J.Num{raw}: P.Session.nat(raw, max)
    case _: None{}

def Wire.nat(+p: J.Json, key: String, max: Nat) -> Maybe<&2, Nat>:
  Wire.nat.j(Maybe.default(&2, J.Json, J.Json.get(p, key), J.Null{}), max)

def Wire.positive(n: Maybe<&2, Nat>) -> Bool:
  match n:
    case Some{0n}: False{}
    case Some{_}: True{}
    case None{}: False{}

def Wire.version(j: J.Json) -> Bool:
  match j:
    case J.Num{"1"}: True{}
    case _: False{}

def Target.kind(+kind: String, +id: String) -> Bool:
  Bool.and(Wire.within(id, 1024n), Bool.or(Bool.and(String.eq(kind, "recent"), String.is_empty(id)),
    Bool.and(Bool.not(String.is_empty(id)), Bool.or(String.eq(kind, "project"), Bool.or(String.eq(kind, "thread"), Bool.or(String.eq(kind, "bot"), String.eq(kind, "room")))))))

def Target.fields(fs: J.Json, +count: Nat) -> Bool:
  match fs:
    case J.End{}: Nat.is_eq(count, 2n)
    case J.Field{+k, J.Str{_}, tail}: Bool.and(Bool.or(String.eq(k, "kind"), String.eq(k, "id")), Target.fields(tail, Nat.add(count, 1n)))
    case _: False{}

def Target.valid(+j: J.Json) -> Bool:
  match j:
    case J.Obj{+fs}: Bool.and(Target.fields(fs, 0n), Target.kind(J.Json.get_str(j, "kind", ""), J.Json.get_str(j, "id", "")))
    case _: False{}

def Target.same(+a: J.Json, +b: J.Json) -> Bool:
  Bool.and(Target.valid(a), Bool.and(Target.valid(b), Bool.and(String.eq(J.Json.get_str(a, "kind", ""), J.Json.get_str(b, "kind", "")), String.eq(J.Json.get_str(a, "id", ""), J.Json.get_str(b, "id", "")))))

def Wire.request(+p: J.Json, generation: String, epoch: String) -> Bool:
  Bool.and(Wire.version(Maybe.default(&2, J.Json, J.Json.get(p, "version"), J.Null{})),
    Bool.and(String.eq(J.Json.get_str(p, "generation", ""), generation),
    Bool.and(String.eq(J.Json.get_str(p, "epoch", ""), epoch),
    Bool.and(Bool.not(String.is_empty(J.Json.get_str(p, "hub", ""))),
    Bool.and(Wire.within(J.Json.get_str(p, "hub", ""), 1024n),
    Bool.and(Wire.positive(Wire.nat(p, "requestRevision", 4294967295n)), Target.valid(J.Json.get_obj(p, "target"))))))))

def Wire.control.id(id: Maybe<&2, Nat>) -> Bool:
  match id:
    case Some{n}: Nat.is_gt(n, Nat.mul(65536n, 2147483648n))
    case None{}: False{}

def Wire.ordinary.rid(n: Maybe<&2, Nat>) -> Bool:
  match n:
    case Some{n}: Nat.is_lt(n, Nat.mul(65536n, 2147483648n))
    case None{}: False{}

def Cursor.write(epoch: String, target: J.Json, query: String, before: Nat) -> String:
  J.Json.show(J.Json.obj([J.KV{"version", J.Num{"1"}}, J.KV{"epoch", J.Str{epoch}}, J.KV{"target", target}, J.KV{"query", J.Str{query}}, J.KV{"before", J.Str{Nat.show(before)}}]))

def Cursor.parsed(valid: Bool, j: J.Json) -> Maybe<&2, Nat>:
  match valid:
    case False{}: None{}
    case True{}: P.Session.nat(J.Json.get_str(j, "before", ""), P.Session.nat.max())

def Cursor.fields(+j: J.Json, epoch: String, target: J.Json, query: String) -> Maybe<&2, Nat>:
  Cursor.parsed(Bool.and(Wire.version(Maybe.default(&2, J.Json, J.Json.get(j, "version"), J.Null{})), Bool.and(String.eq(J.Json.get_str(j, "epoch", ""), epoch), Bool.and(Target.same(J.Json.get_obj(j, "target"), target), String.eq(J.Json.get_str(j, "query", ""), query)))), j)

def Cursor.short(short: Bool, text: String, epoch: String, target: J.Json, query: String) -> Maybe<&2, Nat>:
  match short:
    case False{}: None{}
    case True{}: Cursor.fields(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), epoch, target, query)

def Cursor.read(+text: String, epoch: String, target: J.Json, query: String) -> Maybe<&2, Nat>:
  Cursor.short(Wire.within(text, 1024n), text, epoch, target, query)

def Wire.error(code: String) -> J.Json:
  J.Json.obj([J.KV{"code", J.Str{code}}, J.KV{"retryable", J.Flag{False{}}}, J.KV{"compatibilityRequired", J.Flag{False{}}}])

# A notification contains only routing identity and an opaque projection
# revision. It is not canonical log credit or a body/history transport.
def Wire.changed(generation: String, epoch: String, revision: Nat, view: String, target: J.Json) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"view.changed"}}, J.KV{"version", J.Num{"1"}},
    J.KV{"generation", J.Str{generation}}, J.KV{"epoch", J.Str{epoch}},
    J.KV{"viewId", J.Str{view}}, J.KV{"target", target}, J.KV{"viewRevision", J.Str{Nat.show(revision)}}])
