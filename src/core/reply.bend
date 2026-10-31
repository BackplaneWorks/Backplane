import Base
import ./model.bend as M
import ./ref.bend as R
import ./links.bend as LK
import ./refview.bend as RV
import ./bot.bend as BT
import ./ubersync.bend as US

# Bot chats: replies and context headers (pure; laws rto_*, ctx_*)
# ------------------------------------------------------------------
# A reply is kept in the person's message text itself, ahead of what they
# wrote, as a block the hub builds at send time from the message replied to
# (Rto.resolve):
#
#   <in_reply_to msg=ID chars=N>
#   QUOTE (exactly N characters)
#   </in_reply_to>
#
#   BODY
#
# So it survives a reload and every agent reads the quote with no change to
# its turn, while every client shows only BODY and an "In reply to" line
# naming ID (Rto.of). The quote's length is counted, never searched for, so
# no quote can end the block early. Only bot chats offer Reply (Rto.chat).
type RtoBlock is Data:
  RtoBlock{to: String, quote: String, body: String}

def Rto.open() -> String:
  "<in_reply_to msg="

def Rto.close() -> String:
  "\n</in_reply_to>"

# the most of a message a quote keeps
def Rto.max() -> Nat:
  2000n

# a chat Reply is offered in: a bot's thread or the uberbot's chat
def Rto.chat(+st: M.State, +th: String) -> Bool:
  Bool.or(BT.World.is_bot_thread(M.State.bots(st), th), US.Ub.chat.is(st, th))

def Rto.digit(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))

def Rto.num.go(s: String, +acc: Nat, ok: Bool) -> Nat:
  match s ok:
    case _ False{}: 0n
    case SNil{} True{}: acc
    case SCon{Chr{+c}, t} True{}:
      Rto.num.go(t, Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))), Rto.digit(c))

def Rto.digits.go(s: String, ok: Bool) -> Bool:
  match s:
    case SNil{}: ok
    case SCon{Chr{+c}, t}: Rto.digits.go(t, Bool.and(ok, Rto.digit(c)))

def Rto.digits(+s: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(s)), Rto.digits.go(s, True{}))

def Rto.num(+s: String) -> Nat:
  Rto.num.go(s, 0n, True{})

# up to two blank lines after the block belong to it
def Rto.body.one(+s: String) -> String:
  Bool.pick(String, String.starts_with(s, "\n"), String.drop(s, 1n), s)

def Rto.body(+s: String) -> String:
  Rto.body.one(Rto.body.one(s))

def Rto.tail(ok: Bool, +id: String, +quote: String, +rest: String) -> Maybe<&2, RtoBlock>:
  match ok:
    case True{}: Some{RtoBlock{id, quote, Rto.body(String.drop(rest, 15n))}}
    case False{}: None{}

def Rto.quote(ok: Bool, +id: String, +n: Nat, +rest: String) -> Maybe<&2, RtoBlock>:
  match ok:
    case True{}:
      +r = String.drop(rest, 1n)
      Rto.tail(String.starts_with(String.drop(r, n), Rto.close()), id, String.take(r, n), String.drop(r, n))
    case False{}: None{}

def Rto.count(+id: String, c: R.RefCut) -> Maybe<&2, RtoBlock>:
  match c:
    case R.RefCut{+num, +rest}:
      Rto.quote(Bool.and(Rto.digits(num), String.starts_with(rest, "\n")), id, Rto.num(num), rest)

def Rto.chars(ok: Bool, +id: String, +rest: String) -> Maybe<&2, RtoBlock>:
  match ok:
    case True{}: Rto.count(id, R.Ref.cut(String.drop(rest, 6n), '>', ""))
    case False{}: None{}

def Rto.id(c: R.RefCut) -> Maybe<&2, RtoBlock>:
  match c:
    case R.RefCut{+id, +rest}:
      Rto.chars(Bool.and(Bool.not(String.is_empty(id)), String.starts_with(rest, "chars=")), id, rest)

def Rto.parse(ok: Bool, +s: String) -> Maybe<&2, RtoBlock>:
  match ok:
    case True{}: Rto.id(R.Ref.cut(String.drop(s, 17n), ' ', ""))
    case False{}: None{}

# the reply a message's text carries, if it starts with a whole block
def Rto.of(+text: String) -> Maybe<&2, RtoBlock>:
  Rto.parse(String.starts_with(text, Rto.open()), text)

def Rto.text.k(m: Maybe<&2, RtoBlock>, +text: String) -> String:
  match m:
    case Some{RtoBlock{_, _, +b}}: b
    case None{}: text

# what the person wrote (the text with no reply block)
def Rto.text(+text: String) -> String:
  Rto.text.k(Rto.of(text), text)

def Rto.to.k(m: Maybe<&2, RtoBlock>) -> String:
  match m:
    case Some{RtoBlock{+t, _, _}}: t
    case None{}: ""

# the message a text replies to, "" none
def Rto.to(+text: String) -> String:
  Rto.to.k(Rto.of(text))

def Rto.quote.of.k(m: Maybe<&2, RtoBlock>) -> String:
  match m:
    case Some{RtoBlock{_, +q, _}}: q
    case None{}: ""

def Rto.quote.of(+text: String) -> String:
  Rto.quote.of.k(Rto.of(text))

# the block and the body, the quote cut to Rto.max() (a reply to a reply
# quotes only what that one said)
def Rto.wrap(+to: String, +quote: String, +body: String) -> String:
  +q = String.take(Rto.text(quote), Rto.max())
  Rto.open() ++ to ++ " chars=" ++ Nat.show(String.length(q)) ++ ">\n" ++ q ++ Rto.close() ++ "\n\n" ++ body

def Rto.first.go(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}: ""
    case +h <> t: Bool.pick(String, String.is_empty(String.trim(h)), Rto.first.go(t), String.trim(h))

def Rto.cut(long: Bool, +s: String) -> String:
  match long:
    case True{}: String.take(s, 79n) ++ "…"
    case False{}: s

# one line naming a message: its first line of text, at most 80 characters
def Rto.snip(+text: String) -> String:
  +l = Rto.first.go(String.lines(Rto.text(text)))
  Rto.cut(Nat.is_gt(String.length(l), 80n), l)

def Rto.hit(+e: M.Entry, +id: String) -> Maybe<&2, String>:
  match e:
    case M.Msg{+i, _, _, +x, _}: Bool.pick(Maybe<&2, String>, String.eq(i, id), Some{x}, None{})
    case _: None{}

def Rto.find.go(es: List<&2, M.Entry>, +id: String, found: Maybe<&2, String>) -> Maybe<&2, String>:
  match es found:
    case _ Some{+x}: Some{x}
    case Nil{} None{}: None{}
    case +e <> t None{}: Rto.find.go(t, id, Rto.hit(e, id))

# the text of the message `id` among a thread's entries
def Rto.find(es: List<&2, M.Entry>, +id: String) -> Maybe<&2, String>:
  Rto.find.go(es, id, None{})

def Rto.resolve.k(m: Maybe<&2, String>, +to: String, +text: String) -> String:
  match m:
    case Some{+q}: Rto.wrap(to, q, text)
    case None{}: text

# the text the hub keeps for a message sent in reply to `to` in `thread`:
# the block with the quote, then the text; unchanged when `to` is "" or no
# message of that thread
def Rto.resolve(+st: M.State, +thread: String, +to: String, +text: String) -> String:
  match to:
    case SNil{}: text
    case SCon{+h, +t}: Rto.resolve.k(Rto.find(M.State.mine(st, thread), SCon{h, t}), SCon{h, t}, Rto.text(text))

# Context headers
# ---------------
# A bot's message that links threads or projects (backplane://thread/...,
# backplane://project/..., a thread's web address) gets a header naming the
# one it names most (the first of the most), by its title now, and "+N" for
# the others (MsgCtx). Only links that resolve to a thread or project that
# exists count.
type MsgCtx is Data:
  MsgCtx{uri: String, title: String, more: Nat}

type RtoBest is Data:
  RtoBest{uri: String, n: Nat}

def Ctx.norm(+r: R.Ref) -> String:
  match r:
    case R.Ref{"thread", +id <> +tail, _}: R.Ref.uri(R.Ref{"thread", [id], ""})
    case R.Ref{"project", +id <> +tail, _}: R.Ref.uri(R.Ref{"project", [id], ""})
    case _: ""

def Ctx.uri(m: Maybe<&2, R.Ref>) -> String:
  match m:
    case Some{+r}: Ctx.norm(r)
    case None{}: ""

def Ctx.push(+u: String, acc: List<&2, String>) -> List<&2, String>:
  match u:
    case SNil{}: acc
    case SCon{+h, +t}: SCon{h, t} <> acc

def Ctx.uris(ps: List<&2, LK.Piece>, acc: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}: List.reverse(&2, String, acc)
    case LK.PRef{_, +u, _} <> t: Ctx.uris(t, Ctx.push(Ctx.uri(R.Ref.parse(u)), acc))
    case LK.PLink{_, +u} <> t: Ctx.uris(t, Ctx.push(Ctx.uri(R.Ref.parse(u)), acc))
    case LK.PText{_} <> t: Ctx.uris(t, acc)

def Ctx.title.k(m: Maybe<&2, RV.Card>) -> String:
  match m:
    case Some{RV.Card{_, _, _, +title, _, _, _}}: title
    case None{}: ""

def Ctx.title.m(m: Maybe<&2, R.Ref>, +st: M.State) -> String:
  match m:
    case Some{+r}: Ctx.title.k(RV.Card.of(st, r))
    case None{}: ""

# a link's title now, "" when it names nothing that exists
def Ctx.title(+st: M.State, +uri: String) -> String:
  Ctx.title.m(R.Ref.parse(uri), st)

def Ctx.live(xs: List<&2, String>, +st: M.State, +acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: List.reverse(&2, String, acc)
    case +h <> t: Ctx.live(t, st, Bool.pick(List<&2, String>, String.is_empty(Ctx.title(st, h)), acc, h <> acc))

def Ctx.has(xs: List<&2, String>, +u: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case +h <> t: Bool.or(String.eq(h, u), Ctx.has(t, u))

def Ctx.distinct(xs: List<&2, String>, +acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: List.reverse(&2, String, acc)
    case +h <> t: Ctx.distinct(t, Bool.pick(List<&2, String>, Ctx.has(acc, h), acc, h <> acc))

def Ctx.count(xs: List<&2, String>, +u: String, +n: Nat) -> Nat:
  match xs:
    case Nil{}: n
    case +h <> t: Ctx.count(t, u, Bool.pick(Nat, String.eq(h, u), Nat.add(n, 1n), n))

def Ctx.better(+n: Nat, +d: String, b: RtoBest) -> RtoBest:
  match b:
    case RtoBest{+u, +m}: Bool.pick(RtoBest, Nat.is_gt(n, m), RtoBest{d, n}, RtoBest{u, m})

def Ctx.best(ds: List<&2, String>, +all: List<&2, String>, b: RtoBest) -> RtoBest:
  match ds:
    case Nil{}: b
    case +d <> t: Ctx.best(t, all, Ctx.better(Ctx.count(all, d, 0n), d, b))

def Ctx.make(b: RtoBest, +st: M.State, +more: Nat) -> Maybe<&2, MsgCtx>:
  match b:
    case RtoBest{+u, +n}: Bool.pick(Maybe<&2, MsgCtx>, Nat.is_eq(n, 0n), None{}, Some{MsgCtx{u, Ctx.title(st, u), more}})

def Ctx.pick(+all: List<&2, String>, +st: M.State) -> Maybe<&2, MsgCtx>:
  +ds = Ctx.distinct(all, Nil{})
  Ctx.make(Ctx.best(ds, all, RtoBest{"", 0n}), st, Nat.sub(List.length(&2, String, ds), 1n))

# the header a message's text gets: its thread and project links, live
def Ctx.scan(go: Bool, +st: M.State, +text: String) -> Maybe<&2, MsgCtx>:
  match go:
    case True{}: Ctx.pick(Ctx.live(Ctx.uris(LK.Link.pieces(text), Nil{}), st, Nil{}), st)
    case False{}: None{}

# (a text with no link at all is never scanned)
def Ctx.of(+st: M.State, +text: String) -> Maybe<&2, MsgCtx>:
  Ctx.scan(Bool.or(String.contains(text, "://"), String.contains(text, "](")), st, text)
