import Base
import ./json.bend as J
import ./mail.bend as ML
import ./model.bend as M
import ./ref.bend as R

# Versioned body journal protocol. Bodies never enter event Changes.
type Body is Data:
  Body{conv: ML.Conv, mail: ML.Mail, text: String}

def Body.conv(b: Body) -> ML.Conv:
  match b:
    case Body{v, _, _}: v

def Body.mail(b: Body) -> ML.Mail:
  match b:
    case Body{_, v, _}: v

def Body.text(b: Body) -> String:
  match b:
    case Body{_, _, v}: v

def Body.when(-A: Data, yes: Bool, value: A) -> Maybe<&2, A>:
  match yes:
    case True{}: Some{value}
    case False{}: None{}

def Body.str(j: J.Json) -> Maybe<&2, String>:
  match j:
    case J.Str{s}: Some{s}
    case _: None{}

def Body.need(j: J.Json, k: String) -> Maybe<&2, String>:
  do Maybe<&2, String>:
    v : J.Json <- J.Json.get(j, k)
    Body.str(v)

def Body.nonempty(s: String) -> Maybe<&2, String>:
  match s:
    case SNil{}: None{}
    case SCon{c, t}: Some{SCon{c, t}}

# Deliberately accept only the canonical nonnegative integer grammar.
# No defaults, signs, fractions, exponents or string-to-number coercions.
def Body.digits(s: String, acc: Nat) -> Maybe<&2, Nat>:
  match s:
    case SNil{}: Some{acc}
    case SCon{Chr{+c}, t}:
      do Maybe<&2, Nat>:
        d : Nat <- Body.when(Nat, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), U32.to_nat(U32.sub(c, 48)))
        Body.digits(t, Nat.add(Nat.mul(acc, 10n), d))

def Body.number(j: J.Json) -> Maybe<&2, Nat>:
  match j:
    case J.Num{SCon{Chr{48}, SNil{}}}: Some{0n}
    case J.Num{SCon{Chr{+c}, t}}:
      do Maybe<&2, Nat>:
        d : Nat <- Body.when(Nat, Bool.and(U32.is_ge(c, 49), U32.is_le(c, 57)), U32.to_nat(U32.sub(c, 48)))
        Body.digits(t, d)
    case _: None{}

def Body.num(j: J.Json, k: String) -> Maybe<&2, Nat>:
  do Maybe<&2, Nat>:
    v : J.Json <- J.Json.get(j, k)
    Body.number(v)

def Body.strings(j: J.Json, acc: List<&2, String>) -> Maybe<&2, List<&2, String>>:
  match j:
    case J.End{}: Some{List.reverse(&2, String, acc)}
    case J.Item{J.Str{s}, t}: Body.strings(t, s <> acc)
    case _: None{}

def Body.array(j: J.Json) -> Maybe<&2, List<&2, String>>:
  match j:
    case J.Arr{items}: Body.strings(items, Nil{})
    case _: None{}

def Body.list(j: J.Json, k: String) -> Maybe<&2, List<&2, String>>:
  do Maybe<&2, List<&2, String>>:
    v : J.Json <- J.Json.get(j, k)
    Body.array(v)

def Body.items(xs: List<&2, String>, acc: List<&2, J.Json>) -> J.Json:
  match xs:
    case Nil{}: J.Json.arr(List.reverse(&2, J.Json, acc))
    case h <> t: Body.items(t, J.Str{h} <> acc)

def Body.conv.encode(c: ML.Conv) -> J.Json:
  match c:
    case ML.Conv{id, subject, kind, parts, project, tags, made, last, n}:
      J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"subject", J.Str{subject}}, J.KV{"kind", J.Str{kind}}, J.KV{"parts", Body.items(parts, Nil{})}, J.KV{"project", J.Str{project}}, J.KV{"tags", Body.items(tags, Nil{})}, J.KV{"made", J.Json.nat(made)}, J.KV{"last", J.Json.nat(last)}, J.KV{"n", J.Json.nat(n)}])

def Body.conv.decode(+j: J.Json) -> Maybe<&2, ML.Conv>:
  do Maybe<&2, ML.Conv>:
    +id : String <- Body.need(j, "id")
    subject : String <- Body.need(j, "subject")
    kind : String <- Body.need(j, "kind")
    parts : List<&2, String> <- Body.list(j, "parts")
    project : String <- Body.need(j, "project")
    tags : List<&2, String> <- Body.list(j, "tags")
    made : Nat <- Body.num(j, "made")
    last : Nat <- Body.num(j, "last")
    n : Nat <- Body.num(j, "n")
    valid : String <- Body.nonempty(id)
    return ML.Conv{id, subject, kind, parts, project, tags, made, last, n}

def Body.mail.encode(c: ML.Mail) -> J.Json:
  match c:
    case ML.Mail{id, conv, from, to, kind, ref, want, head, size, at}:
      J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"conv", J.Str{conv}}, J.KV{"from", J.Str{from}}, J.KV{"to", J.Str{to}}, J.KV{"kind", J.Str{kind}}, J.KV{"ref", J.Str{ref}}, J.KV{"want", J.Str{want}}, J.KV{"head", J.Str{head}}, J.KV{"size", J.Json.nat(size)}, J.KV{"at", J.Json.nat(at)}])

def Body.mail.decode(+j: J.Json) -> Maybe<&2, ML.Mail>:
  do Maybe<&2, ML.Mail>:
    +id : String <- Body.need(j, "id")
    conv : String <- Body.need(j, "conv")
    from : String <- Body.need(j, "from")
    to : String <- Body.need(j, "to")
    kind : String <- Body.need(j, "kind")
    ref : String <- Body.need(j, "ref")
    +want : String <- Body.need(j, "want")
    head : String <- Body.need(j, "head")
    size : Nat <- Body.num(j, "size")
    at : Nat <- Body.num(j, "at")
    valid : String <- Body.nonempty(id)
    ok : Unit <- Body.when(Unit, Bool.or(String.eq(want, "fyi"), String.eq(want, "act")), Unit{})
    return ML.Mail{id, conv, from, to, kind, ref, want, head, size, at}

def Body.encode(b: Body) -> J.Json:
  match b:
    case Body{c, m, text}:
      J.Json.obj([J.KV{"v", J.Num{"1"}}, J.KV{"conv", Body.conv.encode(c)}, J.KV{"mail", Body.mail.encode(m)}, J.KV{"text", J.Str{text}}])

def Body.decode(+j: J.Json) -> Maybe<&2, Body>:
  do Maybe<&2, Body>:
    v : Nat <- Body.num(j, "v")
    ok : Unit <- Body.when(Unit, Nat.is_eq(v, 1n), Unit{})
    cj : J.Json <- J.Json.get(j, "conv")
    +c : ML.Conv <- Body.conv.decode(cj)
    mj : J.Json <- J.Json.get(j, "mail")
    +m : ML.Mail <- Body.mail.decode(mj)
    text : String <- Body.need(j, "text")
    same : Unit <- Body.when(Unit, String.eq(ML.Conv.id(c), ML.Mail.conv(m)), Unit{})
    return Body{c, m, text}

def Body.parse(s: String) -> Maybe<&2, Body>:
  do Maybe<&2, Body>:
    j : J.Json <- J.Json.parse(s)
    Body.decode(j)

# Exact, tail-recursive comparison for arbitrarily long body records. Base's
# String.eq rebuilds its input after comparing each tail and exhausts the stack.
# Carry the verdict in the recursive walk to stop at the first mismatch.
def Body.same.go(a: String, b: String, same: Bool) -> Bool:
  match a b same:
    case _ _ False{}: False{}
    case SNil{} SNil{} True{}: True{}
    case SCon{Chr{x}, t} SCon{Chr{y}, u} True{}: Body.same.go(t, u, U32.is_eq(x, y))
    case _ _ _: False{}

def Body.same(a: String, b: String) -> Bool:
  Body.same.go(a, b, True{})

def Body.line(b: Body) -> String:
  # Emit directly onto the terminator. Base's ++ recurses once per character.
  J.Json.emit(Body.encode(b), True{}, "\n")

# ConvSet's legacy wire representation is newline-delimited. The body
# journal preserves the original arrays, including delimiters, losslessly.
def Body.join(xs: List<&2, String>) -> String:
  String.join(xs, "\n")

def Body.metadata(c: ML.Conv) -> M.Change:
  match c:
    case ML.Conv{id, subject, kind, parts, project, tags, made, _, _}:
      M.ConvSet{id, subject, kind, Body.join(parts), project, Body.join(tags), made}

def Body.header(m: ML.Mail) -> M.Change:
  match m:
    case ML.Mail{id, conv, from, to, kind, ref, want, head, size, at}:
      M.MailPosted{id, conv, from, to, kind, ref, want, head, size, at}

def Body.changes(b: Body) -> List<&2, M.Change>:
  match b:
    case Body{c, m, _}: [Body.metadata(c), Body.header(m)]

def Body.recover.new(missing: Bool, b: Body) -> List<&2, M.Change>:
  match missing b:
    case True{} Body{c, m, _}: [Body.metadata(c), Body.header(m)]
    case False{} Body{_, m, _}: [Body.header(m)]

def Body.recover.guard(seen: Bool, +b: Body, box: ML.Box) -> List<&2, M.Change>:
  match seen:
    case True{}: Nil{}
    case False{}: Body.recover.new(String.is_empty(ML.Conv.id(ML.Box.conv(box, ML.Conv.id(Body.conv(b))))), b)

def Body.recover(+b: Body, +box: ML.Box) -> List<&2, M.Change>:
  Body.recover.guard(ML.Box.has(box, ML.Mail.id(Body.mail(b))), b, box)

def Body.path(home: String, conv: String) -> String:
  home ++ "/mail/" ++ R.Ref.pct(conv) ++ ".jsonl"

# Index range reads already frame one physical row. Lex once while checking
# its terminator, so validation neither copies nor re-walks a shared long text.
# A newline is allowed only as the very last character of the physical row.
def Body.row.tokens(s: String, last: U32, st: J.Lex & List<&2, J.Tok>) -> Maybe<&2, List<&2, J.Tok>>:
  match s last:
    case SNil{} 10: Some{J.Lex.go.fin(st)}
    case SNil{} _: None{}
    case SCon{c, t} 10: None{}
    case SCon{Chr{+c}, t} _: Body.row.tokens(t, c, J.Lex.step.pair(Chr{c}, st))

def Body.parse_line(s: String) -> Maybe<&2, Body>:
  do Maybe<&2, Body>:
    tokens : List<&2, J.Tok> <- Body.row.tokens(s, 0, (J.LIdle{}, Nil{}))
    json : J.Json <- J.Parse.fin(J.Parse.go(tokens, J.PGo{Nil{}, None{}}))
    Body.decode(json)
