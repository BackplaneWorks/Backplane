import Base
import ../src/app/mm.bend as MM

law rename_global_invalidation_retained:
  for id: String
  for +t: MM.TouchK
  {MM.TouchK.g(MM.TouchK.rename(id, t)) == MM.TouchK.n(t) : Nat}

def rename_global_invalidation_retained(id, t):
  match t:
    case MM.TouchK{n, g, r, m, l}: {==}

law rename_relationship_invalidation_retained:
  for id: String
  for +t: MM.TouchK
  {MM.TouchK.r(MM.TouchK.rename(id, t)) == MM.TouchK.r(t) : Nat}

def rename_relationship_invalidation_retained(id, t):
  match t:
    case MM.TouchK{n, g, r, m, l}: {==}

law old_touch_index_fallback:
  for n: Nat
  for +g: Nat
  for r: Nat
  for l: List<&2, MM.TkE>
  {MM.TouchK.index_g(MM.TouchK{n, g, r, Map.new(&2, Nat), l}) == g : Nat}

def old_touch_index_fallback(n, g, r, l):
  {==}

def main() -> IO(Unit):
  IO.print("ok rename global/relationship preservation and old-record fallback proofs")
