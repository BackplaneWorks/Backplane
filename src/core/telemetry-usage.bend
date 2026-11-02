import Base
import ./json.bend as J
import ./model.bend as M
import ./telemetry.bend as T
import ./usage-counts.bend as UC

type UsageRow is Data:
  UsageRow{day: String, counts: String}

def Usage.current(privacy_gen: String, +st: M.State) -> Bool:
  Bool.and(M.Metrics.allowed(st), String.eq(privacy_gen, M.Setting.get(st, "telemetry.usage.generation", "")))

# Daily snapshots are cumulative, independently max-merged by the receiver.
# Seven bounded rows can be reconstructed after a restart without scanning
# conversations, token ledgers or files. Private projection keys never go out.
def Usage.event(day: String, counts: String, id: String, v: T.TmView) -> J.Json:
  match v:
    case T.TmView{_, _, _, ver, ch, os, arch, _, _}:
      J.Json.obj([J.KV{"e", J.Str{"usage"}}, J.KV{"day", J.Str{day}}, J.KV{"share", J.Flag{True{}}},
        J.KV{"id", J.Str{id}}, J.KV{"ver", J.Str{ver}}, J.KV{"ch", J.Str{ch}}, J.KV{"os", J.Str{os}}, J.KV{"arch", J.Str{arch}},
        J.KV{"counts", UC.Uc.json(UC.Uc.read(counts))}])

def Usage.one(nonempty: Bool, day: String, counts: String, tail: List<&2, UsageRow>) -> List<&2, UsageRow>:
  match nonempty:
    case False{}: tail
    case True{}: UsageRow{day, counts} <> tail

def Usage.days(n: Nat, +now: Nat, +s: M.State) -> List<&2, UsageRow>:
  match n:
    case 0n: Nil{}
    case 1n+k:
      +day = M.Tok.day(now)
      +counts = M.Metrics.get(s, day)
      Usage.one(Bool.not(String.is_empty(counts)), day, counts, Usage.days(k, Nat.sub(now, 86400n), s))

def Usage.rows.k(ok: Bool, now: Nat, s: M.State) -> List<&2, UsageRow>:
  match ok:
    case False{}: Nil{}
    case True{}: Usage.days(7n, now, s)

def Usage.rows(+now: Nat, +s: M.State) -> List<&2, UsageRow>:
  Usage.rows.k(M.Metrics.allowed(s), now, s)

def Usage.events(xs: List<&2, UsageRow>, +id: String, +v: T.TmView) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case UsageRow{day, counts} <> tail: Usage.event(day, counts, id, v) <> Usage.events(tail, id, v)

def Usage.snapshots.k(ok: Bool, now: Nat, s: M.State, id: String, v: T.TmView) -> List<&2, J.Json>:
  match ok:
    case False{}: Nil{}
    case True{}: Usage.events(Usage.rows(now, s), id, v)

def Usage.snapshots(+now: Nat, +s: M.State, +id: String, +v: T.TmView) -> List<&2, J.Json>:
  Usage.snapshots.k(Bool.and(M.Metrics.allowed(s), Bool.and(T.TmView.share(v), Bool.and(T.TmView.asked(v), T.Tm.uuid.ok(id)))), now, s, id, v)

def Usage.has(xs: List<&2, J.Json>, +j: J.Json) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> tail: Bool.or(String.eq(J.Json.show(h), J.Json.show(j)), Usage.has(tail, j))

def Usage.pending(xs: List<&2, J.Json>, +ack: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case +h <> +tail:
      Bool.pick(List<&2, J.Json>, Usage.has(ack, h), Usage.pending(tail, ack), h <> Usage.pending(tail, ack))

def Usage.body(now: Nat, nonce: String, events: List<&2, J.Json>) -> String:
  J.Json.show(J.Json.obj([J.KV{"v", J.Num{"1"}}, J.KV{"ts", J.Json.nat(now)}, J.KV{"nonce", J.Str{nonce}}, J.KV{"events", J.Json.arr(events)}]))

def Usage.registering(xs: List<&2, T.TmEv>) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> tail: Bool.or(String.eq(T.TmEv.e(h), "install"), Usage.registering(tail))
