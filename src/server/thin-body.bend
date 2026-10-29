import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/thin.bend as T
import ../core/thin-wire.bend as W
import ../core/thinq.bend as Q
import ../core/sha256.bend as S
import ../core/utf8.bend as U8
import ./db.bend as D
import ../core/hub.bend as HB
import ../core/thin-live.bend as TL
import ../core/presentation.bend as P

# Negotiated read-only body windows. Ordinary short messages remain whole;
# large bodies retain their immutable source and use bounded UTF-8 pages.
# Neither a body page nor its local rendering grants action authority.
def Body.enabled(web: Bool, p: J.Json) -> Bool:
  Bool.and(web, Nat.is_eq(J.Json.get_nat(p, "bodyPages", 0n), 1n))

def Body.large.entry(e: M.Entry, blocks: T.Blocks, threshold: Nat) -> Bool:
  match e:
    case M.Msg{_, _, _, _, _}: Nat.is_gt(T.Blocks.n(blocks), threshold)
    case _: Nat.is_gt(T.Blocks.n(blocks), 1n)

def Body.threshold(p: J.Json) -> Nat:
  Bool.pick(Nat, Nat.is_eq(J.Json.get_nat(p, "bodyBudget", 0n), 1n), 1n, 8n)

def Body.large(+r: T.Row, p: J.Json) -> Bool:
  Body.large.entry(T.Row.entry(r), T.Row.blocks(r), Body.threshold(p))

def Body.row.pick(large: Bool, r: T.Row, preview: Nat) -> T.Row:
  match large r:
    case True{} T.Row{n, e, terms, +blocks}: T.Row{n, T.Entry.with_text(e, T.Blocks.join(preview, blocks, 0n, Nil{})), terms, blocks}
    case False{} _: r

def Body.row(+r: T.Row, +p: J.Json) -> T.Row:
  Body.row.pick(Body.large(r, p), r, Bool.pick(Nat, Nat.is_eq(Body.threshold(p), 1n), 1n, 8n))

def Body.rows(ids: List<&2, String>, +index: T.Index, +p: J.Json, acc: List<&2, T.Row>) -> List<&2, T.Row>:
  match ids:
    case Nil{}: acc
    case id <> tail: Body.rows(tail, index, p, Body.row(T.Index.row(index, Q.Q.num(id)), p) <> acc)

def Body.index(enabled: Bool, +index: T.Index, +thread: String, ids: List<&2, String>, p: J.Json) -> T.Index:
  match enabled:
    case False{}: index
    case True{}: T.Index.window.rows(Body.rows(ids, index, p, Nil{}), thread, index)

def Body.query(ord: Nat, until: Nat) -> String:
  "body:" ++ Nat.show(ord) ++ ":" ++ Nat.show(until)

def Body.part(+row: T.Row, epoch: String, target: J.Json) -> J.Json:
  J.Json.obj([J.KV{"id", J.Str{M.Idx.id(T.Row.entry(row))}}, J.KV{"thread", J.Str{M.Idx.thread(T.Row.entry(row))}}, J.KV{"version", J.Str{Nat.show(T.Row.n(row))}},
    J.KV{"cursor", J.Str{W.Cursor.write(epoch, target, Body.query(T.Row.n(row), 0n), 0n)}},
    J.KV{"hasMore", J.Flag{True{}}}, J.KV{"complete", J.Flag{False{}}}, J.KV{"reason", J.Str{"byte-limit"}},
    J.KV{"bodyPage", J.Num{"1"}}, J.KV{"automatic", J.Flag{Bool.not(T.Row.folds(T.Row.entry(row)))}}])

# SQLite slices the UTF-8 BLOB before crossing the IO boundary. Hex avoids
# a split scalar or NUL being repaired/lost by sqlite3_column_text. The
# four-byte lookahead completes a scalar at the fixed page boundary.
def Body.sql() -> String:
  "SELECT seq,ord,id,thread,src,kind,role,akind,hex(substr(CAST(text AS BLOB),CASE WHEN ?5=1 THEN max(0,length(CAST(text AS BLOB))-8192) ELSE ?3 END+1,8196)),at,length(CAST(text AS BLOB)),CASE WHEN ?5=1 THEN max(0,length(CAST(text AS BLOB))-8192) ELSE ?3 END FROM entry WHERE id=?2 AND ((?1>0 AND ord=?1) OR (?1=0 AND ord IN (SELECT value FROM json_each(?4)))) ORDER BY ord DESC LIMIT 1"

type Cursor is Data:
  Cursor{ordinal: Nat, start: Nat, until: Nat}

def Body.field(xs: List<&2, String>, n: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, xs, n), "")

def Body.cursor.valid(valid: Bool, value: Maybe<&2, Nat>, +ord: Nat, +until: Nat) -> Maybe<&2, Cursor>:
  match valid value:
    case True{} Some{start}: Some{Cursor{ord, start, until}}
    case _ _: None{}

def Body.cursor.empty(empty: Bool, +p: J.Json, +epoch: String) -> Maybe<&2, Cursor>:
  match empty:
    case True{}: Some{Cursor{0n, 0n, 0n}}
    case False{}:
      +j = Maybe.default(&2, J.Json, J.Json.parse(J.Json.get_str(p, "cursor", "")), J.Null{})
      +query = J.Json.get_str(j, "query", "")
      +xs = String.split(query, ':')
      +ord = Maybe.default(&2, Nat, P.Session.nat(Body.field(xs, 1n), P.Session.nat.max()), 0n)
      +until = Maybe.default(&2, Nat, P.Session.nat(Body.field(xs, 2n), P.Session.nat.max()), 0n)
      Body.cursor.valid(Bool.and(W.Wire.within(J.Json.get_str(p, "cursor", ""), 1024n), Bool.and(Nat.is_eq(List.length(&2, String, xs), 3n), Bool.and(String.eq(Body.field(xs, 0n), "body"), Nat.is_gt(ord, 0n)))), W.Cursor.read(J.Json.get_str(p, "cursor", ""), epoch, J.Json.get_obj(p, "target"), Body.query(ord, until)), ord, until)

def Body.cursor(+p: J.Json, epoch: String) -> Maybe<&2, Cursor>:
  Body.cursor.empty(String.is_empty(J.Json.get_str(p, "cursor", "")), p, epoch)

type Span is Data:
  Span{text: String, skipped: Nat, used: Nat}

def Body.width(+b: U32) -> Nat:
  Bool.pick(Nat, U32.is_lt(b, 128), 1n, Bool.pick(Nat, U32.is_lt(b, 224), 2n, Bool.pick(Nat, U32.is_lt(b, 240), 3n, 4n)))

type Scan is Data:
  Scan{bytes: List<&2, U32>, fuel: Nat, skipped: Nat, used: Nat, reversed: List<&2, U32>, done: Bool}

def Body.scan.pick(skip: Bool, fits: Bool, +bs: List<&2, U32>, +fuel: Nat, +skipped: Nat, +used: Nat, +acc: List<&2, U32>, +n: Nat) -> Scan:
  match skip fits:
    case True{} _: Scan{List.drop(&2, U32, bs, 1n), Nat.sub(fuel, 1n), Nat.add(skipped, 1n), used, acc, False{}}
    case False{} False{}: Scan{bs, fuel, skipped, used, acc, True{}}
    case False{} True{}: Scan{List.drop(&2, U32, bs, n), Nat.sub(fuel, n), skipped, Nat.add(used, n), List.reverse.go(&2, U32, List.take(&2, U32, bs, n), acc), False{}}

def Body.scan.step(bs: List<&2, U32>, +fuel: Nat, +skipped: Nat, +used: Nat, +acc: List<&2, U32>) -> Scan:
  match bs:
    case Nil{}: Scan{Nil{}, fuel, skipped, used, acc, True{}}
    case +b <> +rest:
      +n = Body.width(b)
      Body.scan.pick(Bool.and(Nat.is_eq(used, 0n), U32.is_eq(U32.and(b, 192), 128)),
        Bool.and(Nat.is_le(n, fuel), Nat.is_le(n, List.length(&2, U32, List.take(&2, U32, b <> rest, 4n)))), b <> rest, fuel, skipped, used, acc, n)

def Body.scan.loop(steps: Nat, state: Scan) -> Span:
  match steps state:
    case 0n Scan{_, _, skipped, used, acc, _}: Span{U8.Utf8.decode(List.reverse(&2, U32, acc)), skipped, used}
    case _ Scan{_, _, skipped, used, acc, True{}}: Span{U8.Utf8.decode(List.reverse(&2, U32, acc)), skipped, used}
    case 1n+k Scan{bs, fuel, skipped, used, acc, False{}}: Body.scan.loop(k, Body.scan.step(bs, fuel, skipped, used, acc))

def Body.span(hex: String, start: Nat, +until: Nat) -> Span:
  Body.scan.loop(8197n, Scan{Maybe.default(&2, List<&2, U32>, S.Hex.decode(hex), Nil{}),
    Bool.pick(Nat, Nat.is_eq(until, 0n), 8192n, Nat.min(8192n, Nat.sub(until, start))), 0n, 0n, Nil{}, False{}})

def Body.owns(+row: T.Row, +thread: String, +id: String) -> Bool:
  Bool.and(Nat.is_gt(T.Row.n(row), 0n), Bool.and(String.eq(M.Idx.id(T.Row.entry(row)), id),
    Bool.or(String.eq(M.Idx.thread(T.Row.entry(row)), thread), String.eq(M.Link.from(M.Idx.id(T.Row.entry(row))), thread))))

type Read is Data:
  Read{code: String, message: J.Json}

def Body.result.step(+row: T.Row, span: Span, +total: Nat, +start: Nat, p: J.Json, +epoch: String, back: Nat) -> Read:
  match span:
    case Span{text, skipped, used}:
      +first = Nat.add(start, skipped)
      +end = Nat.add(first, used)
      +ord = T.Row.n(row)
      +target = J.Json.get_obj(p, "target")
      Read{"", J.Json.obj([J.KV{"id", J.Str{M.Idx.id(T.Row.entry(row))}}, J.KV{"thread", J.Str{M.Idx.thread(T.Row.entry(row))}},
        J.KV{"version", J.Str{Nat.show(ord)}}, J.KV{"text", J.Str{text}}, J.KV{"bodyPage", J.Num{"1"}},
        J.KV{"start", J.Json.nat(first)}, J.KV{"end", J.Json.nat(end)}, J.KV{"total", J.Json.nat(total)},
        J.KV{"hasMore", J.Flag{Nat.is_lt(end, total)}}, J.KV{"complete", J.Flag{Nat.is_ge(end, total)}},
        J.KV{"cursor", J.Str{Bool.pick(String, Nat.is_lt(end, total), W.Cursor.write(epoch, target, Body.query(ord, 0n), end), "")}},
        J.KV{"previous", J.Str{Bool.pick(String, Nat.is_gt(first, 0n), W.Cursor.write(epoch, target, Body.query(ord, first), Nat.sub(first, back)), "")}}])}

def Body.result(row: T.Row, span: Span, total: Nat, start: Nat, p: J.Json, epoch: String) -> Read:
  Body.result.step(row, span, total, start, p, epoch, 8192n)

# Live body ranges use immutable process-local block coordinates. A delta
# changes the version, so no cursor can splice two stream incarnations.
def Body.live.metadata(stream: TL.Stream, part: J.Json) -> J.Json:
  match stream part:
    case TL.Stream{generation, _, blocks, cut, _} J.Obj{fields}:
      J.Obj{J.Field{"streamGeneration", J.Str{Nat.show(generation)}, J.Field{"streamCut", J.Flag{cut},
        J.Field{"streamBlocks", J.Json.nat(T.Blocks.n(blocks)), fields}}}}
    case _ _: part

def Body.live.part(+stream: TL.Stream, +thread: String, +epoch: String, +target: J.Json) -> J.Json:
  Body.live.metadata(stream, Body.part(T.Row{TL.Stream.version(stream), M.Msg{TL.Stream.id(thread), thread, M.Assistant{}, "", 0n}, Map.new(&2, Bool), TL.Stream.blocks(stream)}, epoch, target))

def Body.live.preview(+on: Bool, +stream: TL.Stream) -> String:
  match on:
    case False{}: TL.Stream.preview(stream)
    case True{}: Bool.pick(String, TL.Stream.cut(stream), "…\n", "") ++ T.Blocks.join(8n, TL.Stream.blocks(stream), Nat.sub(T.Blocks.n(TL.Stream.blocks(stream)), 8n), Nil{})

def Body.live.page(+valid: Bool, +stream: TL.Stream, +thread: String, +p: J.Json, +epoch: String, +start: Nat, +until: Nat) -> Read:
  match valid:
    case False{}: Read{"STALE_CURSOR", J.Null{}}
    case True{}:
      +n = T.Blocks.n(TL.Stream.blocks(stream))
      +bound = Bool.pick(Nat, Nat.is_eq(until, 0n), n, until)
      +count = Nat.min(16n, Nat.sub(bound, start))
      Body.result.step(T.Row{TL.Stream.version(stream), M.Msg{TL.Stream.id(thread), thread, M.Assistant{}, "", 0n}, Map.new(&2, Bool), T.Blocks.of("")},
        Span{Bool.pick(String, Bool.and(TL.Stream.cut(stream), Nat.is_eq(start, 0n)), "…\n", "") ++ T.Blocks.join(count, TL.Stream.blocks(stream), start, Nil{}), 0n, count}, n, start, p, epoch, 16n)

def Body.live.cursor(+cursor: Maybe<&2, Cursor>, +stream: TL.Stream, +thread: String, +p: J.Json, +epoch: String) -> Read:
  match cursor:
    case None{}: Read{"STALE_CURSOR", J.Null{}}
    case Some{Cursor{+version, +at, +until}}:
      +n = T.Blocks.n(TL.Stream.blocks(stream))
      +start = Bool.pick(Nat, Bool.and(J.Json.get_bool(p, "bodyTail", False{}), Bool.and(Nat.is_eq(at, 0n), Nat.is_eq(until, 0n))), Nat.sub(n, 16n), at)
      Body.live.page(Bool.and(String.eq(J.Json.get_str(p, "bodyVersion", ""), Nat.show(TL.Stream.version(stream))),
        Bool.and(Nat.is_eq(version, TL.Stream.version(stream)), Bool.and(Nat.is_lt(start, n), Bool.or(Nat.is_eq(until, 0n), Bool.and(Nat.is_gt(until, start), Nat.is_le(until, n)))))),
        stream, thread, p, epoch, start, until)

def Body.live.read(+stream: TL.Stream, +thread: String, +p: J.Json, +epoch: String) -> Read:
  Body.live.cursor(Body.cursor(p, epoch), stream, thread, p, epoch)

def Body.checked.bounds(valid: Bool, +row: T.Row, xs: List<&2, String>, +start: Nat, until: Nat, total: Nat, p: J.Json, epoch: String) -> Read:
  match valid:
    case False{}: Read{"STALE_CURSOR", J.Null{}}
    case True{}: Body.result(row, Body.span(Body.field(xs, 8n), start, until), total, start, p, epoch)

def Body.checked(valid: Bool, +xs: List<&2, String>, +row: T.Row, +cursor: Cursor, +p: J.Json, epoch: String) -> Read:
  match valid cursor:
    case False{} _: Read{"NOT_FOUND", J.Null{}}
    case True{} Cursor{_, +start, +until}:
      +total = Q.Q.num(Body.field(xs, 10n))
      Body.checked.bounds(Bool.and(String.eq(J.Json.get_str(p, "bodyVersion", ""), Nat.show(T.Row.n(row))),
        Bool.and(Nat.is_lt(start, total), Bool.or(Nat.is_eq(until, 0n), Bool.and(Nat.is_gt(until, start), Nat.is_le(until, total))))), row, xs, start, until, total, p, epoch)

def Body.cursor.at(cursor: Cursor, start: Nat) -> Cursor:
  match cursor:
    case Cursor{ord, _, until}: Cursor{ord, start, until}

def Body.read.rows(xs: List<&2, List<&2, String>>, cursor: Cursor, +thread: String, +p: J.Json, epoch: String) -> Read:
  match xs:
    case Nil{}: Read{"NOT_FOUND", J.Null{}}
    case +row <> _:
      # Q.row reads the ordinary first ten fields; never parse hex as body.
      +meta : T.Row = T.Row{Q.Q.num(Body.field(row, 1n)), M.Msg{Body.field(row, 2n), Body.field(row, 3n), M.Assistant{}, "", Q.Q.num(Body.field(row, 9n))}, Map.new(&2, Bool), T.Blocks.of("")}
      Body.checked(Body.owns(meta, thread, J.Json.get_str(p, "entryId", "")), row, meta, Body.cursor.at(cursor, Q.Q.num(Body.field(row, 11n))), p, epoch)

def Body.read.cursor(value: Maybe<&2, Cursor>, h: String, thread: String, +p: J.Json, epoch: String, ids: List<&2, String>) -> IO(Read):
  match value:
    case None{}: IO.pure(Read, Read{"STALE_CURSOR", J.Null{}})
    case Some{+cursor}:
      match cursor:
        case Cursor{ord, +start, +until}:
          do IO<Read>:
            +rows : List<&2, List<&2, String>> <- D.Db.query.t(h, Body.sql(), [D.DbInt{Nat.show(ord)}, D.DbText{J.Json.get_str(p, "entryId", "")}, D.DbInt{Nat.show(start)}, D.DbText{J.Json.show(J.Json.arr(Q.Q.json.nats(ids)))}, D.DbInt{Bool.pick(String, Bool.and(J.Json.get_bool(p, "bodyTail", False{}), Bool.and(Nat.is_eq(start, 0n), Nat.is_eq(until, 0n))), "1", "0")}], 1)
            return Body.read.rows(rows, cursor, thread, p, epoch)

def Body.read(h: String, thread: String, +p: J.Json, +epoch: String, ids: List<&2, String>) -> IO(Read):
  Body.read.cursor(Body.cursor(p, epoch), h, thread, p, epoch, ids)

# Memory/mirror and legacy report compatibility use the existing immutable
# block index. These coordinates are opaque block positions, like live text.
def Body.row.page(+valid: Bool, +row: T.Row, +start: Nat, +until: Nat, +p: J.Json, +epoch: String) -> Read:
  match valid:
    case False{}: Read{"STALE_CURSOR", J.Null{}}
    case True{}:
      +n = T.Blocks.n(T.Row.blocks(row))
      +bound = Bool.pick(Nat, Nat.is_eq(until, 0n), n, until)
      +count = Nat.min(16n, Nat.sub(bound, start))
      Body.result.step(row, Span{T.Blocks.join(count, T.Row.blocks(row), start, Nil{}), 0n, count}, n, start, p, epoch, 16n)

def Body.row.cursor(+value: Maybe<&2, Cursor>, +row: T.Row, +thread: String, +p: J.Json, +epoch: String) -> Read:
  match value:
    case None{}: Read{"STALE_CURSOR", J.Null{}}
    case Some{Cursor{+ord, +at, +until}}:
      +n = T.Blocks.n(T.Row.blocks(row))
      +start = Bool.pick(Nat, Bool.and(J.Json.get_bool(p, "bodyTail", False{}), Bool.and(Nat.is_eq(at, 0n), Nat.is_eq(until, 0n))), Nat.sub(n, 16n), at)
      Body.row.page(Bool.and(Body.owns(row, thread, J.Json.get_str(p, "entryId", "")), Bool.and(
        String.eq(J.Json.get_str(p, "bodyVersion", ""), Nat.show(T.Row.n(row))), Bool.and(
          Bool.or(Nat.is_eq(ord, 0n), Nat.is_eq(ord, T.Row.n(row))), Bool.and(Nat.is_lt(start, n),
            Bool.or(Nat.is_eq(until, 0n), Bool.and(Nat.is_gt(until, start), Nat.is_le(until, n))))))), row, start, until, p, epoch)

def Body.row.read(+row: T.Row, thread: String, +p: J.Json, +epoch: String) -> Read:
  Body.row.cursor(Body.cursor(p, epoch), row, thread, p, epoch)

def Body.ordinal(+p: J.Json) -> Nat:
  Maybe.default(&2, Nat, P.Session.nat(J.Json.get_str(p, "bodyVersion", ""), P.Session.nat.max()), 0n)

# Old cut reports have a bounded stored prefix and an exact legacy suffix.
# A normal message never takes this whole-row compatibility path.
def Body.legacy.enabled(+p: J.Json) -> Bool:
  Bool.and(HB.Lr.is(J.Json.get_str(p, "entryId", "")), Bool.not(String.is_empty(M.Link.from(J.Json.get_str(p, "entryId", "")))))

def Body.legacy.rows(rows: List<&2, List<&2, String>>) -> T.Row:
  match rows:
    case row <> _: Q.Q.row(row)
    case Nil{}: T.Row.blank()

def Body.legacy.on(on: Bool, h: String, +p: J.Json) -> IO(T.Row):
  match on:
    case False{}: IO.pure(T.Row, T.Row.blank())
    case True{}:
      do IO<T.Row>:
        +rows : List<&2, List<&2, String>> <- D.Db.query.t(h, "SELECT " ++ Q.Q.cols.full() ++ " FROM entry WHERE ord=?1 AND id=?2 AND kind='msg' AND length(text)<=8192 AND substr(text,-length(?3))=?3 LIMIT 1",
          [D.DbInt{Nat.show(Body.ordinal(p))}, D.DbText{J.Json.get_str(p, "entryId", "")}, D.DbText{HB.Lr.old.tail(M.Link.from(J.Json.get_str(p, "entryId", "")))}], 1)
        return Body.legacy.rows(rows)

def Body.legacy.read(h: String, +p: J.Json) -> IO(T.Row):
  Body.legacy.on(Body.legacy.enabled(p), h, p)

# Pressure keeps the actual reading identity while reducing both sides of
# its window. Keeping every newer row up to an old anchor cannot converge.
def Body.window.anchor.go(ids: List<&2, String>, found: Bool, +index: T.Index, +anchor: String, +at: Nat) -> Nat:
  match ids found:
    case _ True{}: Nat.sub(at, 1n)
    case Nil{} False{}: 0n
    case id <> rest False{}:
      Body.window.anchor.go(rest, String.eq(M.Idx.id(T.Row.entry(T.Index.row(index, Q.Q.num(id)))), anchor), index, anchor, Nat.add(at, 1n))

def Body.window.anchor(ids: List<&2, String>, index: T.Index, anchor: String, at: Nat) -> Nat:
  Body.window.anchor.go(ids, False{}, index, anchor, at)

def Body.window(+ids: List<&2, String>, index: T.Index, anchor: String) -> List<&2, String>:
  +n = List.length(&2, String, ids)
  +count = Nat.max(1n, Nat.div(n, 2n))
  +start = Nat.min(Body.window.anchor(ids, index, anchor, 0n), Nat.sub(n, count))
  List.take(&2, String, List.drop(&2, String, ids, start), count)
