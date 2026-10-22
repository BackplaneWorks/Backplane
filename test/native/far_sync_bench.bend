import Base
import ../../src/core/far.bend as FR
import ../../src/core/model.bend as M
import ../../src/core/codec.bend as C
import ../../src/core/json.bend as J
import ../../src/core/client.bend as U

# What a push, a join and a client cost as a mirror grows (docs/bots.md,
# "Threads elsewhere"), on a synthetic history of N messages of one thread:
#   scripts/build-js.sh test/native/far_sync_bench.bend build/far_sync_bench
#   build/far_sync_bench           (4000, 16000 and 64000 items, 100 pushes each)
# Each line is ms for the whole step; "per" is per push or per chunk.
#   mirror push   a batch of one item taken into a mirror of N items
#   mirror join   a client joining a mirror of N items, chunk by chunk
#   client fold   the whole mirror folded into a client, then one-item batches
#   out queue     one-item commits queued behind a push in flight

def Bench.item(+n: Nat) -> FR.Item:
  FR.Item{n, C.Change.encode(M.MessagePosted{"t1", "m:" ++ Nat.show(n), M.Assistant{}, "reply number " ++ Nat.show(n) ++ " with some words to make it a realistic length of text for a message", n})}

# items lo+1..lo+k, ascending
def Bench.make(+k: Nat, +lo: Nat, acc: List<&2, FR.Item>) -> List<&2, FR.Item>:
  match k:
    case 0n:
      acc
    case 1n+p:
      Bench.make(p, lo, Bench.item(Nat.add(lo, k)) <> acc)

def Bench.pushes(k: Nat, +i: FR.In, +head: Nat) -> FR.In:
  match k:
    case 0n:
      i
    case 1n+p:
      Bench.pushes(p, FR.In.put(i, head, Nat.add(head, 1n), [Bench.item(Nat.add(head, 1n))]), Nat.add(head, 1n))

# a mirror of n items put in as pushes of 500 (a giant batch is not what a hub sends)
def Bench.fill(k: Nat, +i: FR.In, +lo: Nat) -> FR.In:
  match k:
    case 0n:
      i
    case 1n+p:
      Bench.fill(p, FR.In.put(i, lo, Nat.add(lo, 500n), Bench.make(500n, lo, Nil{})), Nat.add(lo, 500n))

def Bench.count(i: FR.In) -> Nat:
  List.length(&2, FR.Item, FR.In.items(i))

def Bench.next(c: FR.FC) -> Nat:
  match c:
    case FR.FC{_, +n, _}:
      n

def Bench.done(c: FR.FC) -> Bool:
  match c:
    case FR.FC{_, _, +d}:
      d

def Bench.text(c: FR.FC) -> String:
  match c:
    case FR.FC{+t, _, _}:
      t

# the chunks a join sends (the mirror's items in pieces of In.most()), last first
def Bench.chunks(fuel: Nat, +i: FR.In, +after: Nat, acc: List<&2, String>, stop: Bool) -> List<&2, String>:
  match fuel stop:
    case _ True{}:
      acc
    case 0n _:
      acc
    case 1n+f False{}:
      +c : FR.FC = FR.In.chunk(i, after)
      Bench.chunks(f, i, Bench.next(c), Bench.text(c) <> acc, Bench.done(c))

# a client taking each chunk in order
def Bench.fold(ts: List<&2, String>, +u: U.Ui) -> U.Ui:
  match ts:
    case Nil{}:
      u
    case +t <> r:
      Bench.fold(r, U.Ui.recv(u, Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{})))

def Bench.client(k: Nat, +u: U.Ui, +head: Nat) -> U.Ui:
  match k:
    case 0n:
      u
    case 1n+p:
      Bench.client(p, U.Ui.recv(u, Maybe.default(&2, J.Json, J.Json.parse(FR.Fr.msg("p1", "box", False{}, head, Nat.add(head, 1n), [Bench.item(Nat.add(head, 1n))])), J.Null{})), Nat.add(head, 1n))

def Bench.out(k: Nat, +o: FR.Out, +head: Nat) -> FR.Out:
  match k:
    case 0n:
      o
    case 1n+p:
      Bench.out(p, FR.Out.add(o, head, Nat.add(head, 1n), [Bench.item(Nat.add(head, 1n))]), Nat.add(head, 1n))

def Bench.ms(+what: String, +a: Nat, +b: Nat, +per: Nat, +x: Nat) -> IO(Unit):
  IO.print(what ++ ": " ++ Nat.show(Nat.sub(b, a)) ++ " ms, " ++ Nat.show(Nat.div(Nat.mul(Nat.sub(b, a), 1000n), Bool.pick(Nat, Nat.is_eq(per, 0n), 1n, per))) ++ " us per, check " ++ Nat.show(x))

def Bench.run(+n: Nat, +k: Nat) -> IO(Unit):
  do IO<Unit>:
    +i0 : FR.In = Bench.fill(Nat.div(n, 500n), FR.In.new("p1", "box"), 0n)
    +c0 : Nat = Bench.count(i0)
    IO.print("mirror of " ++ Nat.show(c0) ++ " items, " ++ Nat.show(k) ++ " pushes")
    +t0 : Nat <- IO.now()
    +i1 : FR.In = Bench.pushes(k, i0, n)
    +c1 : Nat = Bench.count(i1)
    +t1 : Nat <- IO.now()
    Bench.ms("mirror push (total of the pushes)", t0, t1, k, c1)
    +cs : List<&2, String> = Bench.chunks(100000n, i1, 0n, Nil{}, False{})
    +ch : Nat = List.length(&2, String, cs)
    +t2 : Nat <- IO.now()
    Bench.ms("mirror join (all chunks)", t1, t2, ch, ch)
    +u1 : U.Ui = Bench.fold(List.reverse(&2, String, cs), U.Ui.init(1790000000n))
    +h1 : Nat = FR.Fr.head(U.Ui.state(u1), "p1")
    +t3 : Nat <- IO.now()
    Bench.ms("client fold of the chunks", t2, t3, ch, h1)
    +u2 : U.Ui = Bench.client(k, u1, n)
    +h2 : Nat = FR.Fr.head(U.Ui.state(u2), "p1")
    +t4 : Nat <- IO.now()
    Bench.ms("client fold of one-item batches (total)", t3, t4, k, h2)
    +o1 : FR.Out = Bench.out(k, FR.Out.new("p1"), 0n)
    +w : Nat = List.length(&2, FR.Item, FR.Out.waiting(o1))
    +t5 : Nat <- IO.now()
    Bench.ms("out queue behind a push in flight (total)", t4, t5, k, w)
    return Unit{}

def main() -> IO(Unit):
  do IO<Unit>:
    Bench.run(4000n, 100n)
    Bench.run(16000n, 100n)
    Bench.run(32000n, 100n)
