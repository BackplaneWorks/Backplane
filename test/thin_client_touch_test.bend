import Base
import ./check.bend as C
import ../src/core/thin-pace.bend as P

def private(cid: String) -> P.Touch:
  P.Touch.add(P.Touch.none(), P.Touch.client(cid))

def level(name: String, t: P.Touch, th: String, cid: String, expected: String) -> IO(Unit):
  C.Check.str(name, Nat.show(P.Touch.level.for(t, th, cid)), expected)

type Batch is Data:
  Batch{paces: Map<&2, P.Pace>, sent: Nat, foreign: Nat}

def Batch.empty() -> Batch:
  Batch{Map.new(&2, P.Pace), 0n, 0n}

def Batch.apply(+paced: P.Paced, +cid: String, +owner: String, b: Batch) -> Batch:
  match b:
    case Batch{ps, sent, foreign}:
      Batch{Map.set(&2, P.Pace, ps, cid, P.Paced.pace(paced)),
        Nat.add(sent, Bool.pick(Nat, P.Paced.send(paced), 1n, 0n)),
        Nat.add(foreign, Bool.pick(Nat, Bool.and(P.Paced.send(paced), Bool.not(String.eq(cid, owner))), 1n, 0n))}

def Batch.one(+cid: String, +owner: String, t: P.Touch, now: Nat, b: Batch) -> Batch:
  match b:
    case Batch{+ps, sent, foreign}:
      Batch.apply(P.Pace.step(P.Pace.of(ps, cid), now, P.Touch.level.for(t, "same-thread", cid)), cid, owner, Batch{ps, sent, foreign})

# Every completion is passed to all twenty existing paced connections. They
# deliberately show the same thread: thread ownership cannot isolate replies.
def Batch.clients(n: Nat, +owner: String, +t: P.Touch, +now: Nat, b: Batch) -> Batch:
  match n:
    case 0n: b
    case 1n+next:
      +k = next
      Batch.clients(k, owner, t, now, Batch.one(Nat.show(k), owner, t, now, b))

def Batch.completions(n: Nat, b: Batch) -> Batch:
  match n:
    case 0n: b
    case 1n+next:
      +k = next
      +owner = Nat.show(k)
      Batch.completions(k, Batch.clients(20n, owner, private(owner), 1000n, b))

def Batch.sent(b: Batch) -> Nat:
  match b:
    case Batch{_, sent, _}: sent

def Batch.foreign(b: Batch) -> Nat:
  match b:
    case Batch{_, _, foreign}: foreign

def main() -> IO(Unit):
  do IO<Unit>:
    level("private owner urgent", private("7"), "thread", "7", "2")
    level("other connection same thread stays quiet", private("7"), "thread", "8", "0")
    level("other connection different thread stays quiet", private("7"), "other", "8", "0")
    level("owner without selected thread still urgent", private("7"), "", "7", "2")
    C.Check.str("private completion creates no sidebar work", Bool.show(P.Touch.any(private("7"))), "False")
    C.Check.str("legacy thread policy ignores private completion", Nat.show(P.Touch.level(private("7"), "thread")), "0")
    level("client prefix cannot match another client", private("70"), "thread", "7", "0")
    level("client suffix cannot match another client", private("17"), "thread", "7", "0")
    level("plain thread ID cannot impersonate client marker", P.Touch.add(P.Touch.none(), "7"), "other", "7", "1")
    level("live thread marker cannot impersonate client marker", P.Touch.add(P.Touch.none(), P.Touch.live("7")), "other", "7", "0")
    level("empty client never gains private ownership", private(""), "thread", "", "0")
    level("empty marker cannot match real client", private(""), "thread", "7", "0")
    level("no client marker means no phantom ownership", P.Touch.none(), "thread", "7", "0")
    level("another private marker does not erase real thread owner", P.Touch.add(private("7"), "thread"), "thread", "8", "2")
    level("real thread change keeps sidebar cadence", P.Touch.add(private("7"), "thread"), "other", "8", "1")
    level("private owner remains urgent beside real thread change", P.Touch.add(private("7"), "thread"), "other", "7", "2")
    level("mixed private and live touch keeps live owner urgent", P.Touch.add(private("7"), P.Touch.live("thread")), "thread", "8", "2")
    level("mixed private and live touch leaves others quiet", P.Touch.add(private("7"), P.Touch.live("thread")), "other", "8", "0")
    level("global touch still reaches every connection", P.Touch.add(private("7"), "*"), "other", "8", "2")
    level("two private completions reach each owner", P.Touch.add(private("7"), P.Touch.client("8")), "thread", "8", "2")
    level("two private completions leave third connection quiet", P.Touch.add(private("7"), P.Touch.client("8")), "thread", "9", "0")
    +batch : Batch = Batch.completions(20n, Batch.empty())
    C.Check.str("twenty completions send twenty owned notices", Nat.show(Batch.sent(batch)), "20")
    C.Check.str("twenty completions send no foreign notices", Nat.show(Batch.foreign(batch)), "0")
    C.Check.str("flush produces no deferred foreign notices", Nat.show(Batch.sent(Batch.clients(20n, "", P.Touch.none(), 1500n, batch))), "20")
