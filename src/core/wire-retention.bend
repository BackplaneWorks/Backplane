import Base

# In store mode the durable, indexed log is authoritative. No old event
# payload belongs in the hub or a client's replay-credit record. The empty
# list is a zero-byte cache; reads use bounded disk cursor pages instead.
# Compatibility in-memory hubs retain the original complete sequence.
def WireRetention.keep(stored: Bool, xs: List<&2, String>) -> List<&2, String>:
  match stored:
    case True{}: Nil{}
    case False{}: xs

# Queued changes are already visible to clients before disk recovers. Their
# public wire list is newest first, counted independently from State.seq.
def WireRetention.durable(total: Nat, queued: List<&2, String>) -> Nat:
  Nat.sub(total, List.length(&2, String, queued))

# Pending replay has no new payload copy: at most 1024 references, oldest
# first. The ordinary outbox credit further limits each offered frame.
def WireRetention.pending(total: Nat, since: Nat, queued: List<&2, String>) -> List<&2, String>:
  List.take(&2, String, List.reverse(&2, String, List.take(&2, String, queued, Nat.sub(total, since))), 1024n)
