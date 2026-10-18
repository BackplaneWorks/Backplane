import Base

# One grammar, independent of the model and the message scanner.
# Shapes: thread/id, bot/id, room/id, person, project/id, routine/id,
# task/id, pr/project/number, work/id, file/project/path/segments...,
# machine/link, conv/id, mail/conv/id, mailroom/bot-or-*, decision/bot/id,
# charter/bot. Remote ids retain link~id. Only threads have a fragment.
type Ref is Data:
  Ref{kind: String, rest: List<&2, String>, frag: String}

def Ref.kinds() -> List<&2, String>:
  ["thread", "bot", "room", "person", "project", "routine", "task", "pr", "work", "file", "machine", "conv", "mail", "mailroom", "decision", "charter"]

def Ref.has(xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> t: Bool.or(String.eq(h, s), Ref.has(t, s))

def Ref.known(kind: String) -> Bool:
  Ref.has(Ref.kinds(), kind)

def Ref.safe(+u: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(u, 65), U32.is_le(u, 90)), Bool.and(U32.is_ge(u, 97), U32.is_le(u, 122))),
    Bool.or(Bool.and(U32.is_ge(u, 48), U32.is_le(u, 57)), Bool.or(Bool.or(U32.is_eq(u, 45), U32.is_eq(u, 46)), Bool.or(U32.is_eq(u, 95), U32.is_eq(u, 126)))))

def Ref.hex(+u: U32) -> Char:
  Chr{Bool.pick(U32, U32.is_lt(u, 10), (48 + u : U32), (55 + u : U32))}

# UTF-8 bytes prepended to a reversed output: no recursive byte list.
def Ref.byte(+u: U32, acc: String) -> String:
  SCon{Ref.hex(U32.and(u, 15)), SCon{Ref.hex(U32.shrn(u, 4n)), SCon{'%', acc}}}

def Ref.cp(a: Bool, b: Bool, c: Bool, +u: U32, acc: String) -> String:
  match a b c:
    case True{} _ _: Ref.byte(u, acc)
    case False{} True{} _:
      Ref.byte(U32.or(128, U32.and(u, 63)), Ref.byte(U32.or(192, U32.shrn(u, 6n)), acc))
    case False{} False{} True{}:
      Ref.byte(U32.or(128, U32.and(u, 63)), Ref.byte(U32.or(128, U32.and(U32.shrn(u, 6n), 63)), Ref.byte(U32.or(224, U32.shrn(u, 12n)), acc)))
    case False{} False{} False{}:
      Ref.byte(U32.or(128, U32.and(u, 63)), Ref.byte(U32.or(128, U32.and(U32.shrn(u, 6n), 63)), Ref.byte(U32.or(128, U32.and(U32.shrn(u, 12n), 63)), Ref.byte(U32.or(240, U32.shrn(u, 18n)), acc))))

def Ref.enc.step(safe: Bool, +u: U32, acc: String) -> String:
  match safe:
    case True{}: SCon{Chr{u}, acc}
    case False{}: Ref.cp(U32.is_lt(u, 128), U32.is_lt(u, 2048), U32.is_lt(u, 65536), u, acc)

def Ref.enc(s: String, acc: String) -> String:
  match s:
    case SNil{}: String.reverse(acc)
    case SCon{Chr{+u}, t}: Ref.enc(t, Ref.enc.step(Ref.safe(u), u, acc))

def Ref.pct(s: String) -> String:
  Ref.enc(s, "")

# Percent tokens and UTF-8 decoding are both streaming. Invalid hex stays
# literal (including incomplete % and %A); invalid UTF-8 yields U+FFFD.
type RefDec is Data:
  RefDec{acc: String, cp: U32, owe: Nat, floor: U32}

def Ref.dec.start(a: Bool, b: Bool, c: Bool, d: Bool, +u: U32, acc: String) -> RefDec:
  match a b c d:
    case True{} _ _ _: RefDec{SCon{Chr{u}, acc}, 0, 0n, 0}
    case False{} True{} _ _: RefDec{acc, U32.and(u, 31), 1n, 128}
    case False{} False{} True{} _: RefDec{acc, U32.and(u, 15), 2n, 2048}
    case False{} False{} False{} True{}: RefDec{acc, U32.and(u, 7), 3n, 65536}
    case False{} False{} False{} False{}: RefDec{SCon{Chr{65533}, acc}, 0, 0n, 0}

def Ref.dec.lead(+u: U32, acc: String) -> RefDec:
  Ref.dec.start(U32.is_lt(u, 128), Bool.and(U32.is_ge(u, 194), U32.is_le(u, 223)), Bool.and(U32.is_ge(u, 224), U32.is_le(u, 239)), Bool.and(U32.is_ge(u, 240), U32.is_le(u, 244)), u, acc)

def Ref.dec.end(+cp: U32, +floor: U32, acc: String) -> RefDec:
  RefDec{SCon{Chr{Bool.pick(U32, Bool.or(U32.is_lt(cp, floor), Bool.or(U32.is_gt(cp, 1114111), Bool.and(U32.is_ge(cp, 55296), U32.is_le(cp, 57343)))), 65533, cp)}, acc}, 0, 0n, 0}

def Ref.dec.cont(ok: Bool, +u: U32, acc: String, +cp: U32, owe: Nat, floor: U32) -> RefDec:
  match ok owe:
    case True{} 1n+0n: Ref.dec.end(U32.or(U32.shln(cp, 6n), U32.and(u, 63)), floor, acc)
    case True{} 1n+n: RefDec{acc, U32.or(U32.shln(cp, 6n), U32.and(u, 63)), n, floor}
    case True{} 0n: Ref.dec.lead(u, acc)
    case False{} _: Ref.dec.lead(u, SCon{Chr{65533}, acc})

def Ref.dec.byte(+u: U32, d: RefDec) -> RefDec:
  match d:
    case RefDec{acc, _, 0n, _}: Ref.dec.lead(u, acc)
    case RefDec{acc, cp, 1n+n, floor}: Ref.dec.cont(Bool.and(U32.is_ge(u, 128), U32.is_le(u, 191)), u, acc, cp, 1n+n, floor)

def Ref.dec.raw(c: Char, d: RefDec) -> RefDec:
  match d:
    case RefDec{acc, _, 0n, _}: RefDec{SCon{c, acc}, 0, 0n, 0}
    case RefDec{acc, _, 1n+n, _}: RefDec{SCon{c, SCon{Chr{65533}, acc}}, 0, 0n, 0}

def Ref.nibble(a: Bool, b: Bool, c: Bool, +u: U32) -> U32:
  match a b c:
    case True{} _ _: (u - 48 : U32)
    case False{} True{} _: (u - 55 : U32)
    case False{} False{} True{}: (u - 87 : U32)
    case False{} False{} False{}: 16

def Ref.unhex(+c: Char) -> U32:
  +u = Char.to_u32(c)
  Ref.nibble(Bool.and(U32.is_ge(u, 48), U32.is_le(u, 57)), Bool.and(U32.is_ge(u, 65), U32.is_le(u, 70)), Bool.and(U32.is_ge(u, 97), U32.is_le(u, 102)), u)

type RefPct is Data:
  RefPct{dec: RefDec, mode: Nat, hi: Char}

def Ref.un.start(pct: Bool, c: Char, d: RefDec) -> RefPct:
  match pct:
    case True{}: RefPct{d, 1n, '0'}
    case False{}: RefPct{Ref.dec.raw(c, d), 0n, '0'}

def Ref.un.hex(ok: Bool, +n: U32, +c: Char, d: RefDec, mode: Nat, +hi: Char) -> RefPct:
  match ok mode:
    case True{} 1n: RefPct{d, 2n, c}
    case True{} _: RefPct{Ref.dec.byte((Ref.unhex(hi) * 16 + n : U32), d), 0n, '0'}
    case False{} 1n: Ref.un.start(Char.is_eq(c, '%'), c, Ref.dec.raw('%', d))
    case False{} _: Ref.un.start(Char.is_eq(c, '%'), c, Ref.dec.raw(hi, Ref.dec.raw('%', d)))

def Ref.un.step(+c: Char, st: RefPct) -> RefPct:
  match st:
    case RefPct{d, 0n, _}: Ref.un.start(Char.is_eq(c, '%'), c, d)
    case RefPct{d, m, hi}: Ref.un.hex(U32.is_lt(Ref.unhex(c), 16), Ref.unhex(c), c, d, m, hi)

def Ref.un.fin(st: RefPct) -> RefDec:
  match st:
    case RefPct{d, 0n, _}: d
    case RefPct{d, 1n, _}: Ref.dec.raw('%', d)
    case RefPct{d, _, hi}: Ref.dec.raw(hi, Ref.dec.raw('%', d))

def Ref.un.go(s: String, st: RefPct) -> RefDec:
  match s:
    case SNil{}: Ref.un.fin(st)
    case SCon{+c, t}: Ref.un.go(t, Ref.un.step(c, st))

def Ref.un.out(d: RefDec) -> String:
  match d:
    case RefDec{acc, _, 0n, _}: String.reverse(acc)
    case RefDec{acc, _, _, _}: String.reverse(SCon{Chr{65533}, acc})

def Ref.unpct(s: String) -> String:
  Ref.un.out(Ref.un.go(s, RefPct{RefDec{"", 0, 0n, 0}, 0n, '0'}))

# Tail-recursive splitting; no Base.String.split on unbounded text.
type RefSplit is Data:
  RefSplit{word: String, out: List<&2, String>}

def Ref.split.step(cut: Bool, c: Char, st: RefSplit) -> RefSplit:
  match cut st:
    case True{} RefSplit{w, out}: RefSplit{"", String.reverse(w) <> out}
    case False{} RefSplit{w, out}: RefSplit{SCon{c, w}, out}

def Ref.split.go(s: String, +sep: Char, st: RefSplit) -> List<&2, String>:
  match s st:
    case SNil{} RefSplit{w, out}: List.reverse(&2, String, String.reverse(w) <> out)
    case SCon{+c, t} _: Ref.split.go(t, sep, Ref.split.step(Char.is_eq(c, sep), c, st))

def Ref.split(s: String, sep: Char) -> List<&2, String>:
  Ref.split.go(s, sep, RefSplit{"", Nil{}})

# Cut at the first delimiter; return the untouched suffix.
type RefCut is Data:
  RefCut{head: String, tail: String}

def Ref.head_eq(s: String, sep: Char) -> Bool:
  match s:
    case SNil{}: False{}
    case SCon{c, _}: Char.is_eq(c, sep)

def Ref.cut.scan(s: String, +sep: Char, acc: String, hit: Bool) -> RefCut:
  match s hit:
    case SNil{} _: RefCut{String.reverse(acc), ""}
    case SCon{c, t} True{}: RefCut{String.reverse(acc), t}
    case SCon{c, +t} False{}: Ref.cut.scan(t, sep, SCon{c, acc}, Ref.head_eq(t, sep))

def Ref.cut(+s: String, +sep: Char, acc: String) -> RefCut:
  Ref.cut.scan(s, sep, acc, Ref.head_eq(s, sep))

def Ref.segments(xs: List<&2, String>, acc: String) -> String:
  match xs:
    case Nil{}: acc
    case h <> t: Ref.segments(t, acc ++ "/" ++ Ref.pct(h))

def Ref.path(r: Ref) -> String:
  match r:
    case Ref{k, xs, _}: Ref.segments(xs, k)

def Ref.fragment(f: String) -> String:
  match f:
    case SNil{}: ""
    case SCon{h, t}: "#" ++ Ref.pct(SCon{h, t})

def Ref.uri(r: Ref) -> String:
  match r:
    case Ref{k, xs, f}: "backplane://" ++ Ref.path(Ref{k, xs, ""}) ++ Ref.fragment(f)

def Ref.nonempty(xs: List<&2, String>, ok: Bool) -> Bool:
  match xs:
    case Nil{}: ok
    case h <> t: Ref.nonempty(t, Bool.and(ok, Bool.not(String.is_empty(h))))

def Ref.shape(+k: String, xs: List<&2, String>) -> Bool:
  match xs:
    case Nil{}: String.eq(k, "person")
    case _ <> Nil{}:
      Bool.not(Ref.has(["person", "pr", "file", "mail", "decision"], k))
    case _ <> _ <> Nil{}:
      Ref.has(["pr", "file", "mail", "decision"], k)
    case _ <> _ <> _ <> _: String.eq(k, "file")

def Ref.accept(ok: Bool, r: Ref) -> Maybe<&2, Ref>:
  match ok:
    case True{}: Some{r}
    case False{}: None{}

def Ref.checked(+k: String, +xs: List<&2, String>, +f: String) -> Maybe<&2, Ref>:
  Ref.accept(Bool.and(Ref.known(k), Bool.and(Ref.shape(k, xs), Bool.and(Ref.nonempty(xs, True{}), Bool.or(String.is_empty(f), String.eq(k, "thread"))))), Ref{k, xs, f})

def Ref.decode(xs: List<&2, String>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: List.reverse(&2, String, acc)
    case h <> t: Ref.decode(t, Ref.unpct(h) <> acc)

# One trailing slash is ignored; empty interior segments remain invalid.
def Ref.untrail.rev(s: String) -> String:
  match s:
    case SCon{'/', t}: String.reverse(t)
    case _: String.reverse(s)

def Ref.untrail(s: String) -> String:
  Ref.untrail.rev(String.reverse(s))

def Ref.web(+base: String, r: Ref) -> String:
  match r:
    case Ref{k, xs, f}:
      Ref.untrail(base) ++ "/#/" ++ Ref.path(Ref{k, xs, ""}) ++ Ref.fragment(f)

def Ref.parse.parts(xs: List<&2, String>, f: String) -> Maybe<&2, Ref>:
  match xs:
    case Nil{}: None{}
    case k <> t: Ref.checked(k, Ref.decode(t, Nil{}), Ref.unpct(f))

def Ref.parse.path(c: RefCut) -> Maybe<&2, Ref>:
  match c:
    case RefCut{p, f}: Ref.parse.parts(Ref.split(Ref.untrail(p), '/'), f)

def Ref.web.path(ok: Bool, tail: String) -> Maybe<&2, Ref>:
  match ok:
    case True{}: Ref.parse.path(Ref.cut(String.drop(tail, 1n), '#', ""))
    case False{}: None{}

def Ref.web.origin(c: RefCut) -> Maybe<&2, Ref>:
  match c:
    case RefCut{+host, +tail}:
      Ref.web.path(Bool.and(Bool.not(String.is_empty(Ref.untrail(host))), Bool.and(Bool.not(String.contains(Ref.untrail(host), "/")), String.starts_with(tail, "/"))), tail)

def Ref.legacy(ok: Bool, id: String) -> Maybe<&2, Ref>:
  match ok:
    case True{}: Ref.checked("thread", [Ref.unpct(id)], "")
    case False{}: None{}

def Ref.parse.bp.k(legacy: Bool, +s: String) -> Maybe<&2, Ref>:
  match legacy:
    case True{}: Ref.legacy(Bool.not(String.is_empty(String.drop(s, 12n))), String.drop(s, 12n))
    case False{}: Ref.parse.path(Ref.cut(s, '#', ""))

def Ref.parse.bp(+s: String) -> Maybe<&2, Ref>:
  Ref.parse.bp.k(String.starts_with(s, "open?thread="), s)

def Ref.parse.scheme(bp: Bool, http: Bool, https: Bool, +s: String) -> Maybe<&2, Ref>:
  match bp http https:
    case True{} _ _:
      Ref.parse.bp(String.drop(s, 12n))
    case False{} True{} _: Ref.web.origin(Ref.cut(String.drop(s, 7n), '#', ""))
    case False{} False{} True{}: Ref.web.origin(Ref.cut(String.drop(s, 8n), '#', ""))
    case False{} False{} False{}: None{}

def Ref.parse(+s: String) -> Maybe<&2, Ref>:
  Ref.parse.scheme(String.starts_with(s, "backplane://"), String.starts_with(s, "http://"), String.starts_with(s, "https://"), s)

# Addresses use decoded segments joined by /. Conv ids contain : and |,
# never /; file paths already use / as their segment separator. A decoded
# segment containing / is URI-roundtrippable but not address-roundtrippable.
def Ref.addr(r: Ref) -> String:
  match r:
    case Ref{"person", _, _}: "person"
    case Ref{k, xs, _}: k ++ ":" ++ String.join(xs, "/")

def Ref.addr.parts(c: RefCut) -> Maybe<&2, Ref>:
  match c:
    case RefCut{k, tail}: Ref.checked(k, Ref.split(tail, '/'), "")

def Ref.of_addr.k(person: Bool, s: String) -> Maybe<&2, Ref>:
  match person:
    case True{}: Some{Ref{"person", Nil{}, ""}}
    case False{}: Ref.addr.parts(Ref.cut(s, ':', ""))

def Ref.of_addr(+s: String) -> Maybe<&2, Ref>:
  Ref.of_addr.k(String.eq(s, "person"), s)

def Ref.thread(id: String) -> Ref:
  Ref{"thread", [id], ""}

def Ref.bot(id: String) -> Ref:
  Ref{"bot", [id], ""}

def Ref.room(id: String) -> Ref:
  Ref{"room", [id], ""}

def Ref.project(id: String) -> Ref:
  Ref{"project", [id], ""}

def Ref.routine(id: String) -> Ref:
  Ref{"routine", [id], ""}

def Ref.task(id: String) -> Ref:
  Ref{"task", [id], ""}

def Ref.work(id: String) -> Ref:
  Ref{"work", [id], ""}

def Ref.machine(id: String) -> Ref:
  Ref{"machine", [id], ""}

def Ref.conv(id: String) -> Ref:
  Ref{"conv", [id], ""}

def Ref.mailroom(id: String) -> Ref:
  Ref{"mailroom", [id], ""}

def Ref.charter(id: String) -> Ref:
  Ref{"charter", [id], ""}

def Ref.person() -> Ref:
  Ref{"person", Nil{}, ""}

def Ref.pr(project: String, number: String) -> Ref:
  Ref{"pr", [project, number], ""}

def Ref.file(project: String, path: List<&2, String>) -> Ref:
  Ref{"file", project <> path, ""}

def Ref.mail(conv: String, id: String) -> Ref:
  Ref{"mail", [conv, id], ""}

def Ref.decision(bot: String, id: String) -> Ref:
  Ref{"decision", [bot, id], ""}

def Ref.msg(thread: String, msg: String) -> Ref:
  Ref{"thread", [thread], msg}

# A link id is a word, not the address prefix inside a conversation id.
def Ref.link.go(s: String, ok: Bool) -> Bool:
  match s:
    case SNil{}: ok
    case SCon{Chr{+u}, t}: Ref.link.go(t, Bool.and(ok, Ref.safe(u)))

def Ref.link(+s: String) -> Bool:
  Ref.link.go(s, Bool.not(String.is_empty(s)))

def Ref.remote.part(c: RefCut) -> String:
  match c:
    case RefCut{+link, +id}: Bool.pick(String, Bool.and(Ref.link(link), Bool.not(String.is_empty(id))), link, "")

def Ref.remote(r: Ref) -> String:
  match r:
    case Ref{_, Nil{}, _}: ""
    case Ref{_, id <> _, _}: Ref.remote.part(Ref.cut(id, '~', ""))

# Exact digit groups, not Nat.read (normalization must stay cheap).
def Ref.digits.go(s: String, ok: Bool) -> Bool:
  match s:
    case SNil{}: ok
    case SCon{+c, t}: Ref.digits.go(t, Bool.and(ok, Char.is_digit(c)))

def Ref.digits(+s: String) -> Bool:
  Ref.digits.go(s, Bool.not(String.is_empty(s)))

def Ref.bare.groups(xs: List<&2, String>, +kind: String, id: String) -> Maybe<&2, Ref>:
  match xs:
    case a <> b <> Nil{}: Ref.accept(Bool.and(Bool.not(String.is_empty(kind)), Bool.and(Ref.digits(a), Ref.digits(b))), Ref{kind, [id], ""})
    case a <> b <> c <> Nil{}: Ref.accept(Bool.and(String.eq(kind, "bot"), Bool.and(Ref.digits(a), Bool.and(Ref.digits(b), Ref.digits(c)))), Ref{"task", [id], ""})
    case _: None{}

def Ref.bare.kind(c: Char) -> String:
  match c:
    case 't': "thread"
    case 'b': "bot"
    case 'p': "project"
    case 'r': "room"
    case 'o': "routine"
    case _: ""

def Ref.bare.local(s: String, +id: String) -> Maybe<&2, Ref>:
  match s:
    case SNil{}: None{}
    case SCon{+c, t}: Ref.bare.groups(Ref.split(t, '-'), Ref.bare.kind(c), id)

def Ref.bare.route(remote: Bool, +link: String, tail: String, +id: String) -> Maybe<&2, Ref>:
  match remote:
    case False{}: Ref.bare.local(id, id)
    case True{}: Ref.bare.local(Bool.pick(String, Ref.link(link), tail, ""), id)

def Ref.bare.remote(c: RefCut, +id: String) -> Maybe<&2, Ref>:
  match c:
    case RefCut{+link, +tail}:
      # No ~ means a local id; a remote link must itself be nonempty.
      Ref.bare.route(String.contains(id, "~"), link, tail, id)

def Ref.bare(+w: String) -> Maybe<&2, Ref>:
  Ref.bare.remote(Ref.cut(w, '~', ""), w)
