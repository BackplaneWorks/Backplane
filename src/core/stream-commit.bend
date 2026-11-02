import Base
import ./model.bend as M
import ./live.bend as LV
import ./thin-live.bend as TLIVE

# A committed assistant message or terminal turn can change a live buffer.
# Other commits must leave the stream map (including its versions) alone.
def Streams.any.go(cs: List<&2, M.Change>, found: Bool) -> Bool:
  match cs found:
    case _ True{}: True{}
    case Nil{} False{}: False{}
    case c <> rest False{}:
      Streams.any.go(rest, Bool.not(String.is_empty(LV.Live.ended(c))))

def Streams.any(cs: List<&2, M.Change>) -> Bool:
  Streams.any.go(cs, False{})

def Streams.affected.add(empty: Bool, thread: String, affected: Set()) -> Set():
  match empty:
    case True{}: affected
    case False{}: Set.add(affected, thread)

def Streams.affected.go(cs: List<&2, M.Change>, affected: Set()) -> Set():
  match cs:
    case Nil{}: affected
    case c <> rest:
      +thread = LV.Live.ended(c)
      Streams.affected.go(rest, Streams.affected.add(String.is_empty(thread), thread, affected))

# Unique thread IDs, exactly the nonempty results of Live.ended.
def Streams.affected(cs: List<&2, M.Change>) -> Set():
  Streams.affected.go(cs, Set.new())

def Streams.commit.one(hit: Bool, b: LV.Buf, version: Nat, streams: Map<&2, TLIVE.Stream>) -> Map<&2, TLIVE.Stream>:
  match hit:
    case False{}: streams
    case True{}:
      +buf = b
      Map.set(&2, TLIVE.Stream, streams, LV.Buf.thread(buf), TLIVE.Stream.snapshot(buf, version))

def Streams.member(result: Set() & Bool) -> Bool:
  (_, hit) = result
  hit

def Streams.commit.buffers(bs: List<&2, LV.Buf>, +affected: Set(), +version: Nat, streams: Map<&2, TLIVE.Stream>) -> Map<&2, TLIVE.Stream>:
  match bs:
    case Nil{}: streams
    case +b <> rest:
      Streams.commit.buffers(rest, affected, version,
        Streams.commit.one(Streams.member(Set.has(affected, LV.Buf.thread(b))), b, version, streams))

def Streams.commit.clear(threads: List<&2, String>, streams: Map<&2, TLIVE.Stream>) -> Map<&2, TLIVE.Stream>:
  match threads:
    case Nil{}: streams
    case thread <> rest: Streams.commit.clear(rest, Map.del(&2, TLIVE.Stream, streams, thread))

def Streams.commit.when(ending: Bool, cs: List<&2, M.Change>, bs: List<&2, LV.Buf>, version: Nat, streams: Map<&2, TLIVE.Stream>) -> Map<&2, TLIVE.Stream>:
  match ending:
    case False{}: streams
    case True{}:
      +affected = Streams.affected(cs)
      Streams.commit.buffers(bs, affected, version, Streams.commit.clear(Set.to_list(affected), streams))

# bs MUST already be Steer.keep(cs, old, Live.ends(cs, old)). Snapshot
# final buffers once, so repeated endings preserve their final generation
# and ahead messages preserve their retained tail. Affected slots absent
# from bs are deleted; unaffected slots survive even without a buffer.
# Metadata-only commits never traverse bs or streams.
def Streams.commit(+cs: List<&2, M.Change>, bs: List<&2, LV.Buf>, version: Nat, streams: Map<&2, TLIVE.Stream>) -> Map<&2, TLIVE.Stream>:
  Streams.commit.when(Streams.any(cs), cs, bs, version, streams)
