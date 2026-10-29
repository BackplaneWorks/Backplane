import Base
import ../core/mailprivate.bend as PM
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/bot.bend as BT
import ../core/mail.bend as ML
import ../core/maildisposition.bend as MD
import ../core/ref.bend as R
import ../core/store.bend as ST
import ./db.bend as D

def Context.owner.live(live: Bool, +s: M.State, bot: String) -> String:
  match live:
    case False{}: ""
    case True{}: BT.Bot.thread(BT.World.bot(M.State.bots(s), bot))

# Full owner history, on demand. No arbitrary thread/bot parameter and no
# separate copy of the owner's memory. Bound parameters keep text inert.
def Context.owner.got(r: Maybe<&2, ML.Run>, +s: M.State, +caller: String) -> String:
  match r:
    case None{}: ""
    case Some{+r}: Context.owner.live(MD.Disposition.launch(s, caller), s, ML.Run.bot(r))

def Context.owner(+s: M.State, +caller: String) -> String:
  Bool.pick(String, PM.Private.live(s, caller), PM.Private.target(s, caller), Context.owner.got(ML.Box.run_of(M.State.mail(s), caller), s, caller))

def Context.fields(j: J.Json) -> Bool:
  match j:
    case J.End{}: True{}
    case J.Field{"query", J.Str{_}, t}: Context.fields(t)
    case J.Field{"before", J.Num{_}, t}: Context.fields(t)
    case _: False{}

def Context.args(j: J.Json) -> Bool:
  match j:
    case J.Obj{f}: Context.fields(f)
    case _: False{}

def Context.sql() -> String:
  "SELECT seq,id,role,text,at FROM entry WHERE thread=?1 AND kind='msg' AND (?2=0 OR seq<?2) AND (?3='' OR instr(lower(text),lower(?3))>0) ORDER BY seq DESC LIMIT 21"

def Context.rows(xs: List<&2, List<&2, String>>, n: Nat) -> List<&2, J.Json>:
  match xs n:
    case _ 0n: Nil{}
    case Nil{} _: Nil{}
    case (seq <> id <> role <> text <> at <> _) <> t 1n+k:
      J.Json.obj([J.KV{"seq", J.Num{seq}}, J.KV{"id", J.Str{id}}, J.KV{"role", J.Str{role}}, J.KV{"text", J.Str{text}}, J.KV{"at", J.Num{at}}]) <> Context.rows(t, k)
    case _ <> t +n: Context.rows(t, n)

def Context.next(xs: List<&2, List<&2, String>>, n: Nat, last: String) -> String:
  match xs n:
    case _ 0n: last
    case Nil{} _: "0"
    case (+seq <> _) <> t 1n+k: Context.next(t, k, seq)
    case _ <> t +n: Context.next(t, n, last)

def Context.result.k(failed: Bool, +xs: List<&2, List<&2, String>>, +owner: String) -> J.Json:
  match failed:
    case True{}: J.Json.obj([J.KV{"error", J.Str{"Owner history unavailable; do not infer missing context or acknowledge an important ask. Escalate to the coordinator."}}])
    case False{}: J.Json.obj([J.KV{"owner", J.Str{owner}}, J.KV{"link", J.Str{R.Ref.uri(R.Ref.thread(owner))}}, J.KV{"messages", J.Json.arr(Context.rows(xs, 20n))}, J.KV{"next", J.Num{Context.next(xs, 20n, "0")}}])

def Context.result(xs: List<&2, List<&2, String>>, +owner: String) -> J.Json:
  match xs:
    case (+err <> _) <> _:
      Context.result.k(String.starts_with(err, "\u{0}"), xs, owner)
    case Nil{}: Context.result.k(False{}, Nil{}, owner)
    case _ <> _: Context.result.k(False{}, xs, owner)

# A full-replay boot has no database handle: its committed wire log is
# complete, newest first. Count every wire line, as store.db does, so the
# same seq cursor works after a later stored boot. Never use this path for
# a stored boot's partial in-memory history.
def Context.log.query(empty: Bool, +o: J.Json, query: String) -> Bool:
  match empty:
    case True{}: True{}
    case False{}: String.contains(String.to_lower(J.Json.get_str(o, "text", "")), query)

def Context.log.scoped(keep: Bool, o: J.Json, +query: String) -> Bool:
  match keep:
    case False{}: False{}
    case True{}: Context.log.query(String.is_empty(query), o, query)

def Context.log.match(+o: J.Json, +owner: String, +query: String, +before: Nat, +seq: Nat) -> Bool:
  # Bool.and/or evaluate their arguments. Keep unrelated and unqueried
  # message bodies out of case conversion, rather than doing it eagerly.
  Context.log.scoped(Bool.and(String.eq(J.Json.get_str(o, "$", ""), "MessagePosted"),
    Bool.and(String.eq(J.Json.get_str(o, "thread", ""), owner),
      Bool.or(Nat.is_eq(before, 0n), Nat.is_lt(seq, before)))), o, query)

def Context.log.rows(ls: List<&2, String>, +owner: String, +query: String, +before: Nat, +seq: Nat, limit: Nat, +acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match ls limit:
    case _ 0n: List.reverse(&2, List<&2, String>, acc)
    case Nil{} _: List.reverse(&2, List<&2, String>, acc)
    case l <> t +limit:
      +o = Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{})
      +keep = Context.log.match(o, owner, query, before, seq)
      +row = {[Nat.show(seq), J.Json.get_str(o, "msg", ""), J.Json.get_str(o, "role", ""), J.Json.get_str(o, "text", ""), Nat.show(J.Json.get_nat(o, "at", 0n))] : List<&2, String>}
      Context.log.rows(t, owner, query, before, Nat.sub(seq, 1n), Nat.sub(limit, Bool.pick(Nat, keep, 1n, 0n)), Bool.pick(List<&2, List<&2, String>>, keep, row <> acc, acc))

# A spawned IO task still runs pure work synchronously on the shared loop.
# Never parse the whole global wire log before giving sockets/timers a turn.
# Keep the exact remaining log, global seq cursor and rows; no scan limit
# truncates history or changes the 20+1 lookahead used by Context.result.
type LogScan is Data:
  LogScan{lines: List<&2, String>, seq: Nat, remaining: Nat, rows: List<&2, List<&2, String>>}

def Context.log.quantum() -> Nat:
  32n

def Context.log.keep(keep: Bool, +o: J.Json, lines: List<&2, String>, +seq: Nat, remaining: Nat, rows: List<&2, List<&2, String>>) -> LogScan:
  match keep:
    case False{}: LogScan{lines, Nat.sub(seq, 1n), remaining, rows}
    case True{}: LogScan{lines, Nat.sub(seq, 1n), Nat.sub(remaining, 1n),
      [Nat.show(seq), J.Json.get_str(o, "msg", ""), J.Json.get_str(o, "role", ""), J.Json.get_str(o, "text", ""), Nat.show(J.Json.get_nat(o, "at", 0n))] <> rows}

def Context.log.step(fuel: Nat, scan: LogScan, +owner: String, +query: String, +before: Nat) -> LogScan:
  match fuel scan:
    case 0n s: s
    case 1n+n LogScan{Nil{}, seq, remaining, rows}: LogScan{Nil{}, seq, remaining, rows}
    case 1n+n LogScan{lines, seq, 0n, rows}: LogScan{lines, seq, 0n, rows}
    case 1n+n LogScan{l <> tail, +seq, remaining, rows}:
      +o = Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{})
      Context.log.step(n, Context.log.keep(Context.log.match(o, owner, query, before, seq), o, tail, seq, remaining, rows), owner, query, before)

def Context.log.after(scan: LogScan, owner: String, k: LogScan -> IO(J.Json)) -> IO(J.Json):
  match scan:
    case LogScan{Nil{}, _, _, rows}: IO.pure(J.Json, Context.result(List.reverse(&2, List<&2, String>, rows), owner))
    case LogScan{_, _, 0n, rows}: IO.pure(J.Json, Context.result(List.reverse(&2, List<&2, String>, rows), owner))
    case s:
      do IO<J.Json>:
        IO.sleep(0)
        k(s)

def Context.log.run(fuel: Nat, scan: LogScan, +owner: String, +query: String, +before: Nat) -> IO(J.Json):
  match fuel:
    # One plus the number of wire lines is more than the possible batches.
    # An invalid internal caller fails closed rather than claiming a page.
    case 0n: IO.pure(J.Json, Context.result.k(True{}, Nil{}, owner))
    case 1n+n:
      Context.log.after(Context.log.step(Context.log.quantum(), scan, owner, query, before), owner,
        s => Context.log.run(n, s, owner, query, before))

def Context.read.source(stored: Bool, +h: String, +owner: String, +a: J.Json, +ls: List<&2, String>) -> IO(J.Json):
  match stored:
    case False{}:
      +size = ST.Store.size(ls)
      Context.log.run(1n+size, LogScan{ls, size, 21n, Nil{}}, owner, String.to_lower(String.take(J.Json.get_str(a, "query", ""), 200n)), J.Json.get_nat(a, "before", 0n))
    case True{}:
      do IO<J.Json>:
        xs : List<&2, List<&2, String>> <- D.Db.query.t(h, Context.sql(), [D.DbText{owner}, D.DbInt{Nat.show(J.Json.get_nat(a, "before", 0n))}, D.DbText{String.take(J.Json.get_str(a, "query", ""), 200n)}], 21)
        return Context.result(xs, owner)

def Context.read(allowed: Bool, +h: String, +owner: String, +a: J.Json, +ls: List<&2, String>) -> IO(J.Json):
  match allowed:
    case False{}: IO.pure(J.Json, J.Json.obj([J.KV{"error", J.Str{"mail_context requires a live owner-scoped mailroom run and only query/before arguments"}}]))
    case True{}: Context.read.source(Bool.not(String.is_empty(h)), h, owner, a, ls)
