import Base
import ../core/mailbody.bend as MB
import ../core/mail.bend as ML
import ../core/ref.bend as R
import ../core/model.bend as M
import ../core/utf8.bend as U
import ./host.bend as H
import ../core/bot.bend as BT

# All public results distinguish an absent body file from a failed operation.
type StoreResult<-A: Data> is Data:
  Stored{value: A}
  Missing{path: String}
  StoreError{path: String, reason: String}

# Base.close cannot report errors; finish owns truncate/write/close together.
# The new open also rejects symlinks and nonregular files (including FIFOs).
def MailFile.open(path: String, write: U32) -> IO(Result<&1, &1, U32 & String, File>):
  import "./effects/mailstore.c"
  import "./effects/mailstore.js"

def MailFile.finish(file: File, cut: U32, text: String) -> IO(Result<&1, &1, U32 & String, Unit>):
  import "./effects/mailstore.c"
  import "./effects/mailstore.js"

def MailStore.error(-A: Data, path: String, e: U32 & String) -> StoreResult<A>:
  (code, msg) = e
  StoreError{path, U32.show(code) ++ ": " ++ msg}

def MailStore.absent(-A: Data, missing: Bool, path: String, e: U32 & String) -> StoreResult<A>:
  match missing:
    case True{}: Missing{path}
    case False{}: MailStore.error(A, path, e)

def MailStore.open_error(-A: Data, path: String, e: U32 & String) -> StoreResult<A>:
  (+code, msg) = e
  MailStore.absent(A, U32.is_eq(code, 2), path, (code, msg))

# Backward scan: skip the incomplete suffix without retaining it, then keep
# just the final complete row. IO is 16 KiB per step; no history scan.
type Tail is Data:
  Tail{pos: U32, cut: U32, found: Bool, done: Bool, row: List<&2, U32>}

def Tail.step.nl(nl: Bool, x: U32, t: Tail) -> Tail:
  match nl t:
    case _ Tail{pos, cut, found, True{}, row}: Tail{pos, cut, found, True{}, row}
    case True{} Tail{+pos, _, False{}, False{}, row}: Tail{U32.sub(pos, 1), pos, True{}, False{}, row}
    case True{} Tail{pos, cut, True{}, False{}, row}: Tail{U32.sub(pos, 1), cut, True{}, True{}, row}
    case False{} Tail{pos, cut, False{}, False{}, row}: Tail{U32.sub(pos, 1), cut, False{}, False{}, row}
    case False{} Tail{pos, cut, True{}, False{}, row}: Tail{U32.sub(pos, 1), cut, True{}, False{}, x <> row}

def Tail.step(+x: U32, t: Tail) -> Tail:
  Tail.step.nl(U32.is_eq(x, 10), x, t)

def Tail.bytes(bs: List<&2, U32>, t: Tail) -> Tail:
  match bs:
    case Nil{}: t
    case h <> rest: Tail.bytes(rest, Tail.step(h, t))

def MailStore.tail.chunk(r: File & Result<&1, &1, U32 & String, List<&2, U32>>, t: Tail) -> IO(File & Result<&2, &2, String, Tail>):
  (file, got) = r
  match got:
    case Fail{(code, msg)}: IO.pure(File & Result<&2, &2, String, Tail>, (file, Fail{U32.show(code) ++ ": " ++ msg}))
    case Done{bs}: IO.pure(File & Result<&2, &2, String, Tail>, (file, Done{Tail.bytes(List.reverse(&2, U32, bs), t)}))

def MailStore.tail.next(r: File & Result<&2, &2, String, Tail>, k: File -> Tail -> IO(File & Result<&2, &2, String, Tail>)) -> IO(File & Result<&2, &2, String, Tail>):
  (file, got) = r
  match got:
    case Fail{e}: IO.pure(File & Result<&2, &2, String, Tail>, (file, Fail{e}))
    case Done{t}: k(file, t)

def MailStore.tail(fuel: Nat, file: File, t: Tail) -> IO(File & Result<&2, &2, String, Tail>):
  match fuel t:
    case _ Tail{0, cut, found, done, row}: IO.pure(File & Result<&2, &2, String, Tail>, (file, Done{Tail{0, cut, found, done, row}}))
    case _ Tail{pos, cut, found, True{}, row}: IO.pure(File & Result<&2, &2, String, Tail>, (file, Done{Tail{pos, cut, found, True{}, row}}))
    case 0n _: IO.pure(File & Result<&2, &2, String, Tail>, (file, Fail{"tail scan exhausted"}))
    case 1n+k Tail{+pos, cut, found, False{}, row}:
      do IO<File & Result<&2, &2, String, Tail>>:
        +start : U32 = Bool.pick(U32, U32.is_lt(pos, 16384), 0, U32.sub(pos, 16384))
        r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_at(file, start, U32.sub(pos, start))
        got : File & Result<&2, &2, String, Tail> <- MailStore.tail.chunk(r, Tail{pos, cut, found, False{}, row})
        MailStore.tail.next(got, f => t => MailStore.tail(k, f, t))

# A syntactically complete but bad row is never repaired away. A suffix
# without a newline is a crashed write and is truncated on the next append.
def MailStore.tail.valid(found: Bool, row: List<&2, U32>) -> Bool:
  match found:
    case False{}: True{}
    case True{}: Maybe.is_some(&2, MB.Body, MB.Body.parse(U.Utf8.decode(row)))

def MailStore.append.finished(path: String, repaired: Bool, r: Result<&1, &1, U32 & String, Unit>) -> StoreResult<Bool>:
  match r:
    case Fail{e}: MailStore.error(Bool, path, e)
    case Done{_}: Stored{repaired}

def MailStore.append.valid(ok: Bool, file: File, path: String, cut: U32, text: String, repaired: Bool) -> IO(StoreResult<Bool>):
  match ok:
    case False{}:
      do IO<StoreResult<Bool>>:
        File.close(file)
        return StoreError{path, "malformed final completed body record"}
    case True{}:
      do IO<StoreResult<Bool>>:
        r : Result<&1, &1, U32 & String, Unit> <- MailFile.finish(file, cut, text)
        return MailStore.append.finished(path, repaired, r)

def MailStore.append.tail(r: File & Result<&2, &2, String, Tail>, path: String, size: U32, text: String) -> IO(StoreResult<Bool>):
  (file, got) = r
  match got:
    case Fail{e}:
      do IO<StoreResult<Bool>>:
        File.close(file)
        return StoreError{path, e}
    case Done{Tail{_, +cut, found, _, row}}:
      MailStore.append.valid(MailStore.tail.valid(found, row), file, path, cut, text, U32.is_ne(cut, size))

def MailStore.append.size(r: File & Result<&1, &1, U32 & String, U32>, path: String, text: String) -> IO(StoreResult<Bool>):
  (file, got) = r
  match got:
    case Fail{e}:
      do IO<StoreResult<Bool>>:
        File.close(file)
        return MailStore.error(Bool, path, e)
    case Done{+size}:
      do IO<StoreResult<Bool>>:
        r : File & Result<&2, &2, String, Tail> <- MailStore.tail(1n+U32.to_nat(size), file, Tail{size, 0, False{}, False{}, Nil{}})
        MailStore.append.tail(r, path, size, text)

def MailStore.append.opened(r: Result<&1, &1, U32 & String, File>, +path: String, text: String) -> IO(StoreResult<Bool>):
  match r:
    case Fail{e}: IO.pure(StoreResult<Bool>, MailStore.error(Bool, path, e))
    case Done{file}:
      do IO<StoreResult<Bool>>:
        r : File & Result<&1, &1, U32 & String, U32> <- File.size(file)
        MailStore.append.size(r, path, text)

def MailStore.append.dir(r: Result<&1, &1, U32 & String, Unit>, +path: String, text: String) -> IO(StoreResult<Bool>):
  match r:
    case Fail{e}: IO.pure(StoreResult<Bool>, MailStore.error(Bool, path, e))
    case Done{_}:
      do IO<StoreResult<Bool>>:
        r : Result<&1, &1, U32 & String, File> <- MailFile.open(path, 1)
        MailStore.append.opened(r, path, text)

def MailStore.append.body(b: Maybe<&2, MB.Body>, +home: String) -> IO(StoreResult<Bool>):
  match b:
    case None{}: IO.pure(StoreResult<Bool>, StoreError{home ++ "/mail", "invalid body"})
    case Some{+b}:
      do IO<StoreResult<Bool>>:
        r : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(home ++ "/mail")
        MailStore.append.dir(r, MB.Body.path(home, ML.Conv.id(MB.Body.conv(b))), MB.Body.line(b))

# Success Bool says whether an incomplete final suffix was discarded.
# Only process-crash consistency: no fsync / power-loss durability promise.
def MailStore.append.raw(home: String, +body: MB.Body) -> IO(StoreResult<Bool>):
  MailStore.append.body(MB.Body.decode(MB.Body.encode(body)), home)

# Only offsets survive the traversal for duplicate detection. A retry reads
# its first row on demand and compares the decoded record exactly, no hashes.
type BodyOffset is Data:
  BodyOffset{at: U32, size: U32}

type Scan is Data:
  Scan{conv: String, before: String, limit: Nat, page: List<&2, MB.Body>, offsets: Map<&2, BodyOffset>, at: U32, stopped: Bool, recovery: Bool, box: List<&2, ML.Box>, changes: List<&2, M.Change>}

def Scan.box(s: Scan) -> ML.Box:
  match s:
    case Scan{_, _, _, _, _, _, _, _, b, _}: Maybe.default(&2, ML.Box, List.head(&2, ML.Box, b), ML.Box.empty())

def Scan.page(s: Scan) -> List<&2, MB.Body>:
  match s:
    case Scan{_, _, _, page, _, _, _, _, _, _}: page

def Scan.changes(s: Scan) -> List<&2, M.Change>:
  match s:
    case Scan{_, _, _, _, _, _, _, _, _, cs}: List.reverse(&2, M.Change, cs)

def Scan.offsets(s: Scan) -> Map<&2, BodyOffset>:
  match s:
    case Scan{_, _, _, _, offsets, _, _, _, _, _}: offsets

def Scan.offset.got(r: Map<&2, BodyOffset> & Maybe<&2, BodyOffset>) -> Maybe<&2, BodyOffset>:
  (_, v) = r
  v

def Scan.offset(s: Scan, id: String) -> Maybe<&2, BodyOffset>:
  Scan.offset.got(Map.pop(&2, BodyOffset, Scan.offsets(s), id))

def Scan.byte_count(text: String, acc: U32) -> U32:
  match text:
    case SNil{}: acc
    case SCon{Chr{+c}, t}:
      Scan.byte_count(t, U32.add(acc, Bool.pick(U32, U32.is_lt(c, 128), 1, Bool.pick(U32, U32.is_lt(c, 2048), 2, Bool.pick(U32, U32.is_lt(c, 65536), 3, 4)))))

def Scan.advance(s: Scan, size: U32) -> Scan:
  match s:
    case Scan{c, b, n, p, os, at, stop, rec, box, cs}: Scan{c, b, n, p, os, U32.add(at, size), stop, rec, box, cs}

def Scan.fold(cs: List<&2, M.Change>, b: ML.Box) -> ML.Box:
  match cs:
    case Nil{}: b
    case M.ConvSet{id, subject, kind, parts, project, tags, at} <> rest:
      Scan.fold(rest, ML.Box.conv_set(b, id, subject, kind, BT.Room.members.of(parts), project, BT.Room.members.of(tags), at))
    case M.MailPosted{id, conv, from, to, kind, ref, want, head, size, at} <> rest:
      Scan.fold(rest, ML.Box.mail(b, ML.Mail{id, conv, from, to, kind, ref, want, head, size, at}))
    case _ <> rest: Scan.fold(rest, b)

def Scan.recovered(+s: Scan, +body: MB.Body) -> Scan:
  match s:
    case Scan{c, b, n, p, os, at, stop, rec, box, cs}:
      +old = Maybe.default(&2, ML.Box, List.head(&2, ML.Box, box), ML.Box.empty())
      +added = MB.Body.recover(body, old)
      Scan{c, b, n, p, os, at, stop, rec, [Scan.fold(added, old)], List.append(&2, M.Change, List.reverse(&2, M.Change, added), cs)}

def Scan.collected(stop: Bool, +s: Scan, body: MB.Body) -> Scan:
  match stop s:
    case True{} _: s
    case False{} Scan{c, +before, +n, page, os, at, _, rec, box, cs}:
      Scan{c, before, n, List.take(&2, MB.Body, body <> page, n), os, at, False{}, rec, box, cs}

def Scan.stopped(s: Scan) -> Bool:
  match s:
    case Scan{_, _, _, _, _, _, stop, _, _, _}: stop

def Scan.cursor(hit: Bool, s: Scan, body: MB.Body) -> Scan:
  match hit s:
    case True{} Scan{c, b, n, page, os, at, _, rec, box, cs}: Scan{c, b, n, page, os, at, True{}, rec, box, cs}
    case False{} +s:
      Scan.collected(Scan.stopped(s), s, body)

def Scan.before(s: Scan) -> String:
  match s:
    case Scan{_, b, _, _, _, _, _, _, _, _}: b

def Scan.apply(rec: Bool, +s: Scan, +body: MB.Body) -> Scan:
  match rec:
    case True{}: Scan.recovered(s, body)
    case False{}:
      Scan.cursor(String.eq(Scan.before(s), ML.Mail.id(MB.Body.mail(body))), s, body)

def Scan.conv(s: Scan) -> String:
  match s:
    case Scan{c, _, _, _, _, _, _, _, _, _}: c

def Scan.unique(s: Scan, +body: MB.Body, +size: U32) -> Scan:
  match s:
    case Scan{c, b, n, page, os, +at, stop, +rec, box, cs}:
      Scan.apply(rec, Scan{c, b, n, page, Map.set(&2, BodyOffset, os, ML.Mail.id(MB.Body.mail(body)), BodyOffset{at, size}), U32.add(at, size), stop, rec, box, cs}, body)

def Scan.accept(valid: Bool, s: Scan, body: MB.Body, size: U32) -> Result<&2, &2, String, Scan>:
  match valid:
    case False{}: Fail{"record conversation does not match body filename"}
    case True{}: Done{Scan.unique(s, body, size)}

# Duplicate reads use bounded chunks as well; memory is at most one prior
# duplicate row in addition to the current row and requested page.
def MailStore.range.next(r: File & Result<&1, &1, U32 & String, List<&2, U32>>, +left: U32, at: U32, acc: List<&2, U32>, k: File -> U32 -> U32 -> List<&2, U32> -> IO(File & Result<&2, &2, String, List<&2, U32>>)) -> IO(File & Result<&2, &2, String, List<&2, U32>>):
  (file, got) = r
  match got:
    case Fail{(code, msg)}: IO.pure(File & Result<&2, &2, String, List<&2, U32>>, (file, Fail{U32.show(code) ++ ": " ++ msg}))
    case Done{Nil{}}: IO.pure(File & Result<&2, &2, String, List<&2, U32>>, (file, Fail{"unexpected EOF during duplicate verification"}))
    case Done{+bytes}:
      +n = U32.from_nat(List.length(&2, U32, bytes))
      k(file, U32.sub(left, n), U32.add(at, n), List.append(&2, U32, List.reverse(&2, U32, bytes), acc))

def MailStore.range(fuel: Nat, file: File, left: U32, +at: U32, acc: List<&2, U32>) -> IO(File & Result<&2, &2, String, List<&2, U32>>):
  match fuel left:
    case _ 0: IO.pure(File & Result<&2, &2, String, List<&2, U32>>, (file, Done{List.reverse(&2, U32, acc)}))
    case 0n _: IO.pure(File & Result<&2, &2, String, List<&2, U32>>, (file, Fail{"duplicate range exhausted"}))
    case 1n+k +left:
      do IO<File & Result<&2, &2, String, List<&2, U32>>>:
        r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_at(file, at, Bool.pick(U32, U32.is_lt(left, 16384), left, 16384))
        MailStore.range.next(r, left, at, acc, f => l => a => bs => MailStore.range(k, f, l, a, bs))

def MailStore.duplicate.equal(equal: Bool, s: Scan, size: U32) -> Result<&2, &2, String, Scan>:
  match equal:
    case True{}: Done{Scan.advance(s, size)}
    case False{}: Fail{"conflicting duplicate mail id"}

def MailStore.duplicate.body(prior: Maybe<&2, MB.Body>, s: Scan, b: MB.Body, size: U32) -> Result<&2, &2, String, Scan>:
  match prior:
    case None{}: Fail{"invalid original duplicate record"}
    case Some{old}: MailStore.duplicate.equal(MB.Body.same(MB.Body.line(old), MB.Body.line(b)), s, size)

def MailStore.duplicate.got(r: File & Result<&2, &2, String, List<&2, U32>>, s: Scan, b: MB.Body, size: U32) -> IO(File & Result<&2, &2, String, Scan>):
  (file, got) = r
  match got:
    case Fail{e}: IO.pure(File & Result<&2, &2, String, Scan>, (file, Fail{e}))
    case Done{bs}: IO.pure(File & Result<&2, &2, String, Scan>, (file, MailStore.duplicate.body(MB.Body.parse(U.Utf8.decode(bs)), s, b, size)))

def MailStore.record.offset(prior: Maybe<&2, BodyOffset>, file: File, +s: Scan, +b: MB.Body, size: U32) -> IO(File & Result<&2, &2, String, Scan>):
  match prior:
    case None{}: IO.pure(File & Result<&2, &2, String, Scan>, (file, Scan.accept(String.eq(Scan.conv(s), ML.Conv.id(MB.Body.conv(b))), s, b, size)))
    case Some{BodyOffset{at, +len}}:
      do IO<File & Result<&2, &2, String, Scan>>:
        r : File & Result<&2, &2, String, List<&2, U32>> <- MailStore.range(1n+U32.to_nat(len), file, len, at, Nil{})
        MailStore.duplicate.got(r, s, b, size)

def MailStore.record.body(b: Maybe<&2, MB.Body>, file: File, +s: Scan, size: U32) -> IO(File & Result<&2, &2, String, Scan>):
  match b:
    case None{}: IO.pure(File & Result<&2, &2, String, Scan>, (file, Fail{"malformed completed body record (JSON, version or schema)"}))
    case Some{+b}: MailStore.record.offset(Scan.offset(s, ML.Mail.id(MB.Body.mail(b))), file, s, b, size)

def MailStore.lines.next(r: File & Result<&2, &2, String, Scan>, k: File -> Scan -> IO(File & Result<&2, &2, String, Scan>)) -> IO(File & Result<&2, &2, String, Scan>):
  (file, got) = r
  match got:
    case Fail{e}: IO.pure(File & Result<&2, &2, String, Scan>, (file, Fail{e}))
    case Done{s}: k(file, s)

def MailStore.lines(lines: List<&2, String>, file: File, s: Scan) -> IO(File & Result<&2, &2, String, Scan>):
  match lines:
    case Nil{}: IO.pure(File & Result<&2, &2, String, Scan>, (file, Done{s}))
    case +h <> t:
      do IO<File & Result<&2, &2, String, Scan>>:
        r : File & Result<&2, &2, String, Scan> <- MailStore.record.body(MB.Body.parse(h), file, s, U32.add(Scan.byte_count(h, 0), 1))
        MailStore.lines.next(r, f => st => MailStore.lines(t, f, st))

def MailStore.stream.lines(r: File & Result<&2, &2, String, Scan>, carry: U.Lines, k: File -> U.Lines -> Scan -> IO(File & Result<&2, &2, String, Scan>)) -> IO(File & Result<&2, &2, String, Scan>):
  (file, got) = r
  match got:
    case Fail{e}: IO.pure(File & Result<&2, &2, String, Scan>, (file, Fail{e}))
    case Done{s}: k(file, carry, s)

def MailStore.stream.feed(pair: U.Lines & List<&2, String>, file: File, s: Scan, k: File -> U.Lines -> Scan -> IO(File & Result<&2, &2, String, Scan>)) -> IO(File & Result<&2, &2, String, Scan>):
  (carry, lines) = pair
  do IO<File & Result<&2, &2, String, Scan>>:
    r : File & Result<&2, &2, String, Scan> <- MailStore.lines(lines, file, s)
    MailStore.stream.lines(r, carry, k)

def MailStore.stream.got(r: File & Result<&1, &1, U32 & String, List<&2, U32>>, carry: U.Lines, s: Scan, k: File -> U.Lines -> Scan -> IO(File & Result<&2, &2, String, Scan>)) -> IO(File & Result<&2, &2, String, Scan>):
  (file, got) = r
  match got:
    case Fail{(code, msg)}: IO.pure(File & Result<&2, &2, String, Scan>, (file, Fail{U32.show(code) ++ ": " ++ msg}))
    case Done{Nil{}}: IO.pure(File & Result<&2, &2, String, Scan>, (file, Done{s}))
    case Done{bs}: MailStore.stream.feed(U.Lines.feed(carry, bs), file, s, k)

def MailStore.stream(fuel: Nat, file: File, carry: U.Lines, s: Scan) -> IO(File & Result<&2, &2, String, Scan>):
  match fuel:
    case 0n: IO.pure(File & Result<&2, &2, String, Scan>, (file, Fail{"stream exhausted"}))
    case 1n+k:
      do IO<File & Result<&2, &2, String, Scan>>:
        r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(file, 16384)
        MailStore.stream.got(r, carry, s, f => c => st => MailStore.stream(k, f, c, st))

def MailStore.scan.closed(r: Result<&2, &2, String, Scan>, path: String) -> StoreResult<Scan>:
  match r:
    case Fail{e}: StoreError{path, e}
    case Done{s}: Stored{s}

def MailStore.scan.done(r: File & Result<&2, &2, String, Scan>, path: String) -> IO(StoreResult<Scan>):
  (file, got) = r
  do IO<StoreResult<Scan>>:
    File.close(file)
    return MailStore.scan.closed(got, path)

def MailStore.scan.size(r: File & Result<&1, &1, U32 & String, U32>, path: String, s: Scan) -> IO(StoreResult<Scan>):
  (file, got) = r
  match got:
    case Fail{e}:
      do IO<StoreResult<Scan>>:
        File.close(file)
        return MailStore.error(Scan, path, e)
    case Done{size}:
      do IO<StoreResult<Scan>>:
        r : File & Result<&2, &2, String, Scan> <- MailStore.stream(2n+U32.to_nat(size), file, U.Lines.new(), s)
        MailStore.scan.done(r, path)

def MailStore.scan.opened(r: Result<&1, &1, U32 & String, File>, path: String, s: Scan) -> IO(StoreResult<Scan>):
  match r:
    case Fail{e}: IO.pure(StoreResult<Scan>, MailStore.open_error(Scan, path, e))
    case Done{file}:
      do IO<StoreResult<Scan>>:
        r : File & Result<&1, &1, U32 & String, U32> <- File.size(file)
        MailStore.scan.size(r, path, s)

def MailStore.scan(+path: String, s: Scan) -> IO(StoreResult<Scan>):
  do IO<StoreResult<Scan>>:
    r : Result<&1, &1, U32 & String, File> <- MailFile.open(path, 0)
    MailStore.scan.opened(r, path, s)

def MailStore.read.cursor(ok: Bool, path: String, page: List<&2, MB.Body>) -> StoreResult<List<&2, MB.Body>>:
  match ok:
    case True{}: Stored{page}
    case False{}: StoreError{path, "unknown mail cursor"}

def MailStore.read.result(r: StoreResult<Scan>, path: String) -> StoreResult<List<&2, MB.Body>>:
  match r:
    case StoreError{p, e}: StoreError{p, e}
    case Missing{p}: Missing{p}
    case Stored{+s}:
      MailStore.read.cursor(Bool.or(String.is_empty(Scan.before(s)), Scan.stopped(s)), path, Scan.page(s))

# n=0 does no file IO; page limit is 200. Complete rows are validated even
# beyond the cursor; a single final unterminated suffix is ignored.
def MailStore.read.stream(+home: String, +conv: String, before: String, +n: Nat) -> IO(StoreResult<List<&2, MB.Body>>):
  match n:
    case 0n: IO.pure(StoreResult<List<&2, MB.Body>>, Stored{Nil{}})
    case 1n+k:
      do IO<StoreResult<List<&2, MB.Body>>>:
        +path : String = MB.Body.path(home, conv)
        r : StoreResult<Scan> <- MailStore.scan(path, Scan{conv, before, Bool.pick(Nat, Nat.is_lt(k, 200n), 1n+k, 200n), Nil{}, Map.new(&2, BodyOffset), 0, False{}, False{}, Nil{}, Nil{}})
        return MailStore.read.result(r, path)

# Recovery visits only encoded, direct child .jsonl files, one at a time.
# No bodies survive into the returned list; the working Box deduplicates
# headers before the lead's eventual event commit.
def MailStore.recover.file(+name: String) -> Bool:
  Bool.and(String.ends_with(name, ".jsonl"), Bool.not(String.contains(name, "/")))

def MailStore.recover.next(r: StoreResult<Scan>, k: ML.Box -> List<&2, M.Change> -> IO(StoreResult<List<&2, M.Change>>)) -> IO(StoreResult<List<&2, M.Change>>):
  match r:
    case StoreError{p, e}: IO.pure(StoreResult<List<&2, M.Change>>, StoreError{p, e})
    case Missing{p}: IO.pure(StoreResult<List<&2, M.Change>>, StoreError{p, "body file vanished during recovery"})
    case Stored{+s}: k(Scan.box(s), Scan.changes(s))

def MailStore.recover.one(yes: Bool, +home: String, +name: String, box: ML.Box, changes: List<&2, M.Change>, k: ML.Box -> List<&2, M.Change> -> IO(StoreResult<List<&2, M.Change>>)) -> IO(StoreResult<List<&2, M.Change>>):
  match yes:
    case False{}: k(box, changes)
    case True{}:
      do IO<StoreResult<List<&2, M.Change>>>:
        r : StoreResult<Scan> <- MailStore.scan(home ++ "/mail/" ++ name, Scan{R.Ref.unpct(String.take(name, Nat.sub(String.length(name), 6n))), "", 0n, Nil{}, Map.new(&2, BodyOffset), 0, False{}, True{}, [box], List.reverse(&2, M.Change, changes)})
        MailStore.recover.next(r, k)

def MailStore.recover.files(names: List<String>, +home: String, box: ML.Box, changes: List<&2, M.Change>) -> IO(StoreResult<List<&2, M.Change>>):
  match names:
    case Nil{}: IO.pure(StoreResult<List<&2, M.Change>>, Stored{changes})
    case +h <> t:
      MailStore.recover.one(MailStore.recover.file(h), home, h, box, changes, b => cs => MailStore.recover.files(t, home, b, cs))

def MailStore.recover.missing(absent: Bool, home: String, e: U32 & String) -> StoreResult<List<&2, M.Change>>:
  match absent:
    case True{}: Stored{Nil{}}
    case False{}: MailStore.error(List<&2, M.Change>, home ++ "/mail", e)

def MailStore.recover.list(r: Result<&1, &1, U32 & String, List<String>>, +home: String, box: ML.Box) -> IO(StoreResult<List<&2, M.Change>>):
  match r:
    case Fail{(+code, msg)}:
      IO.pure(StoreResult<List<&2, M.Change>>, MailStore.recover.missing(U32.is_eq(code, 2), home, (code, msg)))
    case Done{names}: MailStore.recover.files(names, home, box, Nil{})

def MailStore.recover(+home: String, box: ML.Box) -> IO(StoreResult<List<&2, M.Change>>):
  do IO<StoreResult<List<&2, M.Change>>>:
    r : Result<&1, &1, U32 & String, List<String>> <- H.FS.list(home ++ "/mail")
    MailStore.recover.list(r, home, box)

# ---- mail index ----
# One 64-byte ASCII record per physical body row. A 64 KiB header contains
# magic + 4095 checked hash-bucket heads. Hashes only select candidates: IDs and retry
# bodies are always compared exactly. The first occurrence owns arrival order.
# Header writes follow record writes; an interrupted update fails the tail's
# bucket-head check. The journal is authoritative; indexes are disposable.

def MailFile.text(+path: String, +at: U32, +size: U32) -> IO(Result<&1, &1, U32 & String, String>):
  import "./effects/mailstore.c"
  import "./effects/mailstore.js"

def MailFile.patch(+path: String, +at: U32, +text: String, +truncate: U32) -> IO(Result<&1, &1, U32 & String, Unit>):
  import "./effects/mailstore.c"
  import "./effects/mailstore.js"

def MI.then(-A: Data, -B: Data, +r: StoreResult<A>, k: A -> IO(StoreResult<B>)) -> IO(StoreResult<B>):
  match r:
    case Stored{+v}: k(v)
    case Missing{+p}: IO.pure(StoreResult<B>, Missing{p})
    case StoreError{+p, +e}: IO.pure(StoreResult<B>, StoreError{p, e})

def MI.bind(-A: Data, -B: Data, op: IO(StoreResult<A>), k: A -> IO(StoreResult<B>)) -> IO(StoreResult<B>):
  IO.bind(StoreResult<A>, StoreResult<B>, op, r => MI.then(A, B, r, k))

def MI.pure(-A: Data, +v: A) -> IO(StoreResult<A>):
  IO.pure(StoreResult<A>, Stored{v})

def MI.bad(-A: Data, +p: String, +e: String) -> IO(StoreResult<A>):
  IO.pure(StoreResult<A>, StoreError{p, e})

def MI.fx(-A: Data, r: Result<&1, &1, U32 & String, A>, +p: String) -> StoreResult<A>:
  match r:
    case Done{+v}: Stored{v}
    case Fail{e}: MailStore.open_error(A, p, e)

def MI.read(+p: String, +at: U32, +size: U32) -> IO(StoreResult<String>):
  do IO<StoreResult<String>>:
    r : Result<&1, &1, U32 & String, String> <- MailFile.text(p, at, size)
    return MI.fx(String, r, p)

def MI.write(+p: String, +at: U32, +text: String, +truncate: U32) -> IO(StoreResult<Unit>):
  do IO<StoreResult<Unit>>:
    r : Result<&1, &1, U32 & String, Unit> <- MailFile.patch(p, at, text, truncate)
    return MI.fx(Unit, r, p)

def MI.sized(r: File & Result<&1, &1, U32 & String, U32>, +p: String) -> IO(StoreResult<U32>):
  (f, v) = r
  do IO<StoreResult<U32>>:
    File.close(f)
    return MI.fx(U32, v, p)

def MI.opened(r: Result<&1, &1, U32 & String, File>, +p: String) -> IO(StoreResult<U32>):
  match r:
    case Fail{e}: IO.pure(StoreResult<U32>, MailStore.open_error(U32, p, e))
    case Done{f}:
      do IO<StoreResult<U32>>:
        r : File & Result<&1, &1, U32 & String, U32> <- File.size(f)
        MI.sized(r, p)

def MI.size(+p: String) -> IO(StoreResult<U32>):
  do IO<StoreResult<U32>>:
    r : Result<&1, &1, U32 & String, File> <- MailFile.open(p, 0)
    MI.opened(r, p)

def MI.path(+home: String, +conv: String) -> String:
  home ++ "/mail/" ++ R.Ref.pct(conv) ++ ".idx"

def MI.hash(+s: String, +h: U32) -> U32:
  match s:
    case SNil{}: h
    case SCon{Chr{+c}, +t}: MI.hash(t, (U32.xor(h, c) * 16777619 : U32))

def MI.key(+s: String) -> U32:
  MI.hash(s, 2166136261)

def MI.bucket(+h: U32) -> U32:
  U32.add(8, U32.mul((h % 4095 : U32), 16))

def MI.hex.go(+n: Nat, +x: U32, +s: String) -> String:
  match n:
    case 0n: s
    case 1n+k: MI.hex.go(k, U32.shrn(x, 4n), SCon{Chr{U32.add(65, U32.and(x, 15))}, s})

def MI.hex(+x: U32) -> String:
  MI.hex.go(8n, x, "")

def MI.num(+s: String, +acc: U32) -> Maybe<&2, U32>:
  match s:
    case SNil{}: Some{acc}
    case SCon{Chr{+c}, +t}:
      do Maybe<&2, U32>:
        d : U32 <- MB.Body.when(U32, Bool.and(U32.is_ge(c, 65), U32.is_le(c, 80)), U32.sub(c, 65))
        MI.num(t, U32.or(U32.shln(acc, 4n), d))

# checksum covers the record's own position too, so relocation is detected.
type MIndexRow is Data:
  MIndexRow{at: U32, size: U32, hash: U32, prev: U32, first: U32, poison: U32, pos: U32}

def MI.row.text(+r: MIndexRow) -> String:
  match r:
    case MIndexRow{+at, +size, +hash, +prev, +first, +poison, +pos}:
      +s = MI.hex(at) ++ MI.hex(size) ++ MI.hex(hash) ++ MI.hex(prev) ++ MI.hex(first) ++ MI.hex(poison) ++ MI.hex(pos)
      s ++ MI.hex(MI.key(s))

def MI.row.parse(+s: String) -> Maybe<&2, MIndexRow>:
  do Maybe<&2, MIndexRow>:
    valid : Unit <- MB.Body.when(Unit, Nat.is_eq(String.length(s), 64n), Unit{})
    at : U32 <- MI.num(String.take(s, 8n), 0)
    size : U32 <- MI.num(String.take(String.drop(s, 8n), 8n), 0)
    hash : U32 <- MI.num(String.take(String.drop(s, 16n), 8n), 0)
    prev : U32 <- MI.num(String.take(String.drop(s, 24n), 8n), 0)
    first : U32 <- MI.num(String.take(String.drop(s, 32n), 8n), 0)
    poison : U32 <- MI.num(String.take(String.drop(s, 40n), 8n), 0)
    pos : U32 <- MI.num(String.take(String.drop(s, 48n), 8n), 0)
    check : Unit <- MB.Body.when(Unit, String.eq(String.drop(s, 56n), MI.hex(MI.key(String.take(s, 56n)))), Unit{})
    return MIndexRow{at, size, hash, prev, first, poison, pos}

def MI.row.ok(+r: MIndexRow, +pos: U32) -> Bool:
  match r:
    case MIndexRow{+at, +size, _, +prev, +first, +poison, +p}:
      Bool.and(U32.is_eq(p, pos), Bool.and(U32.is_gt(size, 0), Bool.and(U32.is_le(size, U32.sub(4294967295, at)), Bool.and(Bool.or(U32.is_eq(prev, 0), Bool.and(U32.is_ge(prev, 65536), Bool.and(U32.is_lt(prev, pos), U32.is_eq((U32.sub(prev, 65536) % 64 : U32), 0)))), Bool.and(U32.is_ge(first, 65536), Bool.and(U32.is_le(first, pos), Bool.and(U32.is_eq((U32.sub(first, 65536) % 64 : U32), 0), U32.is_le(poison, 1))))))))

def MI.row.checked(+ok: Bool, +r: MIndexRow, +p: String) -> IO(StoreResult<MIndexRow>):
  match ok:
    case True{}: MI.pure(MIndexRow, r)
    case False{}: MI.bad(MIndexRow, p, "invalid mail index bounds")

def MI.row.got(+r: Maybe<&2, MIndexRow>, +p: String, +pos: U32) -> IO(StoreResult<MIndexRow>):
  match r:
    case None{}: MI.bad(MIndexRow, p, "invalid mail index record")
    case Some{+r}:
      MI.row.checked(MI.row.ok(r, pos), r, p)

def MI.row(+p: String, +pos: U32) -> IO(StoreResult<MIndexRow>):
  MI.bind(String, MIndexRow, MI.read(p, pos, 64), s => MI.row.got(MI.row.parse(s), p, pos))

def MI.number(+r: Maybe<&2, U32>, +p: String) -> IO(StoreResult<U32>):
  match r:
    case Some{+v}: MI.pure(U32, v)
    case None{}: MI.bad(U32, p, "invalid mail index pointer")

def MI.body.checked(+ok: Bool, +b: MB.Body, +p: String) -> IO(StoreResult<MB.Body>):
  match ok:
    case True{}: MI.pure(MB.Body, b)
    case False{}: MI.bad(MB.Body, p, "indexed body id or conversation mismatch")

def MI.body.parsed(+r: Maybe<&2, MB.Body>, +p: String, +conv: String, +hash: U32) -> IO(StoreResult<MB.Body>):
  match r:
    case None{}: MI.bad(MB.Body, p, "malformed indexed body record")
    case Some{+b}: MI.body.checked(Bool.and(String.eq(ML.Conv.id(MB.Body.conv(b)), conv), U32.is_eq(MI.key(ML.Mail.id(MB.Body.mail(b))), hash)), b, p)

def MI.body.line(s: String, +p: String, +conv: String, +hash: U32) -> IO(StoreResult<MB.Body>):
  # Require the complete row boundary as well as valid JSON/schema.
  MI.body.parsed(MB.Body.parse_line(s), p, conv, hash)

def MI.body(+path: String, +conv: String, +r: MIndexRow) -> IO(StoreResult<MB.Body>):
  match r:
    case MIndexRow{+at, +size, +hash, _, _, _, _}:
      MI.bind(String, MB.Body, MI.read(path, at, size), s => MI.body.line(s, path, conv, hash))

# A lookup walks only one bucket chain, newest first. The decreasing position
# check bounds corrupt chains; full IDs, not fingerprints, decide identity.
def MI.find.more(+same: Bool, +found: U32, +prev: U32, k: U32 -> IO(StoreResult<U32>)) -> IO(StoreResult<U32>):
  match same:
    case True{}: MI.pure(U32, found)
    case False{}: k(prev)

def MI.find.hash(+hit: Bool, +r: MIndexRow, +path: String, +conv: String, +id: String, k: U32 -> IO(StoreResult<U32>)) -> IO(StoreResult<U32>):
  match hit r:
    case False{} MIndexRow{_, _, _, +prev, _, _, _}: k(prev)
    case True{} MIndexRow{+at, +size, +hash, +prev, +first, +poison, +pos}:
      MI.bind(MB.Body, U32, MI.body(path, conv, MIndexRow{at, size, hash, prev, first, poison, pos}), b => MI.find.more(String.eq(ML.Mail.id(MB.Body.mail(b)), id), first, prev, k))

def MI.find.bound(+ok: Bool, +idx: String, +pos: U32, k: MIndexRow -> IO(StoreResult<U32>)) -> IO(StoreResult<U32>):
  match ok:
    case False{}: MI.bad(U32, idx, "invalid mail index chain")
    case True{}: MI.bind(MIndexRow, U32, MI.row(idx, pos), k)

type MIndex is Data:
  MIndex{end: U32, cut: U32, poison: U32}

# Repair is one streaming pass through the source journal. Only index data,
# the current line and a single collision/retry body are resident.
def MI.check.cover(+ok: Bool, +idx: String, +st: MIndex) -> IO(StoreResult<MIndex>):
  match ok:
    case True{}: MI.pure(MIndex, st)
    case False{}: MI.bad(MIndex, idx, "mail index does not cover journal")

# Normal reads check magic, record alignment, journal coverage and the final
# bucket update. Each selected row is checked again before it is returned.
def MI.check.tail(+head: U32, +r: MIndexRow, +end: U32, +size: U32, +idx: String) -> IO(StoreResult<MIndex>):
  match r:
    case MIndexRow{+at, +len, _, _, _, +poison, +pos}:
      MI.check.cover(Bool.and(U32.is_eq(head, pos), U32.is_eq(U32.add(at, len), size)), idx, MIndex{end, size, poison})

# Reads take only the requested unique rows; retry rows point at their first
# occurrence and are skipped. Cursors compare the exact decoded mail ID.
def MI.page.end(+waiting: Bool, +path: String, +acc: List<&2, MB.Body>) -> IO(StoreResult<List<&2, MB.Body>>):
  match waiting:
    case True{}: MI.bad(List<&2, MB.Body>, path, "unknown mail cursor")
    case False{}: MI.pure(List<&2, MB.Body>, List.reverse(&2, MB.Body, acc))

# Byte chunks keep UTF-8 framing intact when repair splits a code point.
def MI.bytes.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>, +p: String) -> IO(StoreResult<List<&2, U32>>):
  (f, got) = r
  do IO<StoreResult<List<&2, U32>>>:
    File.close(f)
    return MI.fx(List<&2, U32>, got, p)

def MI.bytes.opened(r: Result<&1, &1, U32 & String, File>, +p: String, +at: U32, +size: U32) -> IO(StoreResult<List<&2, U32>>):
  match r:
    case Fail{e}: IO.pure(StoreResult<List<&2, U32>>, MailStore.open_error(List<&2, U32>, p, e))
    case Done{f}:
      do IO<StoreResult<List<&2, U32>>>:
        r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_at(f, at, size)
        MI.bytes.done(r, p)

def MI.bytes(+p: String, +at: U32, +size: U32) -> IO(StoreResult<List<&2, U32>>):
  do IO<StoreResult<List<&2, U32>>>:
    r : Result<&1, &1, U32 & String, File> <- MailFile.open(p, 0)
    MI.bytes.opened(r, p, at, size)

# Pointers are stored with their complement: zeroed and torn slots are invalid.
def MI.pointer(+pos: U32) -> String:
  MI.hex(pos) ++ MI.hex(U32.xor(pos, 4294967295))

def MI.put.write(+idx: String, +at: U32, +size: U32, +hash: U32, +head: U32, +first: U32, +poison: U32, +pos: U32) -> IO(StoreResult<MIndex>):
  MI.bind(Unit, MIndex, MI.write(idx, pos, MI.row.text(MIndexRow{at, size, hash, head, first, poison, pos}), 1), u =>
    MI.bind(Unit, MIndex, MI.write(idx, MI.bucket(hash), MI.pointer(pos), 0), u => MI.pure(MIndex, MIndex{U32.add(pos, 64), U32.add(at, size), poison})))

def MI.put.duplicate(+equal: Bool, +idx: String, +at: U32, +size: U32, +hash: U32, +head: U32, +first: U32, +poison: U32, +pos: U32) -> IO(StoreResult<MIndex>):
  match equal:
    case True{}: MI.put.write(idx, at, size, hash, head, first, poison, pos)
    case False{}: MI.put.write(idx, at, size, hash, head, first, 1, pos)

def MI.put.found(+first: U32, +idx: String, +path: String, +conv: String, +b: MB.Body, +at: U32, +size: U32, +hash: U32, +head: U32, +poison: U32, +pos: U32) -> IO(StoreResult<MIndex>):
  match first:
    case 0: MI.put.write(idx, at, size, hash, head, pos, poison, pos)
    case +first:
      MI.bind(MIndexRow, MIndex, MI.row(idx, first), r =>
        MI.bind(MB.Body, MIndex, MI.body(path, conv, r), old => MI.put.duplicate(MB.Body.same(MB.Body.line(old), MB.Body.line(b)), idx, at, size, hash, head, first, poison, pos)))

def MI.head.parse(+s: String) -> Maybe<&2, U32>:
  do Maybe<&2, U32>:
    +pos : U32 <- MI.num(String.take(s, 8n), 0)
    check : Unit <- MB.Body.when(Unit, String.eq(s, MI.pointer(pos)), Unit{})
    return pos

def MI.head.got(r: Maybe<&2, U32>, p: String) -> IO(StoreResult<U32>):
  MI.number(r, p)

def MI.head(+p: String, +hash: U32) -> IO(StoreResult<U32>):
  MI.bind(String, U32, MI.read(p, MI.bucket(hash), 16), s => MI.head.got(MI.head.parse(s), p))

def MI.check.last(+r: MIndexRow, +idx: String, +end: U32, +size: U32) -> IO(StoreResult<MIndex>):
  match r:
    case MIndexRow{+at, +len, +hash, +prev, +first, +poison, +pos}:
      MI.bind(U32, MIndex, MI.head(idx, hash), h => MI.check.tail(h, MIndexRow{at, len, hash, prev, first, poison, pos}, end, size, idx))

def MI.check.magic(+ok: Bool, +idx: String, +end: U32, +size: U32) -> IO(StoreResult<MIndex>):
  match ok end:
    case False{} _: MI.bad(MIndex, idx, "invalid mail index header or size")
    case True{} 65536: MI.check.cover(U32.is_eq(size, 0), idx, MIndex{65536, 0, 0})
    case True{} _:
      MI.bind(MIndexRow, MIndex, MI.row(idx, U32.sub(end, 64)), r => MI.check.last(r, idx, end, size))

def MI.check(+idx: String, +size: U32) -> IO(StoreResult<MIndex>):
  MI.bind(U32, MIndex, MI.size(idx), +end =>
    MI.bind(String, MIndex, MI.read(idx, 0, 8), magic => MI.check.magic(Bool.and(String.eq(magic, "MAILIDX1"), Bool.and(U32.is_ge(end, 65536), U32.is_eq((U32.sub(end, 65536) % 64 : U32), 0))), idx, end, size)))

def MI.header(n: Nat, acc: String) -> String:
  match n:
    case 0n: "MAILIDX1" ++ acc
    case 1n+k: MI.header(k, MI.pointer(0) ++ acc)

def MI.find.slot(ok: Bool, hit: Bool, r: MIndexRow, path: String, conv: String, id: String, k: U32 -> IO(StoreResult<U32>)) -> IO(StoreResult<U32>):
  match ok:
    case True{}: MI.find.hash(hit, r, path, conv, id, k)
    case False{}: MI.bad(U32, path, "mail index bucket mismatch")

def MI.find.row(+r: MIndexRow, +path: String, +conv: String, +id: String, +hash: U32, k: U32 -> IO(StoreResult<U32>)) -> IO(StoreResult<U32>):
  match r:
    case MIndexRow{+at, +size, +rh, +prev, +first, +poison, +pos}:
      MI.find.slot(U32.is_eq(MI.bucket(rh), MI.bucket(hash)), U32.is_eq(rh, hash), MIndexRow{at, size, rh, prev, first, poison, pos}, path, conv, id, k)

def MI.find(+fuel: Nat, +idx: String, +path: String, +conv: String, +id: String, +hash: U32, +pos: U32, +bound: U32) -> IO(StoreResult<U32>):
  match fuel pos:
    case _ 0: MI.pure(U32, 0)
    case 0n _: MI.bad(U32, idx, "mail index chain exhausted")
    case 1n+k _:
      MI.find.bound(U32.is_lt(pos, bound), idx, pos, r => MI.find.row(r, path, conv, id, hash, p => MI.find(k, idx, path, conv, id, hash, p, pos)))

def MI.put.space(+ok: Bool, +idx: String, +path: String, +conv: String, +b: MB.Body, +at: U32, +size: U32, +pos: U32, +poison: U32) -> IO(StoreResult<MIndex>):
  match ok:
    case False{}: MI.bad(MIndex, idx, "mail index or body exceeds U32 (4 GiB) limit")
    case True{}:
      +id = ML.Mail.id(MB.Body.mail(b))
      +hash = MI.key(id)
      MI.bind(U32, MIndex, MI.head(idx, hash), +head =>
        MI.bind(U32, MIndex, MI.find(1n+U32.to_nat(pos), idx, path, conv, id, hash, head, pos), first => MI.put.found(first, idx, path, conv, b, at, size, hash, head, poison, pos)))

def MI.put(+idx: String, +path: String, +conv: String, +b: MB.Body, +size: U32, +st: MIndex) -> IO(StoreResult<MIndex>):
  match st:
    case MIndex{+pos, +at, +poison}:
      MI.put.space(Bool.and(U32.is_le(pos, 4294967231), U32.is_le(size, U32.sub(4294967295, at))), idx, path, conv, b, at, size, pos, poison)

def MI.build.conv(+ok: Bool, +idx: String, +path: String, +conv: String, +b: MB.Body, +size: U32, +st: MIndex) -> IO(StoreResult<MIndex>):
  match ok:
    case False{}: MI.bad(MIndex, path, "record conversation does not match body filename")
    case True{}: MI.put(idx, path, conv, b, size, st)

def MI.build.body(+b: Maybe<&2, MB.Body>, +idx: String, +path: String, +conv: String, +size: U32, +st: MIndex) -> IO(StoreResult<MIndex>):
  match b:
    case None{}: MI.bad(MIndex, path, "malformed completed body record during index repair")
    case Some{+b}: MI.build.conv(String.eq(ML.Conv.id(MB.Body.conv(b)), conv), idx, path, conv, b, size, st)

def MI.build.lines(+lines: List<&2, String>, +idx: String, +path: String, +conv: String, +st: MIndex) -> IO(StoreResult<MIndex>):
  match lines:
    case Nil{}: MI.pure(MIndex, st)
    case +h <> t:
      MI.bind(MIndex, MIndex, MI.build.body(MB.Body.parse(h), idx, path, conv, U32.add(Scan.byte_count(h, 0), 1), st), s => MI.build.lines(t, idx, path, conv, s))

def MI.build.chunk(pair: U.Lines & List<&2, String>, +idx: String, +path: String, +conv: String, +st: MIndex, k: U.Lines -> MIndex -> IO(StoreResult<MIndex>)) -> IO(StoreResult<MIndex>):
  (carry, lines) = pair
  MI.bind(MIndex, MIndex, MI.build.lines(lines, idx, path, conv, st), s => k(carry, s))

def MI.build.next(+done: Bool, +idx: String, +path: String, +conv: String, +at: U32, +size: U32, +carry: U.Lines, +st: MIndex, k: U.Lines -> MIndex -> IO(StoreResult<MIndex>)) -> IO(StoreResult<MIndex>):
  match done:
    case True{}: MI.pure(MIndex, st)
    case False{}:
      MI.bind(List<&2, U32>, MIndex, MI.bytes(path, at, Bool.pick(U32, U32.is_lt(U32.sub(size, at), 16384), U32.sub(size, at), 16384)), bs => MI.build.chunk(U.Lines.feed(carry, bs), idx, path, conv, st, k))

def MI.build.scan(+fuel: Nat, +idx: String, +path: String, +conv: String, +at: U32, +size: U32, +carry: U.Lines, +st: MIndex) -> IO(StoreResult<MIndex>):
  match fuel:
    case 0n: MI.bad(MIndex, path, "mail index repair exhausted")
    case 1n+k:
      MI.build.next(U32.is_eq(at, size), idx, path, conv, at, size, carry, st, c => s => MI.build.scan(k, idx, path, conv, U32.add(at, Bool.pick(U32, U32.is_lt(U32.sub(size, at), 16384), U32.sub(size, at), 16384)), size, c, s))

def MI.build(+idx: String, +path: String, +conv: String, +size: U32) -> IO(StoreResult<MIndex>):
  MI.bind(Unit, MIndex, MI.write(idx, 0, MI.header(4095n, "AAAAAAAA"), 1), u => MI.build.scan(2n+U32.to_nat(size), idx, path, conv, 0, size, U.Lines.new(), MIndex{65536, 0, 0}))

def MI.ensure.checked(+r: StoreResult<MIndex>, +idx: String, +path: String, +conv: String, +size: U32) -> IO(StoreResult<MIndex>):
  match r:
    case Stored{+st}: MI.pure(MIndex, st)
    case _: MI.build(idx, path, conv, size)

def MI.ensure(+idx: String, +path: String, +conv: String, +size: U32) -> IO(StoreResult<MIndex>):
  do IO<StoreResult<MIndex>>:
    r : StoreResult<MIndex> <- MI.check(idx, size)
    MI.ensure.checked(r, idx, path, conv, size)

# Preserve append's public Bool and crash ordering: source first, index second.
# A missing journal is initialized by raw append and indexed by one repair pass.
def MI.append.new(+r: StoreResult<Bool>, +idx: String, +path: String, +conv: String) -> IO(StoreResult<Bool>):
  MI.then(Bool, Bool, r, repaired => MI.bind(U32, Bool, MI.size(path), size => MI.bind(MIndex, Bool, MI.build(idx, path, conv, size), st => MI.pure(Bool, repaired))))

# Decode at most 64 index records in a single bounded positional read.
def MI.rows.add(r: Maybe<&2, MIndexRow>, pos: U32, acc: List<&2, MIndexRow>, k: List<&2, MIndexRow> -> Maybe<&2, List<&2, MIndexRow>>) -> Maybe<&2, List<&2, MIndexRow>>:
  match r:
    case None{}: None{}
    case Some{+r}:
      do Maybe<&2, List<&2, MIndexRow>>:
        ok : Unit <- MB.Body.when(Unit, MI.row.ok(r, pos), Unit{})
        k(r <> acc)

def MI.rows(fuel: Nat, +s: String, +pos: U32, acc: List<&2, MIndexRow>) -> Maybe<&2, List<&2, MIndexRow>>:
  match fuel s:
    case _ SNil{}: Some{acc}
    case 0n _: None{}
    case 1n+k SCon{+c, +t}:
      MI.rows.add(MI.row.parse(String.take(SCon{c, t}, 64n)), pos, acc, rs => MI.rows(k, String.drop(SCon{c, t}, 64n), U32.add(pos, 64), rs))

def MI.append.indexed(r: StoreResult<MIndex>, +idx: String, +path: String, +conv: String, repaired: Bool) -> IO(StoreResult<Bool>):
  match r:
    case Stored{_}: MI.pure(Bool, repaired)
    case _:
      MI.bind(U32, Bool, MI.size(path), size => MI.bind(MIndex, Bool, MI.build(idx, path, conv, size), st => MI.pure(Bool, repaired)))

def MI.append.put(+idx: String, +path: String, +conv: String, +body: MB.Body, st: MIndex, repaired: Bool) -> IO(StoreResult<Bool>):
  do IO<StoreResult<Bool>>:
    r : StoreResult<MIndex> <- MI.put(idx, path, conv, body, Scan.byte_count(MB.Body.line(body), 0), st)
    MI.append.indexed(r, idx, path, conv, repaired)

def MI.append.after(+r: StoreResult<Bool>, +idx: String, +path: String, +conv: String, +body: MB.Body, +st: MIndex) -> IO(StoreResult<Bool>):
  MI.then(Bool, Bool, r, repaired => MI.append.put(idx, path, conv, body, st, repaired))

def MI.append.ready(+st: MIndex, +home: String, +idx: String, +path: String, +conv: String, +body: MB.Body) -> IO(StoreResult<Bool>):
  do IO<StoreResult<Bool>>:
    r : StoreResult<Bool> <- MailStore.append.raw(home, body)
    MI.append.after(r, idx, path, conv, body, st)

def MI.append.size(+r: StoreResult<U32>, +home: String, +idx: String, +path: String, +conv: String, +body: MB.Body) -> IO(StoreResult<Bool>):
  match r:
    case StoreError{+p, +e}: MI.bad(Bool, p, e)
    case Missing{_}:
      do IO<StoreResult<Bool>>:
        r : StoreResult<Bool> <- MailStore.append.raw(home, body)
        MI.append.new(r, idx, path, conv)
    case Stored{+size}:
      MI.bind(MIndex, Bool, MI.ensure(idx, path, conv, size), st => MI.append.ready(st, home, idx, path, conv, body))

def MI.append(+home: String, +body: MB.Body) -> IO(StoreResult<Bool>):
  do IO<StoreResult<Bool>>:
    +conv : String = ML.Conv.id(MB.Body.conv(body))
    +path : String = MB.Body.path(home, conv)
    r : StoreResult<U32> <- MI.size(path)
    MI.append.size(r, home, MI.path(home, conv), path, conv, body)

# Public signatures are unchanged for the concurrent hub/mailview callers.
def MailStore.append(home: String, +body: MB.Body) -> IO(StoreResult<Bool>):
  MI.append(home, body)

# One bounded IO request for a page's exact body ranges. Decode the rows on a
# flat balanced tree: the first forks distribute all page parsing over cores.
def MailFile.texts(path: String, ranges: List<&2, U32>) -> IO(Result<&1, &1, U32 & String, List<&2, String>>):
  import "./effects/mailstore.c"
  import "./effects/mailstore.js"

def MI.ranges(rows: List<&2, MIndexRow>) -> List<&2, U32>:
  match rows:
    case Nil{}: Nil{}
    case MIndexRow{at, size, _, _, _, _, _} <> t: at <> size <> MI.ranges(t)

def MI.texts(+path: String, rows: List<&2, MIndexRow>) -> IO(StoreResult<List<&2, String>>):
  do IO<StoreResult<List<&2, String>>>:
    r : Result<&1, &1, U32 & String, List<&2, String>> <- MailFile.texts(path, MI.ranges(rows))
    return MI.fx(List<&2, String>, r, path)

def MI.pick.unique(unique: Bool, r: MIndexRow, n: Nat, acc: List<&2, MIndexRow>, k: Nat -> List<&2, MIndexRow> -> List<&2, MIndexRow>) -> List<&2, MIndexRow>:
  match unique:
    case False{}: k(n, acc)
    case True{}: k(Nat.sub(n, 1n), r <> acc)

def MI.pick(rows: List<&2, MIndexRow>, n: Nat, acc: List<&2, MIndexRow>) -> List<&2, MIndexRow>:
  match rows n:
    case _ 0n: List.reverse(&2, MIndexRow, acc)
    case Nil{} _: List.reverse(&2, MIndexRow, acc)
    case MIndexRow{at, size, hash, prev, +first, poison, +pos} <> t _:
      MI.pick.unique(U32.is_eq(first, pos), MIndexRow{at, size, hash, prev, first, poison, pos}, n, acc, left => rs => MI.pick(t, left, rs))

type MIJob is Data:
  MIJob{row: MIndexRow, text: String}

type MIJobs is Data:
  MINone{}
  MIOne{job: MIJob}
  MITwo{a: MIJobs, b: MIJobs}

def MI.jobs(rows: List<&2, MIndexRow>, texts: List<&2, String>, acc: List<&2, MIJobs>) -> Maybe<&2, List<&2, MIJobs>>:
  match rows texts:
    case Nil{} Nil{}: Some{List.reverse(&2, MIJobs, acc)}
    case r <> rt text <> tt: MI.jobs(rt, tt, MIOne{MIJob{r, text}} <> acc)
    case _ _: None{}

def MI.pairs(jobs: List<&2, MIJobs>, acc: List<&2, MIJobs>) -> List<&2, MIJobs>:
  match jobs:
    case Nil{}: List.reverse(&2, MIJobs, acc)
    case a <> Nil{}: List.reverse(&2, MIJobs, a <> acc)
    case a <> b <> t: MI.pairs(t, MITwo{a, b} <> acc)

def MI.tree(fuel: Nat, jobs: List<&2, MIJobs>) -> MIJobs:
  match fuel jobs:
    case _ Nil{}: MINone{}
    case _ j <> Nil{}: j
    case 0n _: MINone{}
    case 1n+k _: MI.tree(k, MI.pairs(jobs, Nil{}))

def MI.parsed(b: Maybe<&2, MB.Body>, conv: String, hash: U32) -> Maybe<&2, MB.Body>:
  match b:
    case None{}: None{}
    case Some{+b}: MB.Body.when(MB.Body, Bool.and(String.eq(ML.Conv.id(MB.Body.conv(b)), conv), U32.is_eq(MI.key(ML.Mail.id(MB.Body.mail(b))), hash)), b)

def MI.parse(text: String, conv: String, r: MIndexRow) -> Maybe<&2, MB.Body>:
  match r:
    case MIndexRow{_, _, hash, _, _, _, _}: MI.parsed(MB.Body.parse_line(text), conv, hash)

type MIRead is Data:
  MRNone{}
  MROne{body: Maybe<&2, MB.Body>}
  MRTwo{a: MIRead, b: MIRead}

def MI.run(jobs: MIJobs, +conv: String) -> MIRead:
  match jobs:
    case MINone{}: MRNone{}
    case MIOne{MIJob{row, text}}: MROne{MI.parse(text, conv, row)}
    case MITwo{a, b}:
      x y = MI.run(a, conv) MI.run(b, conv)
      MRTwo{x, y}

# Fold left then right into the reversed accumulator to retain newest-first
# order when MI.page.end reverses it, even when a page spans several blocks.
def MI.collect(tree: MIRead, acc: Maybe<&2, List<&2, MB.Body>>) -> Maybe<&2, List<&2, MB.Body>>:
  match tree:
    case MRNone{}: acc
    case MROne{b}:
      do Maybe<&2, List<&2, MB.Body>>:
        xs : List<&2, MB.Body> <- acc
        body : MB.Body <- b
        return body <> xs
    case MRTwo{a, b}: MI.collect(b, MI.collect(a, acc))

def MI.batch.done(bodies: Maybe<&2, List<&2, MB.Body>>, path: String, left: Nat, k: Bool -> Nat -> List<&2, MB.Body> -> IO(StoreResult<List<&2, MB.Body>>)) -> IO(StoreResult<List<&2, MB.Body>>):
  match bodies:
    case None{}: MI.bad(List<&2, MB.Body>, path, "invalid indexed body row")
    case Some{bs}: k(False{}, left, bs)

def MI.batch.jobs(jobs: Maybe<&2, List<&2, MIJobs>>, path: String, conv: String, left: Nat, acc: List<&2, MB.Body>, k: Bool -> Nat -> List<&2, MB.Body> -> IO(StoreResult<List<&2, MB.Body>>)) -> IO(StoreResult<List<&2, MB.Body>>):
  match jobs:
    case None{}: MI.bad(List<&2, MB.Body>, path, "incomplete vectored body read")
    case Some{js}:
      do IO<StoreResult<List<&2, MB.Body>>>:
        decoded : MIRead = MI.run(MI.tree(8n, js), conv)
        MI.batch.done(MI.collect(decoded, Some{acc}), path, left, k)

def MI.batch(+rows: List<&2, MIndexRow>, +path: String, conv: String, left: Nat, acc: List<&2, MB.Body>, k: Bool -> Nat -> List<&2, MB.Body> -> IO(StoreResult<List<&2, MB.Body>>)) -> IO(StoreResult<List<&2, MB.Body>>):
  MI.bind(List<&2, String>, List<&2, MB.Body>, MI.texts(path, rows), texts => MI.batch.jobs(MI.jobs(rows, texts, Nil{}), path, conv, left, acc, k))

def MI.page.rows(+rows: List<&2, MIndexRow>, idx: String, +path: String, conv: String, waiting: Bool, before: String, +left: Nat, acc: List<&2, MB.Body>, k: Bool -> Nat -> List<&2, MB.Body> -> IO(StoreResult<List<&2, MB.Body>>)) -> IO(StoreResult<List<&2, MB.Body>>):
  +picked = MI.pick(rows, left, Nil{})
  MI.batch(picked, path, conv, Nat.sub(left, List.length(&2, MIndexRow, picked)), acc, k)

def MI.page.block(rows: Maybe<&2, List<&2, MIndexRow>>, idx: String, path: String, conv: String, waiting: Bool, before: String, left: Nat, acc: List<&2, MB.Body>, k: Bool -> Nat -> List<&2, MB.Body> -> IO(StoreResult<List<&2, MB.Body>>)) -> IO(StoreResult<List<&2, MB.Body>>):
  match rows:
    case None{}: MI.bad(List<&2, MB.Body>, idx, "invalid mail index page")
    case Some{rs}: MI.page.rows(rs, idx, path, conv, waiting, before, left, acc, k)

def MI.page(+fuel: Nat, +idx: String, +path: String, +conv: String, +end: U32, +waiting: Bool, +before: String, +left: Nat, +acc: List<&2, MB.Body>) -> IO(StoreResult<List<&2, MB.Body>>):
  match fuel end left:
    case _ _ 0n: MI.page.end(waiting, path, acc)
    case _ 65536 _: MI.page.end(waiting, path, acc)
    case 0n _ _: MI.bad(List<&2, MB.Body>, idx, "mail index page exhausted")
    case 1n+k _ _:
      +bytes = Bool.pick(U32, U32.is_lt(U32.sub(end, 65536), 4096), U32.sub(end, 65536), 4096)
      +start = U32.sub(end, bytes)
      MI.bind(String, List<&2, MB.Body>, MI.read(idx, start, bytes), text => MI.page.block(MI.rows(64n, text, start, Nil{}), idx, path, conv, waiting, before, left, acc, w => n => bs => MI.page(k, idx, path, conv, start, w, before, n, bs)))

def MI.read.found(+pos: U32, +idx: String, +path: String, +conv: String, +n: Nat) -> IO(StoreResult<List<&2, MB.Body>>):
  match pos:
    case 0: MI.bad(List<&2, MB.Body>, path, "unknown mail cursor")
    case +pos: MI.page(1n+U32.to_nat(pos), idx, path, conv, pos, False{}, "", n, Nil{})

def MI.read.cursor(+empty: Bool, +idx: String, +path: String, +conv: String, +end: U32, +before: String, +n: Nat) -> IO(StoreResult<List<&2, MB.Body>>):
  match empty:
    case True{}: MI.page(1n+U32.to_nat(end), idx, path, conv, end, False{}, "", n, Nil{})
    case False{}:
      +hash = MI.key(before)
      MI.bind(U32, List<&2, MB.Body>, MI.head(idx, hash), head =>
        MI.bind(U32, List<&2, MB.Body>, MI.find(1n+U32.to_nat(end), idx, path, conv, before, hash, head, end), pos => MI.read.found(pos, idx, path, conv, n)))

def MI.read.state(+st: MIndex, +idx: String, +path: String, +conv: String, +before: String, +n: Nat) -> IO(StoreResult<List<&2, MB.Body>>):
  match st:
    case MIndex{+end, _, 0}: MI.read.cursor(String.is_empty(before), idx, path, conv, end, before, n)
    case _: MI.bad(List<&2, MB.Body>, idx, "conflicting duplicate mail id")

def MI.read.retry(+r: StoreResult<List<&2, MB.Body>>, +idx: String, +path: String, +conv: String, +before: String, +n: Nat, +size: U32) -> IO(StoreResult<List<&2, MB.Body>>):
  match r:
    case Stored{+bs}: MI.pure(List<&2, MB.Body>, bs)
    case _:
      MI.bind(MIndex, List<&2, MB.Body>, MI.build(idx, path, conv, size), st => MI.read.state(st, idx, path, conv, before, n))

def MI.read.ready(+st: MIndex, +idx: String, +path: String, +conv: String, +before: String, +n: Nat, +size: U32) -> IO(StoreResult<List<&2, MB.Body>>):
  do IO<StoreResult<List<&2, MB.Body>>>:
    r : StoreResult<List<&2, MB.Body>> <- MI.read.state(st, idx, path, conv, before, n)
    MI.read.retry(r, idx, path, conv, before, n, size)

def MI.read.page(+home: String, +conv: String, +before: String, +n: Nat) -> IO(StoreResult<List<&2, MB.Body>>):
  match n:
    case 0n: MI.pure(List<&2, MB.Body>, Nil{})
    case 1n+k:
      +idx = MI.path(home, conv)
      +path = MB.Body.path(home, conv)
      +limit = Bool.pick(Nat, Nat.is_lt(k, 200n), 1n+k, 200n)
      MI.bind(U32, List<&2, MB.Body>, MI.size(path), +size =>
        MI.bind(MIndex, List<&2, MB.Body>, MI.ensure(idx, path, conv, size), st => MI.read.ready(st, idx, path, conv, before, limit, size)))

def MailStore.read(+home: String, +conv: String, before: String, +n: Nat) -> IO(StoreResult<List<&2, MB.Body>>):
  MI.read.page(home, conv, before, n)
