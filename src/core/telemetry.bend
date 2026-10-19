import Base
import ./json.bend as J
import ./model.bend as M
import ./semver.bend as SV
import ./images.bend as IM
import ./utf8.bend as U8
import ./sign.bend as SG
import ./sha256.bend as SH

# Wire v1 is deliberately an allowlist. Local bookkeeping never enters body.
type TmEv is Data:
  TmEv{e: String, day: String, share: Bool, id: String, ver: String, prev: String, ch: String, os: String, arch: String, pow: String, made: Nat, held: Bool}

type TmState is Data:
  TmState{id: String, ver: String, today: String, share: Bool, reset: String, queue: List<&2, TmEv>, next: Nat, busy: Nat, seen: Bool}

type TmView is Data:
  TmView{share: Bool, asked: Bool, reset: String, ver: String, ch: String, os: String, arch: String, day: String, now: Nat}

type TmPlan is Data:
  TmPlan{state: TmState, need_pow: Bool}

type TmEnv is Data:
  TmEnv{sandbox: Bool, sandbox_env: String, telemetry: String, dnt: String, ci: String, url: String, kid: String, key: String}

def Tm.share(st: M.State) -> Bool:
  Bool.not(String.eq(M.Setting.get(st, "telemetry.share", "on"), "off"))

def Tm.asked(st: M.State) -> Bool:
  String.eq(M.Setting.get(st, "telemetry.asked", ""), "1")

def Tm.reset(st: M.State) -> String:
  M.Setting.get(st, "telemetry.reset", "0")

# A worker must ask the hub again after PoW and before starting HTTP. A privacy
# choice processed while that worker ran invalidates its snapshot.
def Tm.current(v: TmView, +st: M.State) -> Bool:
  match v:
    case TmView{share, asked, reset, _, _, _, _, _, _}:
      Bool.and(Bool.pick(Bool, share, Tm.share(st), Bool.not(Tm.share(st))), Bool.and(Bool.pick(Bool, asked, Tm.asked(st), Bool.not(Tm.asked(st))), String.eq(reset, Tm.reset(st))))

def Tm.fresh() -> TmState:
  TmState{"", "", "", True{}, "0", Nil{}, 0n, 0n, False{}}

def Tm.has(xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> t: Bool.or(String.eq(h, s), Tm.has(t, s))

def Tm.num.go(s: String, +n: Nat) -> Nat:
  match s:
    case SNil{}: n
    case SCon{Chr{+c}, t}: Tm.num.go(t, Nat.add(Nat.mul(n, 10n), U32.to_nat((c - 48 : U32))))

def Tm.digits(s: String) -> Bool:
  match s:
    case SNil{}: True{}
    case SCon{Chr{+c}, t}: Bool.and(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Tm.digits(t))

def Tm.number(+s: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(s)), Tm.digits(s))

def Tm.num(+s: String) -> Nat:
  Bool.pick(Nat, Tm.number(s), Tm.num.go(s, 0n), 0n)

def Tm.hex(s: String) -> Bool:
  match s:
    case SNil{}: True{}
    case SCon{Chr{+c}, t}: Bool.and(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 102))), Tm.hex(t))

def Tm.hex.n(+s: String, n: Nat) -> Bool:
  Bool.and(Nat.is_eq(String.length(s), n), Tm.hex(s))

def Tm.part(+s: String) -> Bool:
  Bool.and(Tm.number(s), Nat.is_le(String.length(s), 4n))

def Tm.ver.base(xs: List<&2, String>) -> Bool:
  match xs:
    case a <> b <> c <> Nil{}: Bool.and(Tm.part(a), Bool.and(Tm.part(b), Tm.part(c)))
    case _: False{}

def Tm.suffix(s: String) -> Bool:
  match s:
    case SNil{}: True{}
    case SCon{Chr{+c}, t}: Bool.and(Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))), Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), U32.is_eq(c, 46))), Tm.suffix(t))

def Tm.ver.parts(xs: List<&2, String>) -> Bool:
  match xs:
    case a <> Nil{}: Tm.ver.base(String.split(a, '.'))
    case a <> +b <> Nil{}: Bool.and(Tm.ver.base(String.split(a, '.')), Bool.and(Bool.not(String.is_empty(b)), Bool.and(Nat.is_le(String.length(b), 32n), Tm.suffix(b))))
    case _: False{}

def Tm.version(+s: String) -> Bool:
  Bool.and(Nat.is_le(String.length(s), 47n), Tm.ver.parts(String.split(s, '-')))

def Tm.day.parts(xs: List<&2, String>) -> Bool:
  match xs:
    case +y <> +m <> +d <> Nil{}:
      Bool.and(Tm.hex.n(y, 4n), Bool.and(Tm.digits(y), Bool.and(Nat.is_eq(String.length(m), 2n), Bool.and(Tm.digits(m), Bool.and(Nat.is_eq(String.length(d), 2n), Bool.and(Tm.digits(d), Bool.and(Nat.is_ge(Tm.num(m), 1n), Bool.and(Nat.is_le(Tm.num(m), 12n), Bool.and(Nat.is_ge(Tm.num(d), 1n), Nat.is_le(Tm.num(d), 31n))))))))))
    case _: False{}

def Tm.day(+s: String) -> Bool:
  Bool.and(Nat.is_eq(String.length(s), 10n), Tm.day.parts(String.split(s, '-')))

def Tm.uuid.ok.parts(xs: List<&2, String>) -> Bool:
  match xs:
    case +a <> +b <> +c <> +d <> +e <> Nil{}:
      Bool.and(Tm.hex.n(a, 8n), Bool.and(Tm.hex.n(b, 4n), Bool.and(Tm.hex.n(c, 4n), Bool.and(String.starts_with(c, "4"), Bool.and(Tm.hex.n(d, 4n), Bool.and(Tm.has(["8", "9", "a", "b"], String.take(d, 1n)), Tm.hex.n(e, 12n)))))))
    case _: False{}

def Tm.uuid.ok(+s: String) -> Bool:
  Bool.and(Nat.is_eq(String.length(s), 36n), Tm.uuid.ok.parts(String.split(s, '-')))

def Tm.channel(+ver: String, +uc: String) -> String:
  Bool.pick(String, Bool.or(String.eq(uc, "dev"), String.contains(ver, "-dev.")), "dev", Bool.pick(String, String.eq(uc, "nightly"), "nightly", "release"))

def Tm.plat(+s: String) -> String & String:
  (Bool.pick(String, String.starts_with(s, "darwin"), "darwin", Bool.pick(String, String.starts_with(s, "windows"), "windows", "linux")), Bool.pick(String, String.contains(s, "arm64"), "arm64", "x64"))

def Tm.gate(env: TmEnv) -> Bool:
  match env:
    case TmEnv{True{}, _, _, _, _, _, _, _}: False{}
    case TmEnv{False{}, +sb, +tm, dnt, ci, url, kid, key}:
      Bool.and(Bool.or(String.is_empty(sb), String.eq(sb, "0")), Bool.and(Bool.not(Bool.or(String.eq(tm, "0"), String.eq(tm, "off"))), Bool.and(Bool.not(String.eq(dnt, "1")), Bool.and(String.is_empty(ci), Bool.and(Bool.not(String.is_empty(url)), Bool.and(Bool.not(String.is_empty(kid)), Bool.not(String.is_empty(key))))))))

# Runtime overrides are explicit local integration tests only. Normal developer
# builds have empty compile defaults even if telemetry env variables are present.
def Tm.loopback.parts(xs: List<&2, String>) -> Bool:
  match xs:
    case +port <> "api" <> "v1" <> "events" <> Nil{}:
      Bool.and(Tm.number(port), Bool.and(Nat.is_gt(Tm.num(port), 0n), Nat.is_le(Tm.num(port), 65535n)))
    case _: False{}

def Tm.loopback(+url: String) -> Bool:
  Bool.and(String.starts_with(url, "http://127.0.0.1:"), Tm.loopback.parts(String.split(String.drop(url, 17n), '/')))

def Tm.config.ok(test: String, +url: String, +kid: String, +key: String, +channel: String) -> Bool:
  Bool.and(Tm.hex.n(key, 64n), Bool.and(Bool.or(String.starts_with(kid, "dev-"), String.starts_with(kid, "rel-")),
    Bool.pick(Bool, String.eq(test, "1"), Tm.loopback(url), Bool.and(String.eq(url, "https://backplane.works/api/v1/events"), Bool.and(String.starts_with(kid, "rel-"), Tm.has(["release", "nightly"], channel))))))

def Tm.clean(+ev: TmEv) -> Bool:
  match ev:
    case TmEv{+e, day, +share, +id, ver, +prev, ch, os, arch, +pow, _, held}:
      +fields = Bool.and(Tm.has(["install", "update", "active"], e), Bool.and(Tm.day(day), Bool.and(Tm.version(ver), Bool.and(Tm.has(["release", "nightly", "dev"], ch), Bool.and(Tm.has(["linux", "darwin", "windows"], os), Tm.has(["x64", "arm64"], arch))))))
      +ident = Bool.pick(Bool, share, Tm.uuid.ok(id), Bool.and(String.is_empty(id), Bool.not(String.eq(e, "active"))))
      +previous = Bool.or(String.is_empty(prev), Bool.and(String.eq(e, "update"), Tm.version(prev)))
      +proof = Bool.or(String.is_empty(pow), Bool.and(Bool.and(share, String.eq(e, "install")), Bool.and(Tm.hex(pow), Nat.is_le(String.length(pow), 16n))))
      Bool.and(Bool.not(held), Bool.and(fields, Bool.and(ident, Bool.and(previous, proof))))

def Tm.ev.json(ev: TmEv) -> J.Json:
  match ev:
    case TmEv{+e, day, +share, id, ver, +prev, ch, os, arch, +pow, _, _}:
      J.Json.obj(List.concat(&2, J.KV, [[J.KV{"e", J.Str{e}}, J.KV{"day", J.Str{day}}], Bool.pick(List<&2, J.KV>, share, [J.KV{"id", J.Str{id}}], Nil{}), [J.KV{"share", J.Flag{share}}, J.KV{"ver", J.Str{ver}}], Bool.pick(List<&2, J.KV>, Bool.and(String.eq(e, "update"), Bool.not(String.is_empty(prev))), [J.KV{"prev", J.Str{prev}}], Nil{}), [J.KV{"ch", J.Str{ch}}, J.KV{"os", J.Str{os}}, J.KV{"arch", J.Str{arch}}], Bool.pick(List<&2, J.KV>, Bool.and(Bool.and(share, String.eq(e, "install")), Bool.not(String.is_empty(pow))), [J.KV{"pow", J.Str{pow}}], Nil{})]))

def Tm.body.evs.k(ok: Bool, ev: TmEv, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match ok:
    case True{}: Tm.ev.json(ev) <> rest
    case False{}: rest

def Tm.body.evs(xs: List<&2, TmEv>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case +h <> t: Tm.body.evs.k(Tm.clean(h), h, Tm.body.evs(t))

def Tm.body.raw(ts: Nat, nonce: String, evs: List<&2, TmEv>) -> String:
  J.Json.show(J.Json.obj([J.KV{"v", J.Num{"1"}}, J.KV{"ts", J.Json.nat(ts)}, J.KV{"nonce", J.Str{nonce}}, J.KV{"events", J.Json.arr(Tm.body.evs(evs))}]))

def Tm.headers(kid: String, +key: String, +ts: Nat, +body: String) -> List<&2, String>:
  ["X-Backplane-Key: " ++ kid, "X-Backplane-Timestamp: " ++ Nat.show(ts), "X-Backplane-Signature: " ++ SG.Sign.header(key, ts, body)]

def Tm.pow.zeros(bs: List<&2, U32>, bits: Nat) -> Bool:
  match bs bits:
    case _ 0n: True{}
    case b <> t +n:
      Bool.and(U32.is_eq(U32.shrn(b, Nat.sub(8n, Bool.pick(Nat, Nat.is_ge(n, 8n), 8n, n))), 0), Tm.pow.zeros(t, Nat.sub(n, 8n)))
    case Nil{} _: False{}

def Tm.pow.ok(id: String, pow: String, bits: Nat) -> Bool:
  Tm.pow.zeros(SH.Sha256.hash(SG.Sign.utf8("bp-pow1:" ++ id ++ ":" ++ pow)), bits)

def Tm.ev.new(e: String, prev: String, view: TmView, id: String) -> TmEv:
  match view:
    case TmView{+share, +asked, _, ver, ch, os, arch, day, now}:
      TmEv{e, day, Bool.and(share, asked), Bool.pick(String, Bool.and(share, asked), id, ""), ver, prev, ch, os, arch, "", now, Bool.not(asked)}

def Tm.ev.off(ev: TmEv) -> TmEv:
  match ev:
    case TmEv{e, day, _, _, ver, prev, ch, os, arch, _, made, _}: TmEv{e, day, False{}, "", ver, prev, ch, os, arch, "", made, False{}}

def Tm.ev.release(ev: TmEv, share: Bool, id: String) -> TmEv:
  match ev share:
    case TmEv{e, day, _, _, ver, prev, ch, os, arch, pow, made, _} True{}: TmEv{e, day, True{}, id, ver, prev, ch, os, arch, pow, made, False{}}
    case x False{}: Tm.ev.off(x)

def TmEv.e(ev: TmEv) -> String:
  match ev:
    case TmEv{e, _, _, _, _, _, _, _, _, _, _, _}: e

def TmEv.id(ev: TmEv) -> String:
  match ev:
    case TmEv{_, _, _, id, _, _, _, _, _, _, _, _}: id

def TmEv.made(ev: TmEv) -> Nat:
  match ev:
    case TmEv{_, _, _, _, _, _, _, _, _, _, made, _}: made

def Tm.queue.cons(m: Maybe<&2, TmEv>, rest: List<&2, TmEv>) -> List<&2, TmEv>:
  match m:
    case Some{x}: x <> rest
    case None{}: rest

def Tm.queue.dropids(xs: List<&2, TmEv>) -> List<&2, TmEv>:
  match xs:
    case Nil{}: Nil{}
    case +h <> t: Tm.queue.cons(Bool.pick(Maybe<&2, TmEv>, String.is_empty(TmEv.id(h)), Some{h}, None{}), Tm.queue.dropids(t))

def Tm.key(ev: TmEv) -> String:
  match ev:
    case TmEv{+e, +day, share, _, ver, _, _, _, _, _, _, _}: e ++ ":" ++ Bool.pick(String, String.eq(e, "active"), "", ver) ++ ":" ++ Bool.pick(String, share, "on", "off") ++ Bool.pick(String, String.eq(e, "active"), day, "")

def Tm.queue.dedupe(xs: List<&2, TmEv>, +seen: List<&2, String>) -> List<&2, TmEv>:
  match xs:
    case Nil{}: Nil{}
    case +h <> t: Tm.queue.cons(Bool.pick(Maybe<&2, TmEv>, Tm.has(seen, Tm.key(h)), None{}, Some{h}), Tm.queue.dedupe(t, Tm.key(h) <> seen))

def Tm.queue.active(xs: List<&2, TmEv>) -> Bool:
  match xs:
    case h <> _: String.eq(TmEv.e(h), "active")
    case Nil{}: False{}

def Tm.queue.trim(xs: List<&2, TmEv>, active: Bool, +n: Nat, +acc: List<&2, TmEv>) -> List<&2, TmEv>:
  match xs active:
    case Nil{} _: List.reverse(&2, TmEv, acc)
    case +h <> +t True{}:
      Tm.queue.trim(t, Tm.queue.active(t), Nat.sub(n, 1n), Bool.pick(List<&2, TmEv>, Nat.is_gt(n, 0n), acc, h <> acc))
    case h <> +t False{}: Tm.queue.trim(t, Tm.queue.active(t), n, h <> acc)

def Tm.queue.limit(+xs: List<&2, TmEv>) -> List<&2, TmEv>:
  List.reverse(&2, TmEv, List.take(&2, TmEv, List.reverse(&2, TmEv, Tm.queue.trim(xs, Tm.queue.active(xs), Nat.sub(List.length(&2, TmEv, xs), 200n), Nil{})), 200n))

def Tm.queue.age(xs: List<&2, TmEv>, +now: Nat) -> List<&2, TmEv>:
  match xs:
    case Nil{}: Nil{}
    case +h <> t: Tm.queue.cons(Bool.pick(Maybe<&2, TmEv>, Nat.is_le(Nat.sub(now, TmEv.made(h)), 604800n), Some{h}, None{}), Tm.queue.age(t, now))

def TmView.share(v: TmView) -> Bool:
  match v:
    case TmView{s, _, _, _, _, _, _, _, _}: s

def TmView.asked(v: TmView) -> Bool:
  match v:
    case TmView{_, a, _, _, _, _, _, _, _}: a

def TmView.ch(v: TmView) -> String:
  match v:
    case TmView{_, _, _, _, x, _, _, _, _}: x

def TmView.os(v: TmView) -> String:
  match v:
    case TmView{_, _, _, _, _, x, _, _, _}: x

def TmView.arch(v: TmView) -> String:
  match v:
    case TmView{_, _, _, _, _, _, x, _, _}: x

def TmState.queue(st: TmState) -> List<&2, TmEv>:
  match st:
    case TmState{_, _, _, _, _, q, _, _, _}: q

def TmState.next(st: TmState) -> Nat:
  match st:
    case TmState{_, _, _, _, _, _, n, _, _}: n

def Tm.needpow(xs: List<&2, TmEv>) -> Bool:
  match xs:
    case Nil{}: False{}
    case TmEv{+e, _, share, _, _, _, _, _, _, pow, _, held} <> t: Bool.or(Bool.and(Bool.and(String.eq(e, "install"), share), Bool.and(String.is_empty(pow), Bool.not(held))), Tm.needpow(t))

def Tm.ready.one(+ev: TmEv, +now: Nat) -> Maybe<&2, TmEv>:
  match ev:
    case TmEv{+e, _, +share, _, _, _, _, _, _, +pow, +made, +held}:
      Bool.pick(Maybe<&2, TmEv>, Bool.and(Nat.is_le(Nat.sub(now, made), 604800n), Bool.and(Bool.not(held), Bool.or(Bool.not(Bool.and(String.eq(e, "install"), share)), Bool.not(String.is_empty(pow))))), Some{ev}, None{})

def Tm.ready(xs: List<&2, TmEv>, +now: Nat) -> List<&2, TmEv>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: Tm.queue.cons(Tm.ready.one(h, now), Tm.ready(t, now))

def Tm.ev.local(ev: TmEv) -> J.Json:
  match ev:
    case TmEv{e, day, share, id, ver, prev, ch, os, arch, pow, made, held}:
      J.Json.obj([J.KV{"e", J.Str{e}}, J.KV{"day", J.Str{day}}, J.KV{"share", J.Flag{share}}, J.KV{"id", J.Str{id}}, J.KV{"ver", J.Str{ver}}, J.KV{"prev", J.Str{prev}}, J.KV{"ch", J.Str{ch}}, J.KV{"os", J.Str{os}}, J.KV{"arch", J.Str{arch}}, J.KV{"pow", J.Str{pow}}, J.KV{"made", J.Json.nat(made)}, J.KV{"held", J.Flag{held}}])

def Tm.ev.eq(+a: TmEv, b: TmEv) -> Bool:
  String.eq(J.Json.show(Tm.ev.local(a)), J.Json.show(Tm.ev.local(b)))

def Tm.sent.has(xs: List<&2, TmEv>, +ev: TmEv) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> t: Bool.or(Tm.ev.eq(h, ev), Tm.sent.has(t, ev))

def Tm.sent.drop(xs: List<&2, TmEv>, +evs: List<&2, TmEv>) -> List<&2, TmEv>:
  match xs:
    case Nil{}: Nil{}
    case +h <> t: Tm.queue.cons(Bool.pick(Maybe<&2, TmEv>, Tm.sent.has(evs, h), None{}, Some{h}), Tm.sent.drop(t, evs))

def Tm.sent(st: TmState, +evs: List<&2, TmEv>, +status: Nat, +retry_after: Nat, +now: Nat) -> TmState:
  match st:
    case TmState{id, ver, today, share, reset, +q, +next, busy, seen}:
      +retry = Bool.or(Bool.or(Nat.is_eq(status, 0n), Nat.is_eq(status, 429n)), Nat.is_ge(status, 500n))
      TmState{id, ver, today, share, reset, Bool.pick(List<&2, TmEv>, retry, q, Tm.sent.drop(q, evs)), Bool.pick(Nat, retry, Nat.add(now, Bool.pick(Nat, Nat.is_ge(retry_after, 3600n), retry_after, 3600n)), next), busy, seen}

def Tm.pow.put.ev(ev: TmEv, +id: String, +proof: String) -> TmEv:
  match ev:
    case TmEv{+e, day, share, +eid, ver, prev, ch, os, arch, +pow, made, held}:
      TmEv{e, day, share, eid, ver, prev, ch, os, arch, Bool.pick(String, Bool.and(String.eq(e, "install"), String.eq(eid, id)), proof, pow), made, held}

def Tm.pow.put.queue(xs: List<&2, TmEv>, +id: String, +proof: String) -> List<&2, TmEv>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: Tm.pow.put.ev(h, id, proof) <> Tm.pow.put.queue(t, id, proof)

def Tm.pow.put(st: TmState, +proof: String) -> TmState:
  match st:
    case TmState{+id, ver, today, share, reset, q, next, busy, seen}:
      TmState{id, ver, today, share, reset, Tm.pow.put.queue(q, id, proof), next, busy, seen}

def TmState.id(st: TmState) -> String:
  match st:
    case TmState{id, _, _, _, _, _, _, _, _}: id

def Tm.json.number(j: J.Json) -> Nat:
  match j:
    case J.Num{s}: Tm.num(s)
    case _: 0n

def Tm.json.num(j: J.Json, key: String) -> Nat:
  Tm.json.number(Maybe.default(&2, J.Json, J.Json.get(j, key), J.Null{}))

def Tm.ev.parse(+j: J.Json) -> TmEv:
  TmEv{J.Json.get_str(j, "e", ""), J.Json.get_str(j, "day", ""), J.Json.get_bool(j, "share", False{}), J.Json.get_str(j, "id", ""), J.Json.get_str(j, "ver", ""), J.Json.get_str(j, "prev", ""), J.Json.get_str(j, "ch", ""), J.Json.get_str(j, "os", ""), J.Json.get_str(j, "arch", ""), J.Json.get_str(j, "pow", ""), Tm.json.num(j, "made"), J.Json.get_bool(j, "held", False{})}

def Tm.evs.parse(j: J.Json) -> List<&2, TmEv>:
  match j:
    case J.Arr{xs}: Tm.evs.parse(xs)
    case J.Item{h, t}: Tm.ev.parse(h) <> Tm.evs.parse(t)
    case _: Nil{}

def Tm.state.of(+j: J.Json) -> TmState:
  TmState{J.Json.get_str(j, "id", ""), J.Json.get_str(j, "ver", ""), J.Json.get_str(j, "today", ""), J.Json.get_bool(j, "share", True{}), J.Json.get_str(j, "reset", "0"), Tm.evs.parse(Maybe.default(&2, J.Json, J.Json.get(j, "queue"), J.Null{})), Tm.json.num(j, "next"), Tm.json.num(j, "busy"), J.Json.get_bool(j, "seen", False{})}

def Tm.evs.local(xs: List<&2, TmEv>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: Tm.ev.local(h) <> Tm.evs.local(t)

def Tm.state.json(st: TmState) -> String:
  match st:
    case TmState{id, ver, today, share, reset, queue, next, busy, seen}:
      J.Json.show(J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"ver", J.Str{ver}}, J.KV{"today", J.Str{today}}, J.KV{"share", J.Flag{share}}, J.KV{"reset", J.Str{reset}}, J.KV{"queue", J.Json.arr(Tm.evs.local(queue))}, J.KV{"next", J.Json.nat(next)}, J.KV{"busy", J.Json.nat(busy)}, J.KV{"seen", J.Flag{seen}}]))

def Tm.ev.unhold(+ev: TmEv, id: String) -> TmEv:
  match ev:
    case TmEv{_, _, _, _, _, _, _, _, _, _, _, True{}}: Tm.ev.release(ev, True{}, id)
    case _: ev

def Tm.batch.fit(fit: Bool, +h: TmEv, n: Nat, bytes: Nat, acc: List<&2, TmEv>, k: Nat -> Nat -> List<&2, TmEv> -> List<&2, TmEv>) -> List<&2, TmEv>:
  match fit:
    case False{}: List.reverse(&2, TmEv, acc)
    case True{}: k( 1n+n, Nat.add(bytes, 1n+String.length(J.Json.show(Tm.ev.json(h)))), h <> acc)

def Tm.batch.step(clean: Bool, +h: TmEv, +n: Nat, +bytes: Nat, acc: List<&2, TmEv>, k: Nat -> Nat -> List<&2, TmEv> -> List<&2, TmEv>) -> List<&2, TmEv>:
  match clean:
    case False{}: k( n, bytes, acc)
    case True{}:
      Tm.batch.fit(Bool.and(Nat.is_lt(n, 50n), Nat.is_le(Nat.add(bytes, 1n+String.length(J.Json.show(Tm.ev.json(h)))), 16256n)), h, n, bytes, acc, k)

# Leave at least 128 bytes for the envelope (a unix timestamp fits in 20).
def Tm.batch.go(xs: List<&2, TmEv>, +n: Nat, +bytes: Nat, acc: List<&2, TmEv>) -> List<&2, TmEv>:
  match xs:
    case Nil{}: List.reverse(&2, TmEv, acc)
    case +h <> t:
      Tm.batch.step(Tm.clean(h), h, n, bytes, acc, nn => bb => aa => Tm.batch.go(t, nn, bb, aa))

def Tm.batch(xs: List<&2, TmEv>) -> List<&2, TmEv>:
  Tm.batch.go(xs, 0n, 0n, Nil{})

def Tm.body(ts: Nat, +nonce: String, evs: List<&2, TmEv>) -> String:
  Tm.body.raw(ts, Bool.pick(String, Tm.hex.n(nonce, 16n), nonce, "0000000000000000"), Tm.batch(evs))

def Bug.url(+ver: String, +os: String, +arch: String) -> String:
  "https://github.com/i2cjak/Backplane/issues/new?body=" ++ IM.Img.enc.bytes(U8.Utf8.encode("Version: " ++ Bool.pick(String, Tm.version(ver), ver, "unknown") ++ "\nOS: " ++ Bool.pick(String, Tm.has(["linux", "darwin", "windows"], os), os, "unknown") ++ " " ++ Bool.pick(String, Tm.has(["x64", "arm64"], arch), arch, "unknown") ++ "\n\nWhat happened:\n"))

def Tm.ev.hold(ev: TmEv) -> TmEv:
  match ev:
    case TmEv{e, day, _, _, ver, prev, ch, os, arch, _, made, _}: TmEv{e, day, False{}, "", ver, prev, ch, os, arch, "", made, True{}}

def Tm.queue.gate.one(ev: TmEv, +v: TmView, +id: String) -> Maybe<&2, TmEv>:
  match ev v:
    case +x TmView{False{}, _, _, _, _, _, _, _, _}:
      Bool.pick(Maybe<&2, TmEv>, String.eq(TmEv.e(x), "active"), None{}, Some{Tm.ev.off(x)})
    case +x TmView{True{}, +asked, _, _, _, _, _, _, +now}:
      Bool.pick(Maybe<&2, TmEv>, Nat.is_gt(Nat.sub(now, TmEv.made(x)), 604800n), None{}, Some{Bool.pick(TmEv, asked, Tm.ev.unhold(x, id), Bool.pick(TmEv, Nat.is_ge(Nat.sub(now, TmEv.made(x)), 259200n), Tm.ev.off(x), Tm.ev.hold(x)))})

def Tm.queue.gate(xs: List<&2, TmEv>, +v: TmView, +id: String) -> List<&2, TmEv>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: Tm.queue.cons(Tm.queue.gate.one(h, v, id), Tm.queue.gate(t, v, id))

def Tm.plan.fields(+id0: String, +ver0: String, +today: String, +share0: Bool, +reset0: String, +q0: List<&2, TmEv>, next: Nat, +seen: Bool, +v: TmView, +fresh_id: String) -> TmState:
  match v:
    case TmView{+share, +asked, +reset, +ver, +ch, +os, +arch, +day, +now}:
      +rotate = Bool.and(share, Bool.or(Bool.not(share0), Bool.not(String.eq(reset0, reset))))
      +id = Bool.pick(String, Bool.and(share, asked), Bool.pick(String, Bool.or(rotate, String.is_empty(id0)), fresh_id, id0), "")
      +q1 = Bool.pick(List<&2, TmEv>, rotate, Tm.queue.dropids(q0), q0)
      +q2 = List.append(&2, TmEv, q1, Bool.pick(List<&2, TmEv>, Bool.or(Bool.not(seen), Bool.and(Bool.and(share, asked), Bool.or(rotate, String.is_empty(id0)))), [Tm.ev.new("install", "", v, id)], Nil{}))
      +q3 = List.append(&2, TmEv, q2, Bool.pick(List<&2, TmEv>, Bool.and(seen, Bool.not(String.eq(ver0, ver))), [Tm.ev.new("update", ver0, v, id)], Nil{}))
      +active = Bool.and(Bool.and(Bool.and(share, asked), share0), Bool.and(Bool.not(rotate), Bool.and(Bool.not(String.is_empty(id0)), Bool.and(Bool.not(String.is_empty(today)), Bool.not(String.eq(today, day))))))
      +q4 = List.append(&2, TmEv, q3, Bool.pick(List<&2, TmEv>, active, [TmEv{"active", today, True{}, id, ver0, "", ch, os, arch, "", now, False{}}], Nil{}))
      TmState{id, ver, day, share, reset, Tm.queue.limit(Tm.queue.dedupe(Tm.queue.gate(Tm.queue.age(q4, now), v, id), Nil{})), next, 0n, True{}}

def Tm.plan.build(st: TmState, +v: TmView, +fresh_id: String) -> TmState:
  match st:
    case TmState{+id0, +ver0, +today, +share0, +reset0, +q0, next, _, +seen}:
      Tm.plan.fields(id0, ver0, today, share0, reset0, q0, next, seen, v, fresh_id)

def Tm.plan(+st: TmState, +v: TmView, +fresh_id: String) -> TmPlan:
  +s = Tm.plan.build(st, v, fresh_id)
  TmPlan{s, Tm.needpow(TmState.queue(s))}

def Tm.uuid.variant(s: String) -> String:
  match s:
    case SCon{c, _}: SCon{SH.Hex.digit((SH.Hex.val(c) .&. 3 .|. 8 : U32)), ""}
    case SNil{}: "8"

def Tm.uuid(+hex32: String) -> String:
  String.take(hex32, 8n) ++ "-" ++ String.take(String.drop(hex32, 8n), 4n) ++ "-4" ++ String.take(String.drop(hex32, 13n), 3n) ++ "-" ++ Tm.uuid.variant(String.drop(hex32, 16n)) ++ String.take(String.drop(hex32, 17n), 3n) ++ "-" ++ String.drop(hex32, 20n)

def Tm.json.type(j: J.Json, typ: Nat) -> Bool:
  match j typ:
    case J.Str{_} 0n: True{}
    case J.Flag{_} 1n: True{}
    case J.Num{+s} 2n: Bool.and(Tm.number(s), Nat.is_le(String.length(s), 20n))
    case J.Arr{_} 3n: True{}
    case _ _: False{}

def Tm.json.field(j: J.Json, key: String, typ: Nat) -> Bool:
  Tm.json.type(Maybe.default(&2, J.Json, J.Json.get(j, key), J.Null{}), typ)

def Tm.ev.valid(+j: J.Json) -> Bool:
  Bool.and(Tm.json.field(j, "e", 0n), Bool.and(Tm.json.field(j, "day", 0n), Bool.and(Tm.json.field(j, "share", 1n), Bool.and(Tm.json.field(j, "id", 0n), Bool.and(Tm.json.field(j, "ver", 0n), Bool.and(Tm.json.field(j, "prev", 0n), Bool.and(Tm.json.field(j, "ch", 0n), Bool.and(Tm.json.field(j, "os", 0n), Bool.and(Tm.json.field(j, "arch", 0n), Bool.and(Tm.json.field(j, "pow", 0n), Bool.and(Tm.json.field(j, "made", 2n), Tm.json.field(j, "held", 1n))))))))))))

def Tm.queue.valid(j: J.Json) -> Bool:
  match j:
    case J.Arr{xs}: Tm.queue.valid(xs)
    case J.Item{h, t}: Bool.and(Tm.ev.valid(h), Tm.queue.valid(t))
    case J.End{}: True{}
    case _: False{}

def Tm.state.valid(+j: J.Json) -> Bool:
  Bool.and(Tm.json.field(j, "id", 0n), Bool.and(Tm.json.field(j, "ver", 0n), Bool.and(Tm.json.field(j, "today", 0n), Bool.and(Tm.json.field(j, "share", 1n), Bool.and(Tm.json.field(j, "reset", 0n), Bool.and(Tm.json.field(j, "next", 2n), Bool.and(Tm.json.field(j, "busy", 2n), Bool.and(Tm.json.field(j, "seen", 1n), Bool.and(Tm.json.field(j, "queue", 3n), Tm.queue.valid(Maybe.default(&2, J.Json, J.Json.get(j, "queue"), J.Null{})))))))))))

def Tm.state.parse.k(m: Maybe<&2, J.Json>) -> TmState:
  match m:
    case Some{+j}:
      +s = Tm.state.of(j)
      Bool.pick(TmState, Tm.state.valid(j), s, Tm.fresh())
    case None{}: Tm.fresh()

def Tm.state.parse(s: String) -> TmState:
  Tm.state.parse.k(J.Json.parse(s))
