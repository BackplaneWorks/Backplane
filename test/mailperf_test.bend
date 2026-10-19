import Base
import ./check.bend as C
import ../src/core/mail.bend as ML

def ids(cs: List<&2, ML.Conv>, acc: String) -> String:
  match cs:
    case Nil{}: acc
    case c <> t: ids(t, acc ++ ML.Conv.id(c) ++ ",")

def reference(+b: ML.Box, +bot: String, status: String, project: String, q: String, skip: Nat, n: Nat) -> List<&2, ML.Conv>:
  +tree = ML.Box.order_of(b, bot)
  ML.Box.query(1n+Nat.mul(3n, ML.Order.count(tree)), ML.MQuery{[tree], skip, n, Nil{}}, b, bot, status, project, String.to_lower(q))

def seed(+n: Nat, b: ML.Box) -> ML.Box:
  match n:
    case 0n: b
    case 1n+k:
      +id = "c" ++ Nat.show(k)
      +bot = "b" ++ Nat.show(Nat.mod(k, 6n))
      seed(k, ML.Box.conv_set(b, id, Bool.pick(String, Nat.is_eq(Nat.mod(k, 17n), 0n), "needle 日本語", "Ordinary subject"), "pair", ["bot:" ++ bot, "thread:w"], "p" ++ Nat.show(Nat.mod(k, 2n)), ["tag"], k))

def posts(+n: Nat, +convs: Nat, b: ML.Box) -> ML.Box:
  match n:
    case 0n: b
    case 1n+k:
      +c = Nat.mod(k, convs)
      posts(k, convs, ML.Box.mail(b, ML.Mail{"m" ++ Nat.show(k), "c" ++ Nat.show(c), "person", "bot:b" ++ Nat.show(Nat.mod(c, 6n)), "thread", "", "fyi", "Latest body header", 18n, Nat.add(10000n, k)}))

def queries(qs: List<&2, String>, +b: ML.Box, +bot: String) -> IO(Unit):
  match qs:
    case Nil{}: IO.pure(Unit, Unit{})
    case +q <> t:
      do IO<Unit>:
        C.Check.str("find equals full metadata scan " ++ bot ++ " " ++ q, ids(ML.Box.find(b, bot, q, 20n), ""), ids(reference(b, bot, "", "", q, 0n, 20n), ""))
        queries(t, b, bot)

def main() -> IO(Unit):
  do IO<Unit>:
    +b : ML.Box = posts(300n, 60n, seed(60n, ML.Box.empty()))
    C.Check.str("global order contains every conversation", Nat.show(ML.Box.count(b, "*", "", "")), "60")
    C.Check.str("global last page rank seek", Nat.show(List.length(&2, ML.Conv, ML.Box.slice(b, "*", 55n, 20n))), "5")
    C.Check.str("thread participant has own complete order", Nat.show(ML.Box.count(b, "thread:w", "", "")), "60")
    C.Check.str("thread without membership empty", Nat.show(ML.Box.count(b, "thread:other", "", "")), "0")
    C.Check.str("bot scope isolated", Nat.show(ML.Box.count(b, "b0", "", "")), "10")
    C.Check.str("status and project count", Nat.show(ML.Box.count(b, "b0", "unread", "p0")), "10")
    C.Check.str("project count exact", Nat.show(ML.Box.count(b, "*", "", "p1")), "30")
    queries(["needle", "日本", "absent-query", "a", "ab", "latest body", "thread:w", "TAG", ""], b, "*")
    queries(["needle", "absent-query", "Latest", "b0"], b, "b0")
    +marked : ML.Box = ML.Box.tagged(ML.Box.marked(b, "b0", "c0", "done", 0n, 0n), "b0", "c0", ["unique-note"])
    C.Check.str("status count remains exact", Nat.show(ML.Box.count(marked, "b0", "unread", "")), "9")
    C.Check.str("status and project page", ids(ML.Box.list(marked, "b0", "done", "p0", 0n, 10n), ""), "c0,")
    C.Check.str("mark tags searched per bot", ids(ML.Box.find(marked, "b0", "unique-note", 10n), ""), "c0,")
    C.Check.str("mark tags do not leak", ids(ML.Box.find(marked, "*", "unique-note", 10n), ""), "")
    +untagged : ML.Box = ML.Box.tagged(marked, "b0", "c0", Nil{})
    C.Check.str("old mark tag removed", ids(ML.Box.find(untagged, "b0", "unique-note", 10n), ""), "")
    +moved : ML.Box = ML.Box.conv_set(marked, "c0", "Replacement", "pair", ["bot:new"], "p9", Nil{}, 20000n)
    C.Check.str("removed bot membership disappears", Nat.show(ML.Box.count(moved, "b0", "", "")), "9")
    C.Check.str("new bot membership appears", Nat.show(ML.Box.count(moved, "new", "", "p9")), "1")
    C.Check.str("old metadata posting disappears", ids(ML.Box.find(moved, "new", "needle", 10n), ""), "")
    +fresh : ML.Box = ML.Box.mail(b, ML.Mail{"newhead", "c1", "person", "bot:b1", "thread", "", "fyi", "distinct-new-preview", 10n, 90000n})
    C.Check.str("global moves newest first", ids(ML.Box.slice(fresh, "*", 0n, 1n), ""), "c1,")
    C.Check.str("new header searchable", ids(ML.Box.find(fresh, "b1", "distinct-new", 10n), ""), "c1,")
    C.Check.str("old head removed", Nat.show(List.length(&2, ML.Conv, ML.Box.find(fresh, "*", "latest body", 200n))), "59")
    C.Check.str("newest header handles late replay", ids(ML.Box.find(ML.Box.mail(fresh, ML.Mail{"late", "c1", "person", "bot:b1", "thread", "", "fyi", "late-only-preview", 10n, 1n}), "b1", "late-only", 10n), ""), "")
    C.Check.str("removed thread membership disappears", Nat.show(ML.Box.count(moved, "thread:w", "", "")), "59")
    C.Check.str("thread order has no bot marks", ML.Mark.bot(ML.Box.mark_of(b, "thread:w", "c0")), "")
    C.Check.str("repeated metadata does not duplicate global rows", Nat.show(ML.Box.count(ML.Box.conv_set(b, "c0", "Again", "pair", ["bot:b0", "thread:w"], "p0", Nil{}, 1n), "*", "", "")), "60")
    C.Check.str("duplicate mail does not duplicate global rows", Nat.show(ML.Box.count(posts(300n, 60n, b), "*", "", "")), "60")
    +workers : ML.Box = ML.Box.conv_set(b, "workers", "No bot", "pair", ["thread:left", "thread:right"], "", Nil{}, 100000n)
    C.Check.str("global includes thread-only conversations", Nat.show(ML.Box.count(workers, "*", "", "")), "61")
    C.Check.str("each thread gets only its own conversation", ids(ML.Box.slice(workers, "thread:left", 0n, 10n), ""), "workers,")
    +person : ML.Box = ML.Box.conv_set(workers, "person-only", "No thread or bot", "pair", ["person"], "", Nil{}, 100001n)
    C.Check.str("global includes conversations with no agent parts", ids(ML.Box.slice(person, "*", 0n, 2n), ""), "person-only,workers,")
