import Base
import ./json.bend as J

# End-anchored, single-lane model ported from TanStack virtual-core 3.19.9
# (MIT), source commit 51b06cd5e078a289fe1cbf25a8d2e647aa98bfff.
# License: timeline-virtual.LICENSE. Algorithm notes: bp-coord/timeline-scroller/DECISION.md.
# Bend Base has no F64. Decimal digit arithmetic preserves the JSON spelling
# of CSS doubles exactly, including fractional offsets beyond F32 precision.
type Px is Data:
  Px{negative: Bool, digits: String, scale: Nat}

def Dig.trim(s: String) -> String:
  match s:
    case SCon{'0', t}: Dig.trim(t)
    case s: s

def Dig.zero(s: String) -> String:
  match s:
    case SNil{}: "0"
    case s: s

def Dig.zeros(n: Nat) -> String:
  match n:
    case 0n: ""
    case 1n+p: "0" ++ Dig.zeros(p)

def Dig.add.go(a: String, b: String, carry: U32, +acc: String) -> String:
  match a b:
    case SNil{} SNil{}:
      Bool.pick(String, U32.is_eq(carry, 0), acc, "1" ++ acc)
    case SCon{Chr{x}, at} SNil{}:
      +sum = U32.add(U32.sub(x, 48), carry)
      Dig.add.go(at, "", U32.div(sum, 10), SCon{Chr{U32.add(48, U32.mod(sum, 10))}, acc})
    case SNil{} SCon{Chr{y}, bt}:
      +sum = U32.add(U32.sub(y, 48), carry)
      Dig.add.go("", bt, U32.div(sum, 10), SCon{Chr{U32.add(48, U32.mod(sum, 10))}, acc})
    case SCon{Chr{x}, at} SCon{Chr{y}, bt}:
      +sum = U32.add(U32.add(U32.sub(x, 48), U32.sub(y, 48)), carry)
      Dig.add.go(at, bt, U32.div(sum, 10), SCon{Chr{U32.add(48, U32.mod(sum, 10))}, acc})

def Dig.add(a: String, b: String) -> String:
  Dig.zero(Dig.trim(Dig.add.go(String.reverse(a), String.reverse(b), 0, "")))

def Dig.sub.go(a: String, b: String, +borrow: U32, acc: String) -> String:
  match a b:
    case SNil{} _: Dig.zero(Dig.trim(acc))
    case SCon{Chr{x}, at} SNil{}:
      +v = U32.sub(x, 48)
      Dig.sub.go(at, "", Bool.pick(U32, U32.is_lt(v, borrow), 1, 0),
        SCon{Chr{U32.add(48, U32.mod(U32.sub(U32.add(10, v), borrow), 10))}, acc})
    case SCon{Chr{x}, at} SCon{Chr{y}, bt}:
      +v = U32.sub(x, 48)
      +w = U32.add(U32.sub(y, 48), borrow)
      Dig.sub.go(at, bt, Bool.pick(U32, U32.is_lt(v, w), 1, 0),
        SCon{Chr{U32.add(48, U32.mod(U32.sub(U32.add(10, v), w), 10))}, acc})

def Dig.sub(a: String, b: String) -> String:
  Dig.sub.go(String.reverse(a), String.reverse(b), 0, "")

def Dig.le(+a: String, +b: String) -> Bool:
  +la = String.length(a)
  +lb = String.length(b)
  Bool.or(Nat.is_lt(la, lb), Bool.and(Nat.is_eq(la, lb), Bool.not(String.is_lt(b, a))))

def Px.make(negative: Bool, +digits: String, scale: Nat) -> Px:
  +d = Dig.zero(Dig.trim(digits))
  Px{Bool.and(negative, Bool.not(String.eq(d, "0"))), d, scale}

def Px.zero() -> Px: Px{False{}, "0", 0n}

def Px.int(n: Nat) -> Px: Px{False{}, Nat.show(n), 0n}

def Px.exponent(negative: Bool, digits: String, +scale: Nat, exp: String) -> Px:
  match exp:
    case SCon{'-', t}: Px.make(negative, digits, Nat.add(scale, Maybe.default(&2, Nat, Nat.read(t), 0n)))
    case SCon{'+', t}:
      +e = Maybe.default(&2, Nat, Nat.read(t), 0n)
      Px.make(negative, digits ++ Dig.zeros(Nat.sub(e, scale)), Nat.sub(scale, e))
    case s:
      +e = Maybe.default(&2, Nat, Nat.read(s), 0n)
      Px.make(negative, digits ++ Dig.zeros(Nat.sub(e, scale)), Nat.sub(scale, e))

def Px.scan(s: String, negative: Bool, +dot: Bool, scale: Nat, acc: String) -> Px:
  match s:
    case SNil{}: Px.make(negative, acc, scale)
    case SCon{'.', t}: Px.scan(t, negative, True{}, scale, acc)
    case SCon{'e', t}: Px.exponent(negative, acc, scale, t)
    case SCon{'E', t}: Px.exponent(negative, acc, scale, t)
    case SCon{c, t}: Px.scan(t, negative, dot, Nat.add(scale, Bool.pick(Nat, dot, 1n, 0n)), acc ++ SCon{c, SNil{}})

def Px.read(s: String) -> Px:
  match s:
    case SCon{'-', t}: Px.scan(t, True{}, False{}, 0n, "")
    case s: Px.scan(s, False{}, False{}, 0n, "")

def Px.scale(p: Px) -> Nat:
  match p:
    case Px{_, _, s}: s

def Px.at(p: Px, +scale: Nat) -> String:
  match p:
    case Px{_, d, s}: Dig.zero(Dig.trim(d ++ Dig.zeros(Nat.sub(scale, s))))

def Px.neg(p: Px) -> Px:
  match p:
    case Px{n, d, s}: Px.make(Bool.not(n), d, s)

def Px.add.signed(same: Bool, +na: Bool, +a: String, +b: String, +scale: Nat) -> Px:
  match same:
    case True{}: Px.make(na, Dig.add(a, b), scale)
    case False{}:
      Bool.pick(Px, Dig.le(b, a), Px.make(na, Dig.sub(a, b), scale), Px.make(Bool.not(na), Dig.sub(b, a), scale))

def Px.add(+a: Px, +b: Px) -> Px:
  match a b:
    case Px{+na, _, +sa} Px{+nb, _, +sb}:
      +s = Bool.pick(Nat, Nat.is_le(sa, sb), sb, sa)
      Px.add.signed(Bool.not(Bool.xor(na, nb)), na, Px.at(a, s), Px.at(b, s), s)

def Px.sub(a: Px, b: Px) -> Px: Px.add(a, Px.neg(b))

def Px.le(+a: Px, +b: Px) -> Bool:
  match a b:
    case Px{False{}, _, +sa} Px{False{}, _, +sb}:
      +s = Bool.pick(Nat, Nat.is_le(sa, sb), sb, sa)
      Dig.le(Px.at(a, s), Px.at(b, s))
    case Px{True{}, _, +sa} Px{True{}, _, +sb}:
      +s = Bool.pick(Nat, Nat.is_le(sa, sb), sb, sa)
      Dig.le(Px.at(b, s), Px.at(a, s))
    case Px{True{}, _, _} Px{False{}, _, _}: True{}
    case _ _: False{}

def Px.max(+a: Px, +b: Px) -> Px: Bool.pick(Px, Px.le(a, b), b, a)

def Px.min(+a: Px, +b: Px) -> Px: Bool.pick(Px, Px.le(a, b), a, b)

def Px.same(+a: Px, +b: Px) -> Bool: Bool.and(Px.le(a, b), Px.le(b, a))

def Px.fraction.signed(negative: Bool, +fraction: Px) -> Px:
  Bool.pick(Px, Bool.and(negative, Bool.not(Px.le(fraction, Px.zero()))), Px.sub(Px.int(1n), fraction), fraction)

# Exact Euclidean modulo one, including negative rounding residuals.
def Px.fraction(p: Px) -> Px:
  match p:
    case Px{n, +d, +s}:
      Px.fraction.signed(n, Px.make(False{}, String.drop(d, Nat.sub(String.length(d), s)), s))

def Px.half(p: Px) -> Px:
  match p:
    case Px{n, +d, s}: Px.make(n, Dig.add(Dig.add(Dig.add(d, d), Dig.add(d, d)), d), 1n+s)

def Px.show(p: Px) -> String:
  match p:
    case Px{n, +d, 0n}: Bool.pick(String, n, "-" ++ d, d)
    case Px{n, +d, 1n+s}:
      +scale = {1n+s : Nat}
      +padded = Dig.zeros(Nat.sub(1n+scale, String.length(d))) ++ d
      +cut = Nat.sub(String.length(padded), scale)
      Bool.pick(String, n, "-", "") ++ String.take(padded, cut) ++ "." ++ String.drop(padded, cut)

def Px.json(p: Px) -> J.Json: J.Num{Px.show(p)}

def Px.get(+j: J.Json, key: String, d: String) -> Px:
  Px.read(J.Json.as_raw(J.Json.get(j, key), d))

type Item is Data:
  Item{key: String, index: Nat, start: Px, size: Px}

type Layout is Data:
  Layout{items: List<&2, Item>, total: Px}

type Anchor is Data:
  Anchor{key: String, offset: Px}

def Item.key(r: Item) -> String:
  match r:
    case Item{k, _, _, _}: k

def Item.start(r: Item) -> Px:
  match r:
    case Item{_, _, s, _}: s

def Item.end(r: Item) -> Px:
  match r:
    case Item{_, _, s, z}: Px.add(s, z)

def Item.read(+j: J.Json) -> Item:
  Item{J.Json.get_str(j, "key", ""), J.Json.get_nat(j, "index", 0n), Px.get(j, "start", "0"), Px.get(j, "size", "120")}

def Items.read(xs: List<&2, J.Json>, acc: List<&2, Item>) -> List<&2, Item>:
  match xs:
    case Nil{}: List.reverse(&2, Item, acc)
    case j <> t: Items.read(t, Item.read(j) <> acc)

def Items.find.k(hit: Bool, r: Item, rest: Maybe<&2, Item>) -> Maybe<&2, Item>:
  match hit:
    case True{}: Some{r}
    case False{}: rest

def Items.head.hit(xs: List<&2, Item>, key: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case r <> _: String.eq(Item.key(r), key)

def Items.find.go(xs: List<&2, Item>, +key: String, hit: Bool) -> Maybe<&2, Item>:
  match xs hit:
    case r <> _ True{}: Some{r}
    case _ <> +t False{}: Items.find.go(t, key, Items.head.hit(t, key))
    case _ _: None{}

def Items.find(+xs: List<&2, Item>, +key: String) -> Maybe<&2, Item>:
  Items.find.go(xs, key, Items.head.hit(xs, key))

def Anchor.capture.k(found: Bool, +r: Item, top: Px, rest: Anchor) -> Anchor:
  match found:
    case True{}: Anchor{Item.key(r), Px.sub(top, Item.start(r))}
    case False{}: rest

# The first row whose end exceeds top also owns its leading gap. This gives
# signed offsets in gaps and above the first row, with no pixel rounding.
def Item.positive(r: Item) -> Bool:
  match r:
    case Item{key, _, _, size}:
      Bool.and(Bool.not(String.starts_with(key, "control:")), Bool.not(Px.le(size, Px.zero())))

def Anchor.head.hit(xs: List<&2, Item>, +top: Px) -> Bool:
  match xs:
    case Nil{}: False{}
    case +r <> _: Bool.and(Item.positive(r), Bool.not(Px.le(Item.end(r), top)))

def Anchor.capture.go(xs: List<&2, Item>, +top: Px, fallback: Anchor, hit: Bool) -> Anchor:
  match xs hit:
    case +r <> _ True{}: Anchor{Item.key(r), Px.sub(top, Item.start(r))}
    case +r <> +t False{}:
      Anchor.capture.go(t, top, Anchor.capture.k(Item.positive(r), r, top, fallback), Anchor.head.hit(t, top))
    case _ _: fallback

def Anchor.capture(+xs: List<&2, Item>, +top: Px, fallback: Anchor) -> Anchor:
  Anchor.capture.go(xs, top, fallback, Anchor.head.hit(xs, top))

def Item.intersects(+r: Item, +top: Px, view: Px) -> Bool:
  Bool.and(Item.positive(r), Bool.and(Bool.not(Px.le(Item.end(r), top)),
    Bool.not(Px.le(Px.add(top, view), Item.start(r)))))

# A prepended estimated row may enter the gap above the reading anchor. Keep
# the existing reading key across measurement passes while it is visible;
# otherwise a newly visible estimate steals the anchor and its resize jumps
# the original text. Always recompute the offset against current DOM top.
def Anchor.prefer(row: Maybe<&2, Item>, +top: Px, view: Px, fresh: Anchor) -> Anchor:
  match row:
    case None{}: fresh
    case Some{+r}: Anchor.capture.k(Item.intersects(r, top, view), r, top, fresh)

def Anchor.reading(+old: List<&2, Item>, previous: J.Json, +top: Px, view: Px) -> Anchor:
  Anchor.prefer(Items.find(old, J.Json.get_str(previous, "anchor", "")), top, view,
    Anchor.capture(old, top, Anchor{"", Px.zero()}))

def Anchor.key(a: Anchor) -> String:
  match a:
    case Anchor{k, _}: k

def Anchor.offset(a: Anchor) -> Px:
  match a:
    case Anchor{_, o}: o

type Position is Data:
  Position{anchor: Anchor, start: Px}

# The signed offset is carried rather than recaptured from rounded DOM output.
def Anchor.move(a: Anchor, start: Px) -> Position: Position{a, start}

def Position.offset(p: Position) -> Px:
  match p:
    case Position{a, _}: Anchor.offset(a)

def Position.top(p: Position) -> Px:
  match p:
    case Position{a, s}: Px.add(s, Anchor.offset(a))

def Anchor.top(row: Maybe<&2, Item>, a: Anchor, fallback: Px) -> Px:
  match row:
    case None{}: fallback
    case Some{r}: Position.top(Anchor.move(a, Item.start(r)))

def Sizes.fields(fs: J.Json, acc: Map<&2, String>) -> Map<&2, String>:
  match fs:
    case J.Field{k, J.Num{s}, t}: Sizes.fields(t, Map.set(&2, String, acc, k, s))
    case J.Field{_, _, t}: Sizes.fields(t, acc)
    case _: acc

def Sizes.map(j: J.Json) -> Map<&2, String>:
  match j:
    case J.Obj{fs}: Sizes.fields(fs, Map.new(&2, String))
    case _: Map.new(&2, String)

type SizeReply is Data:
  SizeReply{map: Map<&2, String>, raw: String}

def Sizes.reply(pair: Map<&2, String> & String) -> SizeReply:
  (m, v) = pair
  SizeReply{m, v}

def Sizes.return.map(reply: SizeReply) -> Map<&2, String>:
  match reply:
    case SizeReply{m, _}: m

def Sizes.return.value(reply: SizeReply) -> String:
  match reply:
    case SizeReply{_, v}: v

def Layout.total(n: Nat, start: Px) -> Px:
  match n:
    case 0n: Px.int(24n)
    case 1n+_: Px.sub(start, Px.int(2n))

def Layout.make(keys: List<&2, J.Json>, sizes: Map<&2, String>, +n: Nat, +start: Px, acc: List<&2, Item>) -> Layout:
  match keys:
    case Nil{}: Layout{List.reverse(&2, Item, acc), Layout.total(n, start)}
    case +j <> t:
      +key = J.Json.as_str(Some{j}, "")
      +reply = Sizes.reply(Map.get(String, "120", sizes, key))
      +size = Px.max(Px.zero(), Px.read(Sizes.return.value(reply)))
      Layout.make(t, Sizes.return.map(reply), 1n+n, Px.add(Px.add(start, size), Px.int(10n)), Item{key, n, start, size} <> acc)

def End.top(total: Px, view: Px) -> Px: Px.max(Px.zero(), Px.sub(total, view))

def End.at(total: Px, top: Px, view: Px) -> Bool:
  Px.le(Px.sub(Px.sub(total, top), view), Px.int(4n))

def Follow.allowed(follow: Bool, atEnd: Bool) -> Bool: Bool.and(follow, atEnd)

def Follow.choose(allowed: Bool, held: Px, end: Px) -> Px:
  match allowed:
    case False{}: held
    case True{}: end

def Key.align.geometry(align: String, +start: Px, +end: Px, +top: Px, +view: Px) -> Px:
  match align:
    case "center": Px.sub(Px.half(Px.add(start, end)), Px.half(view))
    case "end": Px.sub(end, view)
    case "auto":
      Bool.pick(Px, Bool.not(Px.le(top, start)), start,
        Bool.pick(Px, Bool.not(Px.le(end, Px.add(top, view))), Px.sub(end, view), top))
    case _: start

def Key.align(align: String, +r: Item, top: Px, view: Px) -> Px:
  Key.align.geometry(align, Item.start(r), Item.end(r), top, view)

def Key.top(row: Maybe<&2, Item>, align: String, offset: Px, +top: Px, +view: Px, +total: Px) -> Px:
  match row:
    case None{}: top
    case Some{r}: Px.max(Px.zero(), Px.min(End.top(total, view), Px.add(Key.align(align, r, top, view), offset)))

def Restore.top(+bookmark: J.Json, +items: List<&2, Item>, total: Px, view: Px) -> Px:
  +a : Anchor = Anchor{J.Json.get_str(bookmark, "anchor", ""), Px.get(bookmark, "offset", "0")}
  Bool.pick(Px, J.Json.get_bool(bookmark, "atEnd", False{}), End.top(total, view),
    Anchor.top(Items.find(items, Anchor.key(a)), a, Px.get(bookmark, "top", "0")))

def Scroll.preferred(unchanged: Bool, old: List<&2, Item>, previous: J.Json, top: Px, view: Px) -> Anchor:
  match unchanged:
    case True{}: Anchor.reading(old, previous, top, view)
    case False{}: Anchor{"", Px.zero()}

def Plan.preferred(mode: String, +input: J.Json, +top: Px, view: Px) -> Anchor:
  match mode:
    case "update":
      +previous = J.Json.get_obj(input, "previous")
      Anchor.reading(Items.read(J.Json.elems(J.Json.get_obj(previous, "items")), Nil{}), previous, top, view)
    case "measure":
      +previous = J.Json.get_obj(input, "previous")
      Anchor.reading(Items.read(J.Json.elems(J.Json.get_obj(previous, "items")), Nil{}), previous, top, view)
    case "scroll":
      +previous = J.Json.get_obj(input, "previous")
      Scroll.preferred(Px.same(top, Px.sub(Px.get(previous, "top", "0"), Px.get(previous, "trackOffset", "0"))),
        Items.read(J.Json.elems(J.Json.get_obj(previous, "items")), Nil{}), previous, top, view)
    case "restore": Anchor{J.Json.get_str(J.Json.get_obj(input, "bookmark"), "anchor", ""), Px.zero()}
    case "key": Anchor{J.Json.get_str(input, "key", ""), Px.zero()}
    case _: Anchor{"", Px.zero()}

def Plan.top(+mode: String, +input: J.Json, +items: List<&2, Item>, +total: Px, +top: Px, +view: Px, +previous: J.Json, +a: Anchor) -> Px:
  match mode:
    case "init": End.top(total, view)
    case "bottom": End.top(total, view)
    case "scroll": top
    case "restore": Restore.top(J.Json.get_obj(input, "bookmark"), items, total, view)
    case "key": Key.top(Items.find(items, J.Json.get_str(input, "key", "")), J.Json.get_str(input, "align", "start"), Px.get(input, "offset", "0"), top, view, total)
    case _:
      +held = Anchor.top(Items.find(items, Anchor.key(a)), a, top)
      Follow.choose(Follow.allowed(J.Json.get_bool(input, "follow", True{}), End.at(Px.get(previous, "total", "0"), top, Px.get(previous, "view", Px.show(view)))), held, End.top(total, view))

def Window.keep(anchorHit: Bool, inRange: Bool) -> Bool: Bool.or(anchorHit, inRange)

def Window.push(keep: Bool, row: Item, rest: List<&2, Item>) -> List<&2, Item>:
  match keep:
    case True{}: row <> rest
    case False{}: rest

def Window.visible.go(xs: List<&2, Item>, +anchor: String, +lo: Px, +hi: Px, acc: List<&2, Item>) -> List<&2, Item>:
  match xs:
    case Nil{}: List.reverse(&2, Item, acc)
    case +r <> t:
      Window.visible.go(t, anchor, lo, hi, Window.push(Window.keep(String.eq(Item.key(r), anchor), Bool.and(Px.le(lo, Item.end(r)), Px.le(Item.start(r), hi))), r, acc))

def Window.visible(xs: List<&2, Item>, anchor: String, lo: Px, hi: Px) -> List<&2, Item>:
  Window.visible.go(xs, anchor, lo, hi, Nil{})

def Item.json(r: Item) -> J.Json:
  match r:
    case Item{k, n, s, z}: J.Json.obj([J.KV{"key", J.Str{k}}, J.KV{"index", J.Json.nat(n)}, J.KV{"start", Px.json(s)}, J.KV{"size", Px.json(z)}])

def Items.json(xs: List<&2, Item>, acc: List<&2, J.Json>) -> J.Json:
  match xs:
    case Nil{}: J.Json.arr(List.reverse(&2, J.Json, acc))
    case r <> t: Items.json(t, Item.json(r) <> acc)

def Plan.output(+items: List<&2, Item>, +total: Px, +top: Px, +view: Px, preferred: Anchor, +trackOffset: Px) -> J.Json:
  +a = Anchor.prefer(Items.find(items, Anchor.key(preferred)), top, view,
    Anchor.capture(items, top, Anchor{"", Px.zero()}))
  +overscan = Px.add(view, Px.int(720n))
  +visible = Window.visible(items, Anchor.key(a), Px.sub(top, overscan), Px.add(Px.add(top, view), overscan))
  J.Json.obj([J.KV{"items", Items.json(items, Nil{})}, J.KV{"visible", Items.json(visible, Nil{})},
    J.KV{"total", Px.json(total)}, J.KV{"top", Px.json(Px.add(top, trackOffset))}, J.KV{"view", Px.json(view)},
    J.KV{"logicalTop", Px.json(top)}, J.KV{"trackOffset", Px.json(trackOffset)},
    J.KV{"atEnd", J.Flag{End.at(total, top, view)}}, J.KV{"anchor", J.Str{Anchor.key(a)}}, J.KV{"offset", Px.json(Anchor.offset(a))}])

def Plan.layout(layout: Layout, +input: J.Json) -> J.Json:
  match layout:
    case Layout{+items, +total}:
      +view = Px.max(Px.zero(), Px.get(input, "view", "0"))
      +mode = J.Json.get_str(input, "mode", "update")
      +previous = J.Json.get_obj(input, "previous")
      +oldOffset = Px.get(previous, "trackOffset", "0")
      +inputTop = Px.sub(Px.get(input, "top", "0"), oldOffset)
      +a = Plan.preferred(mode, input, inputTop, view)
      +top = Plan.top(mode, input, items, total, inputTop, view, previous, a)
      Plan.output(items, total, top, view, a, Bool.pick(Px, String.eq(mode, "init"), Px.zero(), oldOffset))

# A readback corrects only the physical effect of a rounded/clamped scroll
# write. It uses the previous logical geometry without following or selecting
# a different reading anchor. Normalizing the translation bounds it to <1px.
def Readback.offset(attempt: Nat, +raw: Px) -> Px:
  Bool.pick(Px, Nat.is_ge(attempt, 4n), raw, Px.fraction(raw))

def Plan.readback(+input: J.Json) -> J.Json:
  +previous = J.Json.get_obj(input, "previous")
  +oldOffset = Px.get(previous, "trackOffset", "0")
  +requested = Px.get(previous, "top", "0")
  +logicalTop = Px.sub(requested, oldOffset)
  +residual = Readback.offset(J.Json.get_nat(input, "readbackAttempt", 1n), Px.add(oldOffset, Px.sub(Px.get(input, "top", "0"), requested)))
  Plan.output(Items.read(J.Json.elems(J.Json.get_obj(previous, "items")), Nil{}),
    Px.get(previous, "total", "0"), logicalTop, Px.get(previous, "view", "0"),
    Anchor{J.Json.get_str(previous, "anchor", ""), Px.get(previous, "offset", "0")}, residual)

def Plan.mode(readback: Bool, +input: J.Json) -> J.Json:
  match readback:
    case True{}: Plan.readback(input)
    case False{}: Plan.layout(Layout.make(J.Json.elems(J.Json.get_obj(input, "keys")), Sizes.map(J.Json.get_obj(input, "sizes")), 0n, Px.int(16n), Nil{}), input)

def plan(+input: J.Json) -> J.Json:
  Plan.mode(String.eq(J.Json.get_str(input, "mode", "update"), "readback"), input)
