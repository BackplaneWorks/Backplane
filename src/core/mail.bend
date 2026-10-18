import Base
import ./charter.bend as CH

# Durable mail metadata. No IO and no dependency on the product model.

type Origin is Data:
  Origin{from: String, kind: String, ref: String}

def Origin.from(+x: Origin) -> String:
  match x:
    case Origin{+v, _, _}:
      v

def Origin.kind(+x: Origin) -> String:
  match x:
    case Origin{_, +v, _}:
      v

def Origin.ref(+x: Origin) -> String:
  match x:
    case Origin{_, _, +v}:
      v

type Conv is Data:
  Conv{id: String, subject: String, kind: String, parts: List<&2, String>, project: String, tags: List<&2, String>, made: Nat, last: Nat, n: Nat}

def Conv.id(+x: Conv) -> String:
  match x:
    case Conv{+v, _, _, _, _, _, _, _, _}:
      v

def Conv.subject(+x: Conv) -> String:
  match x:
    case Conv{_, +v, _, _, _, _, _, _, _}:
      v

def Conv.kind(+x: Conv) -> String:
  match x:
    case Conv{_, _, +v, _, _, _, _, _, _}:
      v

def Conv.parts(+x: Conv) -> List<&2, String>:
  match x:
    case Conv{_, _, _, +v, _, _, _, _, _}:
      v

def Conv.project(+x: Conv) -> String:
  match x:
    case Conv{_, _, _, _, +v, _, _, _, _}:
      v

def Conv.tags(+x: Conv) -> List<&2, String>:
  match x:
    case Conv{_, _, _, _, _, +v, _, _, _}:
      v

def Conv.made(+x: Conv) -> Nat:
  match x:
    case Conv{_, _, _, _, _, _, +v, _, _}:
      v

def Conv.last(+x: Conv) -> Nat:
  match x:
    case Conv{_, _, _, _, _, _, _, +v, _}:
      v

def Conv.n(+x: Conv) -> Nat:
  match x:
    case Conv{_, _, _, _, _, _, _, _, +v}:
      v

type Mail is Data:
  Mail{id: String, conv: String, from: String, to: String, kind: String, ref: String, want: String, head: String, size: Nat, at: Nat}

def Mail.id(+x: Mail) -> String:
  match x:
    case Mail{+v, _, _, _, _, _, _, _, _, _}:
      v

def Mail.conv(+x: Mail) -> String:
  match x:
    case Mail{_, +v, _, _, _, _, _, _, _, _}:
      v

def Mail.from(+x: Mail) -> String:
  match x:
    case Mail{_, _, +v, _, _, _, _, _, _, _}:
      v

def Mail.to(+x: Mail) -> String:
  match x:
    case Mail{_, _, _, +v, _, _, _, _, _, _}:
      v

def Mail.kind(+x: Mail) -> String:
  match x:
    case Mail{_, _, _, _, +v, _, _, _, _, _}:
      v

def Mail.ref(+x: Mail) -> String:
  match x:
    case Mail{_, _, _, _, _, +v, _, _, _, _}:
      v

def Mail.wanted(+x: Mail) -> String:
  match x:
    case Mail{_, _, _, _, _, _, +v, _, _, _}:
      v

def Mail.header(+x: Mail) -> String:
  match x:
    case Mail{_, _, _, _, _, _, _, +v, _, _}:
      v

def Mail.size(+x: Mail) -> Nat:
  match x:
    case Mail{_, _, _, _, _, _, _, _, +v, _}:
      v

def Mail.at(+x: Mail) -> Nat:
  match x:
    case Mail{_, _, _, _, _, _, _, _, _, +v}:
      v

type Mark is Data:
  Mark{bot: String, conv: String, status: String, seen: Nat, until: Nat, tags: List<&2, String>}

def Mark.bot(+x: Mark) -> String:
  match x:
    case Mark{+v, _, _, _, _, _}:
      v

def Mark.conv(+x: Mark) -> String:
  match x:
    case Mark{_, +v, _, _, _, _}:
      v

def Mark.status(+x: Mark) -> String:
  match x:
    case Mark{_, _, +v, _, _, _}:
      v

def Mark.seen(+x: Mark) -> Nat:
  match x:
    case Mark{_, _, _, +v, _, _}:
      v

def Mark.until(+x: Mark) -> Nat:
  match x:
    case Mark{_, _, _, _, +v, _}:
      v

def Mark.tags(+x: Mark) -> List<&2, String>:
  match x:
    case Mark{_, _, _, _, _, +v}:
      v

type Run is Data:
  Run{thread: String, bot: String, conv: String, mail: String, at: Nat, live: Bool}

def Run.thread(+x: Run) -> String:
  match x:
    case Run{+v, _, _, _, _, _}:
      v

def Run.bot(+x: Run) -> String:
  match x:
    case Run{_, +v, _, _, _, _}:
      v

def Run.conv(+x: Run) -> String:
  match x:
    case Run{_, _, +v, _, _, _}:
      v

def Run.mail(+x: Run) -> String:
  match x:
    case Run{_, _, _, +v, _, _}:
      v

def Run.at(+x: Run) -> Nat:
  match x:
    case Run{_, _, _, _, +v, _}:
      v

def Run.live(+x: Run) -> Bool:
  match x:
    case Run{_, _, _, _, _, +v}:
      v

type Decision is Data:
  Decision{id: String, bot: String, conv: String, run: String, text: String, reason: String, at: Nat}

def Decision.id(+x: Decision) -> String:
  match x:
    case Decision{+v, _, _, _, _, _, _}:
      v

def Decision.bot(+x: Decision) -> String:
  match x:
    case Decision{_, +v, _, _, _, _, _}:
      v

def Decision.conv(+x: Decision) -> String:
  match x:
    case Decision{_, _, +v, _, _, _, _}:
      v

def Decision.run(+x: Decision) -> String:
  match x:
    case Decision{_, _, _, +v, _, _, _}:
      v

def Decision.text(+x: Decision) -> String:
  match x:
    case Decision{_, _, _, _, +v, _, _}:
      v

def Decision.reason(+x: Decision) -> String:
  match x:
    case Decision{_, _, _, _, _, +v, _}:
      v

def Decision.at(+x: Decision) -> Nat:
  match x:
    case Decision{_, _, _, _, _, _, +v}:
      v

type Flags is Data:
  Flags{outcome: String, own: Bool, direct: Bool, mentioned: Bool}

def Flags.outcome(+x: Flags) -> String:
  match x:
    case Flags{+v, _, _, _}:
      v

def Flags.own(+x: Flags) -> Bool:
  match x:
    case Flags{_, +v, _, _}:
      v

def Flags.direct(+x: Flags) -> Bool:
  match x:
    case Flags{_, _, +v, _}:
      v

def Flags.mentioned(+x: Flags) -> Bool:
  match x:
    case Flags{_, _, _, +v}:
      v

def Conv.empty() -> Conv:
  Conv{"", "", "", Nil{}, "", Nil{}, 0n, 0n, 0n}

def Mark.empty() -> Mark:
  Mark{"", "", "open", 0n, 0n, Nil{}}

def Run.empty() -> Run:
  Run{"", "", "", "", 0n, False{}}

def Get.conv.got(r: Map<&2, Conv> & Conv) -> Conv:
  (_, v) = r
  v

def Get.mark.got(r: Map<&2, Mark> & Mark) -> Mark:
  (_, v) = r
  v

def Get.nat.got(r: Map<&2, Nat> & Nat) -> Nat:
  (_, v) = r
  v

def Get.nat(+m: Map<&2, Nat>, +k: String) -> Nat:
  Get.nat.got(Map.get(Nat, 0n, m, k))

def Get.str.got(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

def Get.str(+m: Map<&2, String>, +k: String) -> String:
  Get.str.got(Map.get(String, "", m, k))

def Get.has(r: Map<&2, Bool> & Bool) -> Bool:
  (_, v) = r
  v

def Max(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_ge(a, b), a, b)

# Base Map is a crit-bit trie. It has sorted traversal but no rank/seek
# iterator; keys/to_list materialize the entire map. A per-bot AVL with
# subtree counts provides O(log n) moves and O(log n + page) rank slices.
# Keys compare (last descending, id ascending); Nat timestamps need no cap.
type Order is Data:
  OEmpty{}
  ONode{height: Nat, count: Nat, at: Nat, id: String, left: Order, right: Order}

type Box is Data:
  Box{convs: Map<&2, Conv>, mails: Map<&2, List<&2, Mail>>, marks: Map<&2, Mark>, order: Map<&2, Order>, counts: Map<&2, Nat>, total: Nat, origins: Map<&2, Origin>, runs: Map<&2, Run>, live: Map<&2, String>, live_counts: Map<&2, Nat>, charters: Map<&2, String>, decisions: Map<&2, List<&2, Decision>>, ids: Map<&2, Bool>, names: Map<&2, String>, snoozes: Order}

def Box.convs(+x: Box) -> Map<&2, Conv>:
  match x:
    case Box{+v, _, _, _, _, _, _, _, _, _, _, _, _, _, +sz}:
      v

def Box.mails(+x: Box) -> Map<&2, List<&2, Mail>>:
  match x:
    case Box{_, +v, _, _, _, _, _, _, _, _, _, _, _, _, +sz}:
      v

def Box.marks(+x: Box) -> Map<&2, Mark>:
  match x:
    case Box{_, _, +v, _, _, _, _, _, _, _, _, _, _, _, +sz}:
      v

def Box.order(+x: Box) -> Map<&2, Order>:
  match x:
    case Box{_, _, _, +v, _, _, _, _, _, _, _, _, _, _, +sz}:
      v

def Box.counts(+x: Box) -> Map<&2, Nat>:
  match x:
    case Box{_, _, _, _, +v, _, _, _, _, _, _, _, _, _, +sz}:
      v

def Box.total(+x: Box) -> Nat:
  match x:
    case Box{_, _, _, _, _, +v, _, _, _, _, _, _, _, _, +sz}:
      v

def Box.origins(+x: Box) -> Map<&2, Origin>:
  match x:
    case Box{_, _, _, _, _, _, +v, _, _, _, _, _, _, _, +sz}:
      v

def Box.runs(+x: Box) -> Map<&2, Run>:
  match x:
    case Box{_, _, _, _, _, _, _, +v, _, _, _, _, _, _, +sz}:
      v

def Box.active(+x: Box) -> Map<&2, String>:
  match x:
    case Box{_, _, _, _, _, _, _, _, +v, _, _, _, _, _, +sz}:
      v

def Box.live_counts(+x: Box) -> Map<&2, Nat>:
  match x:
    case Box{_, _, _, _, _, _, _, _, _, +v, _, _, _, _, +sz}:
      v

def Box.charters(+x: Box) -> Map<&2, String>:
  match x:
    case Box{_, _, _, _, _, _, _, _, _, _, +v, _, _, _, +sz}:
      v

def Box.decision_log(+x: Box) -> Map<&2, List<&2, Decision>>:
  match x:
    case Box{_, _, _, _, _, _, _, _, _, _, _, +v, _, _, +sz}:
      v

def Box.ids(+x: Box) -> Map<&2, Bool>:
  match x:
    case Box{_, _, _, _, _, _, _, _, _, _, _, _, +v, _, +sz}:
      v

def Box.with_convs(+x: Box, +v: Map<&2, Conv>) -> Box:
  match x:
    case Box{_, +x1, +x2, +x3, +x4, +x5, +x6, +x7, +x8, +x9, +x10, +x11, +x12, +names, +sz}:
      Box{v, x1, x2, x3, x4, x5, x6, x7, x8, x9, x10, x11, x12, names, sz}

def Box.with_mails(+x: Box, +v: Map<&2, List<&2, Mail>>) -> Box:
  match x:
    case Box{+x0, _, +x2, +x3, +x4, +x5, +x6, +x7, +x8, +x9, +x10, +x11, +x12, +names, +sz}:
      Box{x0, v, x2, x3, x4, x5, x6, x7, x8, x9, x10, x11, x12, names, sz}

def Box.with_marks(+x: Box, +v: Map<&2, Mark>) -> Box:
  match x:
    case Box{+x0, +x1, _, +x3, +x4, +x5, +x6, +x7, +x8, +x9, +x10, +x11, +x12, +names, +sz}:
      Box{x0, x1, v, x3, x4, x5, x6, x7, x8, x9, x10, x11, x12, names, sz}

def Box.with_order(+x: Box, +v: Map<&2, Order>) -> Box:
  match x:
    case Box{+x0, +x1, +x2, _, +x4, +x5, +x6, +x7, +x8, +x9, +x10, +x11, +x12, +names, +sz}:
      Box{x0, x1, x2, v, x4, x5, x6, x7, x8, x9, x10, x11, x12, names, sz}

def Box.with_counts(+x: Box, +v: Map<&2, Nat>) -> Box:
  match x:
    case Box{+x0, +x1, +x2, +x3, _, +x5, +x6, +x7, +x8, +x9, +x10, +x11, +x12, +names, +sz}:
      Box{x0, x1, x2, x3, v, x5, x6, x7, x8, x9, x10, x11, x12, names, sz}

def Box.with_total(+x: Box, +v: Nat) -> Box:
  match x:
    case Box{+x0, +x1, +x2, +x3, +x4, _, +x6, +x7, +x8, +x9, +x10, +x11, +x12, +names, +sz}:
      Box{x0, x1, x2, x3, x4, v, x6, x7, x8, x9, x10, x11, x12, names, sz}

def Box.with_origins(+x: Box, +v: Map<&2, Origin>) -> Box:
  match x:
    case Box{+x0, +x1, +x2, +x3, +x4, +x5, _, +x7, +x8, +x9, +x10, +x11, +x12, +names, +sz}:
      Box{x0, x1, x2, x3, x4, x5, v, x7, x8, x9, x10, x11, x12, names, sz}

def Box.with_runs(+x: Box, +v: Map<&2, Run>) -> Box:
  match x:
    case Box{+x0, +x1, +x2, +x3, +x4, +x5, +x6, _, +x8, +x9, +x10, +x11, +x12, +names, +sz}:
      Box{x0, x1, x2, x3, x4, x5, x6, v, x8, x9, x10, x11, x12, names, sz}

def Box.with_active(+x: Box, +v: Map<&2, String>) -> Box:
  match x:
    case Box{+x0, +x1, +x2, +x3, +x4, +x5, +x6, +x7, _, +x9, +x10, +x11, +x12, +names, +sz}:
      Box{x0, x1, x2, x3, x4, x5, x6, x7, v, x9, x10, x11, x12, names, sz}

def Box.with_live_counts(+x: Box, +v: Map<&2, Nat>) -> Box:
  match x:
    case Box{+x0, +x1, +x2, +x3, +x4, +x5, +x6, +x7, +x8, _, +x10, +x11, +x12, +names, +sz}:
      Box{x0, x1, x2, x3, x4, x5, x6, x7, x8, v, x10, x11, x12, names, sz}

def Box.with_charters(+x: Box, +v: Map<&2, String>) -> Box:
  match x:
    case Box{+x0, +x1, +x2, +x3, +x4, +x5, +x6, +x7, +x8, +x9, _, +x11, +x12, +names, +sz}:
      Box{x0, x1, x2, x3, x4, x5, x6, x7, x8, x9, v, x11, x12, names, sz}

def Box.with_decision_log(+x: Box, +v: Map<&2, List<&2, Decision>>) -> Box:
  match x:
    case Box{+x0, +x1, +x2, +x3, +x4, +x5, +x6, +x7, +x8, +x9, +x10, _, +x12, +names, +sz}:
      Box{x0, x1, x2, x3, x4, x5, x6, x7, x8, x9, x10, v, x12, names, sz}

def Box.with_ids(+x: Box, +v: Map<&2, Bool>) -> Box:
  match x:
    case Box{+x0, +x1, +x2, +x3, +x4, +x5, +x6, +x7, +x8, +x9, +x10, +x11, _, +names, +sz}:
      Box{x0, x1, x2, x3, x4, x5, x6, x7, x8, x9, x10, x11, v, names, sz}

# Two aliases per bot: current name -> id, and id -> current name.
# Replay of BotSet/BotDeleted maintains them without enumerating World.bots.
def Box.names(+x: Box) -> Map<&2, String>:
  match x:
    case Box{_, _, _, _, _, _, _, _, _, _, _, _, _, +names, +sz}: names

def Box.with_names(+x: Box, +names: Map<&2, String>) -> Box:
  match x:
    case Box{+a, +b, +c, +d, +e, +f, +g, +h, +i, +j, +k, +l, +m, _, +sz}:
      Box{a, b, c, d, e, f, g, h, i, j, k, l, m, names, sz}

# ---- mail 2b ----
def Box.snoozes(b: Box) -> Order:
  match b:
    case Box{_, _, _, _, _, _, _, _, _, _, _, _, _, _, +sz}: sz

def Box.with_snoozes(b: Box, +sz: Order) -> Box:
  match b:
    case Box{+a, +c, +d, +e, +f, +g, +h, +i, +j, +k, +l, +m, +n, +o, _}: Box{a, c, d, e, f, g, h, i, j, k, l, m, n, o, sz}

def Box.bot_deleted(+b: Box, +id: String) -> Box:
  Box.with_names(b, Map.del(&2, String, Map.del(&2, String, Box.names(b), "name:" ++ Get.str(Box.names(b), "id:" ++ id)), "id:" ++ id))

def Box.bot_named_store(+b: Box, +id: String, +name: String) -> Box:
  Box.with_names(b, Map.set(&2, String, Map.set(&2, String, Box.names(b), "id:" ++ id, String.to_lower(name)), "name:" ++ String.to_lower(name), id))

def Box.bot_named(+b: Box, +id: String, +name: String) -> Box:
  Box.bot_named_store(Box.bot_deleted(b, id), id, name)

def Box.sender_id(+known: Bool, +from: String) -> String:
  match known:
    case True{}: "bot:" ++ from
    case False{}: from

def Box.sender_named(+found: Bool, +id: String, +b: Box, +from: String) -> String:
  match found:
    case True{}: "bot:" ++ id
    case False{}: Box.sender_id(Bool.not(String.is_empty(Get.str(Box.names(b), "id:" ++ from))), from)

def Box.sender_lookup(+b: Box, +from: String) -> String:
  +id = Get.str(Box.names(b), "name:" ++ String.to_lower(from))
  Box.sender_named(Bool.not(String.is_empty(id)), id, b, from)

def Box.sender_person(person: Bool, +b: Box, +from: String) -> String:
  match person:
    case True{}: "person"
    case False{}: Box.sender_lookup(b, from)

def Box.sender(+b: Box, +from: String) -> String:
  Box.sender_person(Bool.or(String.eq(from, "you"), String.eq(from, "person")), b, from)

def Order.height(+t: Order) -> Nat:
  match t:
    case OEmpty{}: 0n
    case ONode{+h, _, _, _, _, _}: h

def Order.count(+t: Order) -> Nat:
  match t:
    case OEmpty{}: 0n
    case ONode{_, +n, _, _, _, _}: n

def Order.node(+at: Nat, +id: String, +l: Order, +r: Order) -> Order:
  ONode{1n+Max(Order.height(l), Order.height(r)), 1n+Nat.add(Order.count(l), Order.count(r)), at, id, l, r}

def Order.left(+at: Nat, +id: String, +l: Order, +r: Order) -> Order:
  match l:
    case OEmpty{}: Order.node(at, id, OEmpty{}, r)
    case ONode{_, _, +a, +i, +x, +y}:
      Order.node(a, i, x, Order.node(at, id, y, r))

def Order.right(+at: Nat, +id: String, +l: Order, +r: Order) -> Order:
  match r:
    case OEmpty{}: Order.node(at, id, l, OEmpty{})
    case ONode{_, _, +a, +i, +x, +y}:
      Order.node(a, i, Order.node(at, id, l, x), y)

def Order.left_ready(+double: Bool, +at: Nat, +id: String, +l: Order, +r: Order) -> Order:
  match double l:
    case True{} ONode{_, _, +a, +i, +x, +y}:
      Order.left(at, id, Order.right(a, i, x, y), r)
    case _ +tree:
      Order.left(at, id, tree, r)

def Order.right_ready(+double: Bool, +at: Nat, +id: String, +l: Order, +r: Order) -> Order:
  match double r:
    case True{} ONode{_, _, +a, +i, +x, +y}:
      Order.right(at, id, l, Order.left(a, i, x, y))
    case _ +tree:
      Order.right(at, id, l, tree)

def Order.left_double(+t: Order) -> Bool:
  match t:
    case OEmpty{}: False{}
    case ONode{_, _, _, _, +l, +r}: Nat.is_lt(Order.height(l), Order.height(r))

def Order.right_double(+t: Order) -> Bool:
  match t:
    case OEmpty{}: False{}
    case ONode{_, _, _, _, +l, +r}: Nat.is_gt(Order.height(l), Order.height(r))

def Order.balance.k(+lheavy: Bool, +rheavy: Bool, +at: Nat, +id: String, +l: Order, +r: Order) -> Order:
  match lheavy rheavy:
    case True{} _: Order.left_ready(Order.left_double(l), at, id, l, r)
    case False{} True{}: Order.right_ready(Order.right_double(r), at, id, l, r)
    case False{} False{}: Order.node(at, id, l, r)

def Order.balance(+at: Nat, +id: String, +l: Order, +r: Order) -> Order:
  Order.balance.k(Nat.is_gt(Order.height(l), 1n+Order.height(r)), Nat.is_gt(Order.height(r), 1n+Order.height(l)), at, id, l, r)

def Order.cmp.k(+c: Cmp, +a: String, +b: String) -> Cmp:
  match c:
    case EQ{}: String.order(a, b)
    case LT{}: GT{}
    case GT{}: LT{}

def Order.cmp(+a: Nat, +i: String, +b: Nat, +j: String) -> Cmp:
  Order.cmp.k(Nat.cmp(a, b), i, j)

def Order.root_cmp(+t: Order, +at: Nat, +id: String) -> Cmp:
  match t:
    case OEmpty{}: EQ{}
    case ONode{_, _, +a, +i, _, _}: Order.cmp(at, id, a, i)

def Order.insert(+t: Order, +c: Cmp, +at: Nat, +id: String) -> Order:
  match t c:
    case OEmpty{} _: Order.node(at, id, OEmpty{}, OEmpty{})
    case ONode{_, _, +a, +i, +l, +r} LT{}:
      Order.balance(a, i, Order.insert(l, Order.root_cmp(l, at, id), at, id), r)
    case ONode{_, _, _, _, +l, +r} EQ{}: Order.node(at, id, l, r)
    case ONode{_, _, +a, +i, +l, +r} GT{}:
      Order.balance(a, i, l, Order.insert(r, Order.root_cmp(r, at, id), at, id))

def Order.put(+t: Order, +at: Nat, +id: String) -> Order:
  Order.insert(t, Order.root_cmp(t, at, id), at, id)

# Removing the leftmost node returns its key and the balanced remainder.
type OPop is Data:
  OPop{at: Nat, id: String, rest: Order}

def Order.pop_up(+at: Nat, +id: String, +r: Order, +p: OPop) -> OPop:
  match p:
    case OPop{+a, +i, +l}: OPop{a, i, Order.balance(at, id, l, r)}

def Order.pop(+t: Order) -> OPop:
  match t:
    case OEmpty{}: OPop{0n, "", OEmpty{}}
    case ONode{_, _, +a, +i, OEmpty{}, +r}: OPop{a, i, r}
    case ONode{_, _, +a, +i, +l, +r}: Order.pop_up(a, i, r, Order.pop(l))

def Order.join_pop(+l: Order, +p: OPop) -> Order:
  match p:
    case OPop{+a, +i, +r}: Order.balance(a, i, l, r)

def Order.join(+l: Order, +r: Order) -> Order:
  match r:
    case OEmpty{}: l
    case +tree: Order.join_pop(l, Order.pop(tree))

def Order.delete(+t: Order, +c: Cmp, +at: Nat, +id: String) -> Order:
  match t c:
    case OEmpty{} _: OEmpty{}
    case ONode{_, _, +a, +i, +l, +r} LT{}:
      Order.balance(a, i, Order.delete(l, Order.root_cmp(l, at, id), at, id), r)
    case ONode{_, _, _, _, +l, +r} EQ{}: Order.join(l, r)
    case ONode{_, _, +a, +i, +l, +r} GT{}:
      Order.balance(a, i, l, Order.delete(r, Order.root_cmp(r, at, id), at, id))

def Order.drop(+t: Order, +at: Nat, +id: String) -> Order:
  Order.delete(t, Order.root_cmp(t, at, id), at, id)

# Rank seek skips whole subtrees; a bounded explicit spine then visits
# only the requested page. Fuel is a termination bound, never traversed.
type OCursor is Data:
  OCursor{todo: List<&2, Order>, skip: Nat, n: Nat, acc: List<&2, String>}

def Order.page_step(+c: Cmp, +a: Nat, +i: String, +l: Order, +r: Order, +rest: List<&2, Order>, +skip: Nat, +n: Nat, +acc: List<&2, String>) -> OCursor:
  match c:
    case LT{}: OCursor{l <> Order.node(a, i, OEmpty{}, r) <> rest, skip, n, acc}
    case EQ{}: OCursor{r <> rest, 0n, Nat.sub(n, 1n), i <> acc}
    case GT{}: OCursor{r <> rest, Nat.sub(skip, 1n+Order.count(l)), n, acc}

def Order.page(+fuel: Nat, +cur: OCursor) -> List<&2, String>:
  match fuel cur:
    case 0n OCursor{_, _, _, +acc}: List.reverse(&2, String, acc)
    case 1n+f OCursor{_, _, 0n, +acc}: List.reverse(&2, String, acc)
    case 1n+f OCursor{Nil{}, _, _, +acc}: List.reverse(&2, String, acc)
    case 1n+f OCursor{OEmpty{} <> rest, +skip, +n, +acc}:
      Order.page(f, OCursor{rest, skip, n, acc})
    case 1n+f OCursor{ONode{_, _, +a, +i, +l, +r} <> rest, +skip, +n, +acc}:
      Order.page(f, Order.page_step(Nat.cmp(skip, Order.count(l)), a, i, l, r, rest, skip, n, acc))

def Order.slice(+t: Order, +skip: Nat, +n: Nat) -> List<&2, String>:
  Order.page(1n+Nat.mul(3n, Order.count(t)), OCursor{[t], skip, n, Nil{}})

def Addr.thread(+s: String) -> String:
  Bool.pick(String, String.starts_with(s, "thread:"), String.drop(s, 7n), s)

def Addr.bot(+s: String) -> String:
  Bool.pick(String, String.starts_with(s, "bot:"), String.drop(s, 4n), s)

def Addr.of(+s: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(s), Bool.or(String.eq(s, "person"), String.eq(s, "you"))), "person",
    Bool.pick(String, String.eq(s, "backplane"), "backplane", Bool.pick(String, String.contains(s, ":"), s, "thread:" ++ s)))

def Addr.dm(+s: String) -> String:
  Bool.pick(String, Bool.or(String.eq(s, "you"), String.eq(s, "person")), "person", Bool.pick(String, String.contains(s, ":"), s, "bot:" ++ s))

# Read only a short prefix when extracting today's envelope labels.
def Prefix.until_go(+s: String, +end: Char, +acc: String, +done: Bool) -> String:
  match s done:
    case _ True{}: String.reverse(acc)
    case SNil{} False{}: String.reverse(acc)
    case SCon{+c, +t} False{}:
      Prefix.until_go(t, end, Bool.pick(String, Char.is_eq(c, end), acc, SCon{c, acc}), Char.is_eq(c, end))

def Prefix.until(+s: String, +end: Char) -> String:
  Prefix.until_go(s, end, "", False{})

def Mail.classify_base(+from: String, +system: Bool) -> Origin:
  match system:
    case True{}:
      Origin{Bool.pick(String, String.is_empty(from), "backplane", Addr.of(from)), Bool.pick(String, String.is_empty(from), "system", Bool.pick(String, String.starts_with(from, "bot:"), "bot", "thread")), ""}
    case False{}:
      Origin{Bool.pick(String, String.is_empty(from), "person", Addr.of(from)), Bool.pick(String, String.is_empty(from), "person", Bool.pick(String, String.starts_with(from, "bot:"), "bot", "thread")), ""}

def Mail.classify_choose(+hit: Bool, +o: Origin, +fallback: Origin) -> Origin:
  match hit:
    case True{}: o
    case False{}: fallback

def Mail.envelope_from(+from: String, +label: String) -> String:
  +name = Prefix.until(String.drop(label, 14n), Chr{32})
  Bool.pick(String, Bool.not(String.is_empty(from)), Addr.of(from),
    Bool.pick(String, Bool.or(String.eq(name, "you"), String.starts_with(name, "person@")), "person",
      Bool.pick(String, String.contains(name, "@"), "peer:" ++ Maybe.default(&2, String, List.last(&2, String, String.split(name, Chr{64})), ""), "bot:" ++ name)))

def Mail.direct_unlinked(+name: String) -> Origin:
  Origin{Bool.pick(String, String.starts_with(name, "person@"), "person", Bool.pick(String, String.contains(name, "@"), "peer:" ++ name, "bot:" ++ name)),
    Bool.pick(String, String.starts_with(name, "person@"), "person", Bool.pick(String, String.contains(name, "@"), "peer", "bot")), ""}

def Mail.direct_linked(linked: Bool, +from: String, +label: String) -> Origin:
  match linked:
    case True{}: Mail.classify_base(from, False{})
    case False{}: Mail.direct_unlinked(String.drop(label, 14n))

def Mail.direct_origin(+from: String, +label: String) -> Origin:
  Mail.direct_linked(Bool.not(String.is_empty(from)), from, label)

# Bodies can quote envelopes. Only unlinked incoming bot envelopes may
# identify the person; a known thread sender cannot be erased by its text.
def Mail.wake_shape(+msg: String, +head: String) -> Bool:
  Bool.and(String.contains(msg, ":lr-"), Bool.and(String.starts_with(head, "Wake "), Bool.and(String.contains(head, ":"), Bool.and(Bool.not(String.is_empty(Prefix.until(String.drop(head, 5n), Chr{58}))), Bool.not(String.contains(Prefix.until(String.drop(head, 5n), Chr{58}), " "))))))

def Mail.classify(+from: String, +msg: String, +system: Bool, +text: String) -> Origin:
  +head = String.take(text, 160n)
  +label = Prefix.until(head, Chr{93})
  +room = Maybe.default(&2, String, List.last(&2, String, String.split(label, Chr{35})), "")
  Mail.classify_choose(Bool.and(String.starts_with(head, "[message from "), Bool.not(String.contains(label, " in #"))), Mail.direct_origin(from, label),
  Mail.classify_choose(String.starts_with(head, "Backplane restarted while you were working"), Origin{"backplane", "carry", ""},
  Mail.classify_choose(Bool.and(String.starts_with(head, "[message from "), String.contains(label, " in #")), Origin{Mail.envelope_from(from, label), "room", room},
  Mail.classify_choose(String.starts_with(head, "[routine: "), Origin{"routine:" ++ String.drop(label, 10n), "routine", String.drop(label, 10n)},
  Mail.classify_choose(String.starts_with(head, "[webhook: "), Origin{"hook:" ++ String.drop(label, 10n), "hook", String.drop(label, 10n)},
  Mail.classify_choose(String.starts_with(msg, "t:backplane:l"), Origin{"backplane", "lost", ""},
  Mail.classify_choose(Mail.wake_shape(msg, head), Origin{Addr.of(from), "wake", Prefix.until(String.drop(head, 5n), Chr{58})},
  Mail.classify_choose(String.eq(from, "deploy"), Origin{"backplane", "deploy", ""},
  Mail.classify_choose(Bool.or(String.starts_with(from, "lock:"), String.starts_with(msg, "t:lock:")), Origin{"backplane", "lock", Bool.pick(String, String.starts_with(from, "lock:"), String.drop(from, 5n), Prefix.until(String.drop(msg, 7n), Chr{58}))},
  Mail.classify_choose(Bool.or(String.contains(msg, ":td-"), String.starts_with(head, "[Backplane] Delegated task")), Origin{Addr.of(from), "task", ""},
  Mail.classify_choose(Bool.or(String.contains(msg, ":lr-"), String.starts_with(head, "[Backplane] Launched thread")), Origin{Addr.of(from), "launch", ""},
    Mail.classify_base(from, system))))))))))))

def Mail.pair_k(+c: Cmp, +a: String, +b: String) -> String:
  match c:
    case GT{}: "pair:" ++ b ++ "|" ++ a
    case _: "pair:" ++ a ++ "|" ++ b

def Mail.pair(+a: String, +b: String) -> String:
  Mail.pair_k(String.order(a, b), a, b)

def Mail.dm(+parts: List<&2, String>, +fallback: String) -> String:
  match parts:
    case +a <> +b <> Nil{}: Mail.pair(Addr.dm(a), Addr.dm(b))
    case _: fallback

def Mail.kind_code(+kind: String) -> Nat:
  Bool.pick(Nat, String.eq(kind, "person"), 0n, Bool.pick(Nat, String.eq(kind, "thread"), 1n, Bool.pick(Nat, String.eq(kind, "bot"), 2n, Bool.pick(Nat, String.eq(kind, "room"), 3n, Bool.pick(Nat, String.eq(kind, "launch"), 4n, Bool.pick(Nat, String.eq(kind, "wake"), 5n, Bool.pick(Nat, String.eq(kind, "lost"), 6n, Bool.pick(Nat, String.eq(kind, "task"), 7n, Bool.pick(Nat, String.eq(kind, "routine"), 8n, Bool.pick(Nat, String.eq(kind, "hook"), 9n, Bool.pick(Nat, String.eq(kind, "deploy"), 10n, Bool.pick(Nat, String.eq(kind, "lock"), 11n, Bool.pick(Nat, String.eq(kind, "carry"), 12n, Bool.pick(Nat, String.eq(kind, "peer"), 13n, Bool.pick(Nat, String.eq(kind, "reminder"), 14n, Bool.pick(Nat, String.eq(kind, "system"), 15n, 99n))))))))))))))))

def Mail.route_kind(k: Nat, +from: String, +ref: String, +to: String) -> String:
  match k:
    case 0n: ""
    case 1n: Mail.pair(from, to)
    case 2n: Mail.pair(from, to)
    case 3n:
      Bool.pick(String, String.starts_with(ref, "dm:"), Mail.dm(String.split(String.drop(ref, 3n), Chr{58}), "room:" ++ ref), "room:" ++ ref)
    case 4n: "launch:" ++ Addr.thread(from)
    case 7n: "launch:" ++ Addr.thread(from)
    case 5n: Bool.pick(String, String.is_empty(ref), "launch:" ++ Addr.thread(from), "wake:" ++ ref)
    case 8n: "routine:" ++ ref
    case 9n: "hook:" ++ ref
    case 13n: "peer:" ++ Bool.pick(String, String.starts_with(from, "peer:"), String.drop(from, 5n), from) ++ ":" ++ ref
    case _: "system:" ++ Addr.bot(to)

def Mail.route(o: Origin, to: String) -> String:
  match o:
    case Origin{from, kind, ref}: Mail.route_kind(Mail.kind_code(kind), from, ref, to)

def Mail.want_kind(k: Nat, flags: Flags) -> String:
  match k flags:
    case 4n Flags{outcome, _, _, _}:
      Bool.pick(String, String.eq(outcome, "completed"), "fyi", "act")
    case 7n Flags{_, own, _, _}: Bool.pick(String, own, "act", "fyi")
    case 5n _: "act"
    case 6n _: "act"
    case 8n _: "act"
    case 9n _: "act"
    case 10n Flags{_, own, _, _}: Bool.pick(String, own, "act", "fyi")
    case 11n Flags{_, own, _, _}: Bool.pick(String, own, "act", "fyi")
    case 3n Flags{_, _, _, mentioned}: Bool.pick(String, mentioned, "act", "fyi")
    case 0n Flags{_, _, direct, _}: Bool.pick(String, direct, "act", "fyi")
    case _ Flags{_, own, direct, mentioned}: Bool.pick(String, Bool.or(own, Bool.or(direct, mentioned)), "act", "fyi")

def Mail.want_default(kind: String, flags: Flags) -> String:
  Mail.want_kind(Mail.kind_code(kind), flags)

def Mail.want_given(fyi: Bool, act: Bool, kind: String, flags: Flags) -> String:
  match fyi act:
    case True{} _: "fyi"
    case False{} True{}: "act"
    case False{} False{}: Mail.want_default(kind, flags)

def Mail.want(kind: String, +given: String, flags: Flags) -> String:
  Mail.want_given(String.eq(given, "fyi"), String.eq(given, "act"), kind, flags)

def Mail.head_go(+n: Nat, +s: String, +acc: String) -> String:
  match n s:
    case 0n _: String.reverse(acc)
    case _ SNil{}: String.reverse(acc)
    case 1n+k SCon{+c, +t}:
      match c:
        case Chr{10}: String.reverse(acc)
        case Chr{9}: Mail.head_go(k, t, SCon{Chr{32}, acc})
        case Chr{13}: Mail.head_go(k, t, SCon{Chr{32}, acc})
        case _: Mail.head_go(k, t, SCon{c, acc})

def Mail.head(+text: String) -> String:
  Mail.head_go(160n, text, "")

def Mail.size_go(text: String, +n: Nat) -> Nat:
  match text:
    case SNil{}: n
    case SCon{_, tail}: Mail.size_go(tail, 1n+n)

def Mail.text_size(text: String) -> Nat:
  Mail.size_go(text, 0n)

def Box.empty() -> Box:
  Box{Map.new(&2, Conv), Map.new(&2, List<&2, Mail>), Map.new(&2, Mark), Map.new(&2, Order), Map.new(&2, Nat), 0n, Map.new(&2, Origin), Map.new(&2, Run), Map.new(&2, String), Map.new(&2, Nat), Map.new(&2, String), Map.new(&2, List<&2, Decision>), Map.new(&2, Bool), Map.new(&2, String), OEmpty{}}

def Box.conv(+b: Box, +id: String) -> Conv:
  Get.conv.got(Map.get(Conv, Conv.empty(), Box.convs(b), id))

def Box.mark_of(+b: Box, +bot: String, +conv: String) -> Mark:
  Get.mark.got(Map.get(Mark, Mark.empty(), Box.marks(b), bot ++ "/" ++ conv))

def Box.order_got(r: Map<&2, Order> & Order) -> Order:
  (_, v) = r
  v

def Box.order_of(+b: Box, +bot: String) -> Order:
  Box.order_got(Map.get(Order, OEmpty{}, Box.order(b), Bool.pick(String, String.starts_with(bot, "thread:"), "*", bot)))

def Box.unread(+b: Box, +bot: String) -> Nat:
  Get.nat(Box.counts(b), bot)

def Box.unread_all(+b: Box) -> Nat:
  Box.total(b)

def Box.mark_count(+b: Box, +m: Mark, +was: Nat, +now: Nat) -> Box:
  Box.with_total(Box.with_counts(b, Map.set(&2, Nat, Box.counts(b), Mark.bot(m), Nat.add(Nat.sub(Box.unread(b, Mark.bot(m)), was), now))), Nat.add(Nat.sub(Box.total(b), was), now))

def Mark.unread(+m: Mark) -> Nat:
  Bool.pick(Nat, String.eq(Mark.status(m), "unread"), 1n, 0n)

def Snooze.key(+m: Mark) -> String:
  Mark.bot(m) ++ "/" ++ Mark.conv(m)

def Snooze.put(+yes: Bool, +tree: Order, +m: Mark) -> Order:
  match yes:
    case False{}: tree
    case True{}: Order.put(tree, Mark.until(m), Snooze.key(m))

def Snooze.drop(+yes: Bool, +tree: Order, +m: Mark) -> Order:
  match yes:
    case False{}: tree
    case True{}: Order.drop(tree, Mark.until(m), Snooze.key(m))

def Box.mark(+b: Box, +m: Mark) -> Box:
  +old = Box.mark_of(b, Mark.bot(m), Mark.conv(m))
  Box.with_snoozes(Box.with_marks(Box.mark_count(b, m, Mark.unread(old), Mark.unread(m)), Map.set(&2, Mark, Box.marks(b), Snooze.key(m), m)),
    Snooze.put(String.eq(Mark.status(m), "snoozed"), Snooze.drop(String.eq(Mark.status(old), "snoozed"), Box.snoozes(b), old), m))

def Box.marked(+b: Box, +bot: String, +conv: String, +status: String, +seen: Nat, +until: Nat) -> Box:
  Box.mark(b, Mark{bot, conv, status, seen, until, Mark.tags(Box.mark_of(b, bot, conv))})

def Box.tagged(+b: Box, +bot: String, +conv: String, +tags: List<&2, String>) -> Box:
  +m = Box.mark_of(b, bot, conv)
  Box.mark(b, Mark{bot, conv, Mark.status(m), Mark.seen(m), Mark.until(m), tags})

def Parts.has(+xs: List<&2, String>, +key: String, +found: Bool) -> Bool:
  match xs found:
    case _ True{}: True{}
    case Nil{} False{}: False{}
    case +h <> t False{}: Parts.has(t, key, String.eq(h, key))

def Parts.add(+xs: List<&2, String>, +key: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, Parts.has(xs, key, False{}), xs, key <> xs)

def Box.part_mark(+new: Bool, +b: Box, +bot: String, +conv: String) -> Box:
  match new:
    case True{}: Box.mark(b, Mark{bot,conv, "open", 0n, 0n, Nil{}})
    case False{}: b

def Box.part(+yes: Bool, +b: Box, +part: String, +c: Conv) -> Box:
  match yes:
    case False{}: b
    case True{}:
      +bot = String.drop(part, 4n)
      Box.part_mark(String.is_empty(Mark.bot(Box.mark_of(b, bot, Conv.id(c)))),
        Box.with_order(b, Map.set(&2, Order, Box.order(b), bot, Order.put(Box.order_of(b, bot), Conv.last(c), Conv.id(c)))), bot, Conv.id(c))

def Box.parts(+parts: List<&2, String>, +b: Box, +c: Conv) -> Box:
  match parts:
    case Nil{}: b
    case +h <> t: Box.parts(t, Box.part(String.starts_with(h, "bot:"),b,h, c), c)

def Box.unpart_mark(+removed: Bool, +b: Box, +bot: String, +id: String) -> Box:
  match removed:
    case False{}: b
    case True{}:
      Box.with_snoozes(Box.with_marks(Box.mark_count(b, Mark{bot, id, "open", 0n, 0n, Nil{}}, Mark.unread(Box.mark_of(b, bot, id)), 0n), Map.del(&2, Mark, Box.marks(b), bot ++ "/" ++ id)), Snooze.drop(String.eq(Mark.status(Box.mark_of(b, bot, id)), "snoozed"), Box.snoozes(b), Box.mark_of(b, bot, id)))

def Box.unpart(+yes: Bool, +b: Box, +part: String, +old: Conv, +parts: List<&2, String>) -> Box:
  match yes:
    case False{}: b
    case True{}:
      +bot = String.drop(part, 4n)
      Box.unpart_mark(Bool.not(Parts.has(parts, part, False{})),
        Box.with_order(b, Map.set(&2, Order, Box.order(b), bot, Order.drop(Box.order_of(b, bot), Conv.last(old), Conv.id(old)))), bot, Conv.id(old))

def Box.unparts(+xs: List<&2, String>, +b: Box, +old: Conv, +parts: List<&2, String>) -> Box:
  match xs:
    case Nil{}: b
    case +h <> t: Box.unparts(t, Box.unpart(String.starts_with(h, "bot:"),b,h,old, parts),old, parts)

def Box.conv_put(+b: Box, +c: Conv) -> Box:
  +old = Box.conv(b, Conv.id(c))
  Box.parts(Conv.parts(c), Box.with_convs(Box.unparts(Conv.parts(old), b, old, Conv.parts(c)), Map.set(&2, Conv, Box.convs(b), Conv.id(c), c)), c)

def Box.conv_set(+b: Box, +id: String, +subject: String, +kind: String, +parts: List<&2, String>, +project: String, +tags: List<&2, String>, +at: Nat) -> Box:
  +old = Box.conv(b, id)
  Box.conv_put(b, Conv{id, subject, kind, parts, project, tags, Bool.pick(Nat, String.is_empty(Conv.id(old)), at, Conv.made(old)), Bool.pick(Nat, String.is_empty(Conv.id(old)), at, Conv.last(old)), Conv.n(old)})

def Box.received_part(+yes: Bool, +b: Box, +part: String, +c: Conv) -> Box:
  match yes:
    case False{}: b
    case True{}:
      +bot = String.drop(part, 4n)
      +m = Box.mark_of(b, bot, Conv.id(c))
      Box.mark(b, Mark{bot, Conv.id(c), "unread", Mark.seen(m), 0n, Mark.tags(m)})

def Box.received(+parts: List<&2, String>, +b: Box, +c: Conv, +from: String) -> Box:
  match parts:
    case Nil{}: b
    case +h <> t:
      Box.received(t, Box.received_part(Bool.and(String.starts_with(h, "bot:"), Bool.not(String.eq(h, from))), b, h, c), c, from)

def Box.mails_got(r: Map<&2, List<&2, Mail>> & List<&2, Mail>) -> List<&2, Mail>:
  (_, v) = r
  v

def Box.headers(+b: Box, +conv: String) -> List<&2, Mail>:
  Box.mails_got(Map.get(List<&2, Mail>, Nil{}, Box.mails(b), conv))

def Conv.source_kind(k: Nat) -> String:
  match k:
    case 0n: "pair"
    case 1n: "pair"
    case 2n: "pair"
    case 3n: "room"
    case 4n: "launch"
    case 7n: "launch"
    case 5n: "wake"
    case 8n: "routine"
    case 9n: "hook"
    case 13n: "peer"
    case _: "system"

def Conv.mail_kind(+id: String, kind: String) -> String:
  Bool.pick(String, String.starts_with(id, "pair:"), "pair",
  Bool.pick(String, String.starts_with(id, "room:"), "room",
  Bool.pick(String, String.starts_with(id, "launch:"), "launch",
  Bool.pick(String, String.starts_with(id, "wake:"), "wake",
  Bool.pick(String, String.starts_with(id, "system:"), "system",
  Bool.pick(String, String.starts_with(id, "routine:"), "routine",
  Bool.pick(String, String.starts_with(id, "hook:"), "hook",
  Bool.pick(String, String.starts_with(id, "peer:"), "peer", Conv.source_kind(Mail.kind_code(kind))))))))))

# ConvSet owns membership. Only a missing conversation infers its first
# participants from the mail's addresses (a room is not its own member).
def Conv.mail_parts(missing: Bool, +parts: List<&2, String>, +from: String, +to: String) -> List<&2, String>:
  match missing:
    case True{}: Parts.add(Parts.add(parts, from), to)
    case False{}: parts

def Box.mail_conv(+m: Mail, +old: Conv) -> Conv:
  match m old:
    case Mail{_, +id, +from, +to, +kind, _, _, +head, _, +at} Conv{+was, +subject, +k, +parts, +project, +tags, +made, +last, +n}:
      Conv{id, Bool.pick(String, String.is_empty(was), head, subject), Bool.pick(String, String.is_empty(was), Conv.mail_kind(id, kind), k),
        Conv.mail_parts(String.is_empty(was), parts, from, to), project, tags, Bool.pick(Nat, String.is_empty(was), at, made), Max(at, last), 1n+n}

def Box.has(+b: Box, +id: String) -> Bool:
  Get.has(Map.has(&2, Bool, Box.ids(b), id))

def Box.origin(+b: Box, +thread: String, +msg: String, +o: Origin) -> Box:
  Box.with_origins(b, Map.set(&2, Origin, Box.origins(b), thread ++ "/" ++ msg, o))

def Box.origin_got(r: Map<&2, Origin> & Origin) -> Maybe<&2, Origin>:
  (_, o) = r
  Some{o}

def Get.pop(-A: Data, r: Map<&2, A> & Maybe<&2, A>) -> Maybe<&2, A>:
  (_, value) = r
  value

def Box.origin_of(+b: Box, +thread: String, +msg: String) -> Maybe<&2, Origin>:
  Get.pop(Origin, Map.pop(&2, Origin, Box.origins(b), thread ++ "/" ++ msg))

def Box.run_of(+b: Box, +thread: String) -> Maybe<&2, Run>:
  Get.pop(Run, Map.pop(&2, Run, Box.runs(b), thread))

def Box.live(+b: Box, +bot: String, +conv: String) -> String:
  Get.str(Box.active(b), bot ++ "/" ++ conv)

def Box.live_n(+b: Box, +bot: String) -> Nat:
  Get.nat(Box.live_counts(b), bot)

def Box.run_finish(+live: Bool, +b: Box, +th: String, +bot: String, +conv: String, +m: String, +at: Nat) -> Box:
  match live:
    case False{}: b
    case True{}:
      Box.with_live_counts(Box.with_active(Box.with_runs(b, Map.set(&2, Run, Box.runs(b), th, Run{th, bot, conv, m, at, False{}})), Map.del(&2, String, Box.active(b), bot ++ "/" ++ conv)), Map.set(&2, Nat, Box.live_counts(b), bot, Nat.sub(Box.live_n(b, bot), 1n)))

def Box.run_end_got(+b: Box, +r: Maybe<&2, Run>) -> Box:
  match r:
    case None{}: b
    case Some{Run{+th, +bot, +conv, +m, +at, +live}}:
      Box.run_finish(live, b, th, bot, conv, m, at)

def Box.run_end(+b: Box, +thread: String) -> Box:
  Box.run_end_got(b, Box.run_of(b, thread))

def Box.run_store(+b: Box, +r: Run) -> Box:
  Box.with_live_counts(Box.with_active(Box.with_runs(b, Map.set(&2, Run, Box.runs(b), Run.thread(r), r)), Map.set(&2, String, Box.active(b), Run.bot(r) ++ "/" ++ Run.conv(r), Run.thread(r))), Map.set(&2, Nat, Box.live_counts(b), Run.bot(r), 1n+Box.live_n(b, Run.bot(r))))

def Box.run_set(+b: Box, +thread: String, +bot: String, +conv: String, +mail: String, +at: Nat) -> Box:
  Box.run_store(Box.run_end(Box.run_end(b, thread), Box.live(b, bot, conv)), Run{thread, bot, conv, mail, at, True{}})

def Box.charter(+b: Box, +bot: String) -> String:
  +text = Get.str(Box.charters(b), bot)
  Bool.pick(String, String.is_empty(text), CH.Charter.default(), text)

def Box.charter_set(+b: Box, +bot: String, +text: String) -> Box:
  Box.with_charters(b, Map.set(&2, String, Box.charters(b), bot, text))

def Box.decisions_got(r: Map<&2, List<&2, Decision>> & List<&2, Decision>) -> List<&2, Decision>:
  (_, ds) = r
  ds

def Box.decisions(+b: Box, +bot: String, +n: Nat) -> List<&2, Decision>:
  List.take(&2, Decision, Box.decisions_got(Map.get(List<&2, Decision>, Nil{}, Box.decision_log(b), bot)), n)

def Box.decision(+b: Box, +d: Decision) -> Box:
  Box.with_decision_log(b, Map.set(&2, List<&2, Decision>, Box.decision_log(b), Decision.bot(d), d <> Box.decisions(b, Decision.bot(d), 199n)))

def Box.convs_for(+ids: List<&2, String>, +b: Box) -> List<&2, Conv>:
  match ids:
    case Nil{}: Nil{}
    case +id <> t: Box.conv(b, id) <> Box.convs_for(t, b)

def Box.slice(+b: Box, +bot: String, +skip: Nat, +n: Nat) -> List<&2, Conv>:
  Box.convs_for(Order.slice(Box.order_of(b, bot), skip, n), b)

# List filters visit the order index, never a whole-state map. With no
# filters, slice goes directly to the requested rank.
def Box.matches(+b: Box, +bot: String, +id: String, +status: String, +project: String) -> Bool:
  Bool.and(Bool.or(Bool.not(String.starts_with(bot, "thread:")), Parts.has(Conv.parts(Box.conv(b, id)), bot, False{})), Bool.and(Bool.or(String.is_empty(status), String.eq(Mark.status(Box.mark_of(b, bot, id)), status)), Bool.or(String.is_empty(project), String.eq(Conv.project(Box.conv(b, id)), project))))

type MQuery is Data:
  MQuery{todo: List<&2, Order>, skip: Nat, n: Nat, acc: List<&2, Conv>}

def Box.query_step(+hit: Bool, +b: Box, +id: String, +todo: List<&2, Order>, +skip: Nat, +n: Nat, +acc: List<&2, Conv>) -> MQuery:
  match hit skip:
    case False{} _: MQuery{todo, skip, n, acc}
    case True{} 0n: MQuery{todo, 0n, Nat.sub(n, 1n), Box.conv(b, id) <> acc}
    case True{} 1n+k: MQuery{todo, k, n, acc}

# All compared text is short header metadata, not message bodies.
def Search.str(+s: String, +q: String) -> Bool:
  String.contains(String.to_lower(s), q)

def Search.strings(+xs: List<&2, String>, +q: String, +found: Bool) -> Bool:
  match xs found:
    case _ True{}: True{}
    case Nil{} False{}: False{}
    case +h <> t False{}: Search.strings(t, q, Search.str(h, q))

def Box.newest_head(+ms: List<&2, Mail>) -> String:
  match ms:
    case Nil{}: ""
    case +m <> _: Mail.header(m)

def Box.search_k(+empty: Bool, +b: Box, +bot: String, +id: String, +q: String) -> Bool:
  match empty:
    case True{}: True{}
    case False{}:
      +c = Box.conv(b, id)
      Bool.or(Search.str(Conv.subject(c), q), Bool.or(Search.strings(Conv.parts(c), q, False{}), Bool.or(Search.strings(Conv.tags(c), q, False{}), Bool.or(Search.strings(Mark.tags(Box.mark_of(b, bot, id)), q, False{}), Search.str(Box.newest_head(Box.headers(b, id)), q)))))

def Box.search(+b: Box, +bot: String, +id: String, +q: String) -> Bool:
  Box.search_k(String.is_empty(q), b, bot, id, q)

# Empty left is a cursor sentinel; each real node is expanded at most once.
def Box.query(+fuel: Nat, +cur: MQuery, +b: Box, +bot: String, +status: String, +project: String, +q: String) -> List<&2, Conv>:
  match fuel cur:
    case 0n MQuery{_, _, _, +acc}: List.reverse(&2, Conv, acc)
    case 1n+f MQuery{_, _, 0n, +acc}: List.reverse(&2, Conv, acc)
    case 1n+f MQuery{Nil{}, _, _, +acc}: List.reverse(&2, Conv, acc)
    case 1n+f MQuery{OEmpty{} <> rest, +skip, +n, +acc}: Box.query(f, MQuery{rest, skip, n, acc}, b, bot, status, project, q)
    case 1n+f MQuery{ONode{_, _, +a, +i, OEmpty{}, +r} <> rest, +skip, +n, +acc}:
      Box.query(f, Box.query_step(Bool.and(Box.matches(b, bot, i, status, project), Box.search(b, bot, i, q)), b, i, r <> rest, skip, n, acc), b, bot, status, project, q)
    case 1n+f MQuery{ONode{_, _, +a, +i, +l, +r} <> rest, +skip, +n, +acc}:
      Box.query(f, MQuery{l <> Order.node(a, i, OEmpty{}, r) <> rest, skip, n, acc}, b, bot, status, project, q)

def Box.list_k(+plain: Bool, +b: Box, +bot: String, +status: String, +project: String, +skip: Nat, +n: Nat) -> List<&2, Conv>:
  match plain:
    case True{}: Box.slice(b, bot, skip, n)
    case False{}:
      +tree = Box.order_of(b, bot)
      Box.query(1n+Nat.mul(3n, Order.count(tree)), MQuery{[tree], skip, n, Nil{}}, b, bot, status, project, "")

def Box.list(+b: Box, +bot: String, +status: String, +project: String, +skip: Nat, +n: Nat) -> List<&2, Conv>:
  Box.list_k(Bool.and(Bool.not(String.starts_with(bot, "thread:")), Bool.and(String.is_empty(status), String.is_empty(project))), b, bot, status, project, skip, n)

def Box.find(+b: Box, +bot: String, +q: String, +n: Nat) -> List<&2, Conv>:
  +tree = Box.order_of(b, bot)
  Box.query(1n+Nat.mul(3n, Order.count(tree)), MQuery{[tree], 0n, n, Nil{}}, b, bot, "", "", String.to_lower(q))

# Keep the newest header first even when old mail is replayed late. The
# remaining headers are retained in arrival order, with constant work.
def Headers.add_k(+newer: Bool, +m: Mail, +h: Mail, +rest: List<&2, Mail>) -> List<&2, Mail>:
  match newer:
    case True{}: m <> h <> rest
    case False{}: h <> m <> rest

def Headers.add(+m: Mail, +ms: List<&2, Mail>) -> List<&2, Mail>:
  match ms:
    case Nil{}: [m]
    case +h <> rest: Headers.add_k(Nat.is_ge(Mail.at(m), Mail.at(h)), m, h, rest)

def Box.mail_fresh(+b: Box, +m: Mail, +c: Conv) -> Box:
  Box.received(Conv.parts(c), Box.with_ids(Box.with_mails(Box.conv_put(b, c), Map.set(&2, List<&2, Mail>, Box.mails(b), Mail.conv(m), Headers.add(m, Box.headers(b, Mail.conv(m))))), Map.set(&2, Bool, Box.ids(b), Mail.id(m), True{})), c, Mail.from(m))

# The membership guard is separate so idempotence is a small, general law.
def Box.mail_guard(+seen: Bool, +b: Box, +m: Mail) -> Box:
  match seen:
    case True{}: b
    case False{}: Box.mail_fresh(b, m, Box.mail_conv(m, Box.conv(b, Mail.conv(m))))

def Box.mail(+b: Box, +m: Mail) -> Box:
  Box.mail_guard(Box.has(b, Mail.id(m)), b, m)

# O(log conversations): a map lookup and the first header, never a scan.
def Box.newest(+b: Box, id: String) -> Mail:
  Maybe.default(&2, Mail, List.head(&2, Mail, Box.headers(b, id)), Mail{"", "", "", "", "", "", "", "", 0n, 0n})

# Walk only due entries and their search spine. Order is newest first,
# so future roots prune the left subtree and visit only their right.
type DueCursor is Data:
  DueCursor{todo: List<&2, Order>, out: List<&2, Mark>}

def Snooze.step(+due: Bool, +i: String, +l: Order, +r: Order, +rest: List<&2, Order>, +out: List<&2, Mark>, +b: Box) -> DueCursor:
  match due:
    case False{}: DueCursor{r <> rest, out}
    case True{}: DueCursor{l <> r <> rest, Get.mark.got(Map.get(Mark, Mark.empty(), Box.marks(b), i)) <> out}

def Snooze.walk(+fuel: Nat, +cur: DueCursor, +b: Box, +now: Nat) -> List<&2, Mark>:
  match fuel cur:
    case 0n DueCursor{_, +out}: out
    case 1n+f DueCursor{Nil{}, +out}: out
    case 1n+f DueCursor{OEmpty{} <> +t, +out}: Snooze.walk(f, DueCursor{t, out}, b, now)
    case 1n+f DueCursor{ONode{_, _, +at, +i, +l, +r} <> +t, +out}:
      Snooze.walk(f, Snooze.step(Nat.is_le(at, now), i, l, r, t, out, b), b, now)

def Box.due(+b: Box, now: Nat) -> List<&2, Mark>:
  Snooze.walk(1n+Nat.mul(3n, Order.count(Box.snoozes(b))), DueCursor{[Box.snoozes(b)], Nil{}}, b, now)
