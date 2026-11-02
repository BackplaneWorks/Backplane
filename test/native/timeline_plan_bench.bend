import Base
import ../../src/core/json.bend as J
import ../../src/core/timeline-virtual.bend as V

def size.of(m: Nat) -> String:
  match m:
    case 0n: "87.33333587646484"
    case 1n: "24"
    case 2n: "151.5"
    case 3n: "23.999998092651367"
    case _: "412.796875"

def size.at(n: Nat) -> String: size.of(Nat.mod(n, 5n))

def ks(+n: Nat, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match n:
    case 0n: acc
    case 1n+k: ks(k, J.Str{"row:" ++ Nat.show(k)} <> acc)

def zs(+n: Nat, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match n:
    case 0n: acc
    case 1n+k: zs(k, J.Num{size.at(k)} <> acc)

def req(+n: Nat, top: String, mode: String, previous: J.Json) -> J.Json:
  J.Json.obj([J.KV{"keys", J.Json.arr(ks(n, Nil{}))}, J.KV{"sizeList", J.Json.arr(zs(n, Nil{}))}, J.KV{"estimate", J.Num{"120"}},
    J.KV{"previous", previous}, J.KV{"top", J.Num{top}}, J.KV{"view", J.Num{"785"}}, J.KV{"mode", J.Str{mode}},
    J.KV{"follow", J.Flag{False{}}}, J.KV{"overscan", J.Num{"785"}}])

def loop(+k: Nat, +n: Nat, prev: J.Json) -> J.Json:
  match k:
    case 0n: prev
    case 1n+j: loop(j, n, V.plan(req(n, Nat.show(Nat.add(20000n, Nat.mul(j, 613n))) ++ ".5", "scroll", prev)))

def run(+k: Nat, +n: Nat) -> String:
  +p0 = V.plan(req(n, "0", "init", J.Null{}))
  +p = loop(k, n, p0)
  +p2 = V.plan(req(n, "31234.25", "update", p))
  +p3 = V.plan(J.Json.obj([J.KV{"mode", J.Str{"readback"}}, J.KV{"previous", p2}, J.KV{"top", J.Num{"31234"}}, J.KV{"readbackAttempt", J.Num{"1"}}]))
  J.Json.show(p3)

def main() -> IO(Unit):
  IO.print(run(20n, 1000n))
