import Base
import ./thin.bend as T
import ./live.bend as L
import ./json.bend as J
import ./model.bend as M

# Process-local UTF8 blocks. Local deltas extend only the last <=512-byte
# block and new text.
type Stream is Data:
  Stream{generation: Nat, size: Nat, blocks: T.Blocks, cut: Bool, version: Nat}

def Stream.blank() -> Stream:
  Stream{0n, 0n, T.Blocks.of(""), False{}, 0n}

def Stream.get.k( p: Map<&2, Stream> & Stream) -> Stream:
  (_, value) = p
  value

def Stream.get( +streams: Map<&2, Stream>, +thread: String) -> Stream:
  Stream.get.k(Map.get(Stream, Stream.blank(), streams, thread))

def Stream.blocks( +s: Stream) -> T.Blocks:
  match s:
    case Stream{_, _, blocks, _, _}: blocks

def Stream.version( +s: Stream) -> Nat:
  match s:
    case Stream{_, _, _, _, version}: version

def Stream.size( +s: Stream) -> Nat:
  match s:
    case Stream{_, size, _, _, _}: size

def Stream.cut( +s: Stream) -> Bool:
  match s:
    case Stream{_, _, _, cut, _}: cut

def Stream.snapshot( +b: L.Buf, +version: Nat) -> Stream:
  match b:
    case L.Buf{_, chunks, size, cut, gen}: Stream{gen, size, T.Blocks.of(String.join(List.reverse(&2, String, chunks), "")), cut, version}

def Streams.snapshot( +bs: List<&2, L.Buf>, +version: Nat, +streams: Map<&2, Stream>) -> Map<&2, Stream>:
  match bs:
    case Nil{}: streams
    case +b <> tail: Streams.snapshot(tail, version, Map.set(&2, Stream, streams, L.Buf.thread(b), Stream.snapshot(b, version)))

def Bytes.go( +text: String, +n: Nat) -> Nat:
  match text:
    case SNil{}: n
    case SCon{Chr{c}, tail}: Bytes.go(tail, Nat.add(n, T.Bytes.char(c)))

def Blocks.append(+blocks: T.Blocks, +text: String) -> T.Blocks:
  match blocks:
    case T.Blocks{+n, parts}:
      +last = T.Blocks.get(blocks, Nat.sub(n, 1n))
      T.Blocks.go(text, T.BlockBuild{Nat.sub(n, 1n), String.reverse(last), Bytes.go(last, 0n), parts})

def Stream.append( +s: Stream, +text: String, +version: Nat) -> Stream:
  match s:
    case Stream{g, size, blocks, cut, _}: Stream{g, Nat.add(size, String.length(text)), Blocks.append(blocks, text), cut, version}

def Streams.delta_one( +append: Bool, +old: Stream, +b: L.Buf, +text: String, +version: Nat) -> Stream:
  match append:
    case True{}: Stream.append(old, text, version)
    case False{}: Stream.snapshot(b, version)

def Streams.delta_pick( +hit: Bool, +b: L.Buf, +old: Stream, +text: String, +version: Nat, +found: Stream) -> Stream:
  match hit b old:
    case False{} _ _: found
    case True{} L.Buf{_, _, size, cut, gen} Stream{oldGen, oldSize, _, _, _}:
      Streams.delta_one(Bool.and(Bool.not(cut), Bool.and(Nat.is_eq(gen, oldGen), Nat.is_eq(size, Nat.add(oldSize, String.length(text))))), old, b, text, version)

def Streams.delta_find( +bs: List<&2, L.Buf>, +old: Stream, +thread: String, +text: String, +version: Nat, +found: Stream) -> Stream:
  match bs:
    case Nil{}: found
    case +b <> tail: Streams.delta_find(tail, old, thread, text, version, Streams.delta_pick(String.eq(L.Buf.thread(b), thread), b, old, text, version, found))

def Streams.delta(+streams: Map<&2, Stream>, +bs: List<&2, L.Buf>, +thread: String, +text: String, +version: Nat) -> Map<&2, Stream>:
  Map.set(&2, Stream, streams, thread, Streams.delta_find(bs, Stream.get(streams, thread), thread, text, version, Stream.get(streams, thread)))

# The streamed reply whole: message text is never folded. Only what the
# hub's own buffer dropped past Live.cap() (the stream's cut) is marked.
def Stream.preview(+s: Stream) -> String:
  Bool.pick(String, Stream.cut(s), "…\n", "") ++
    T.Blocks.join(T.Blocks.n(Stream.blocks(s)), Stream.blocks(s), 0n, Nil{})

def Stream.id( +thread: String) -> String:
  "@live:" ++ thread

def Stream.entry( +s: Stream, +thread: String, +now: Nat) -> M.Entry:
  M.Msg{Stream.id(thread), thread, M.Assistant{}, Stream.preview(s), now}

# Empty streams produce no synthetic entry. No canonical event is emitted.
def Stream.entries( +nonempty: Bool, +s: Stream, +thread: String, +now: Nat) -> List<&2, M.Entry>:
  match nonempty:
    case True{}: [Stream.entry(s, thread, now)]
    case False{}: Nil{}
