import Base
import ./model.bend as M
import ./thin.bend as T
import ./stamp.bend as STP
import ./tz.bend as TZ
import ./sched.bend as SCH

# A search's results as shown (pure)
# ==================================
# The top bar's search (Ctrl+Shift+F) looks through every thread
# (thin.bend's Index.search.all). Each result shows the words around its
# match with the query's words lit (Snip.*), and when it was said; the
# results come grouped (Hits.order): the open thread's first, then by
# project and by thread, each group where its newest result was. The hub
# sends them so (server/thin-view.bend's Search.found); clients draw
# them as they come.

# Snippets
# --------
# A result's text cut around the first of the query's words in it (a
# whole word, read as thin.bend's Words.of reads them: ASCII letters in
# either case): before it, the rest of its line and the line above; after
# it, the rest of its line and the line below; "…" where a line was cut.
# Lines stay lines: a reply of short lines reads as lines.

# characters looked through for the first match (a match further into a
# long text shows the text's start)
def Snip.budget() -> Nat:
  65536n

def Snip.before() -> Nat:
  80n

def Snip.after() -> Nat:
  160n

def Snip.lower(c: Char) -> U32:
  T.Words.code(Char.to_lower(c))

def Snip.nl(c: Char) -> Bool:
  U32.is_eq(T.Words.code(c), 10)

# the walk to the match. pre: the text before the word being read,
# reversed; word, low: that word as written and lowered, reversed; left:
# characters still to look through. Once found, post: the text after the
# word, reversed, at most Snip.after() characters (cut: there was more)
type SnipWalk is Data:
  SnipRead{pre: String, word: String, low: String, left: Nat}
  SnipHit{pre: String, word: String, post: String, left: Nat}
  SnipDone{pre: String, word: String, post: String, cut: Bool}
  SnipMiss{}

# a word ended at c: one of the query's, or on to the next
def Snip.word(hit: Bool, +c: Char, +pre: String, +word: String, +left: Nat) -> SnipWalk:
  match hit:
    case True{}: SnipHit{pre, String.reverse(word), SCon{c, SNil{}}, Nat.sub(Snip.after(), 1n)}
    case False{}: SnipRead{SCon{c, word ++ pre}, "", "", left}

def Snip.read(letter: Bool, +c: Char, +terms: Map<&2, Bool>, +pre: String, +word: String, +low: String, +left: Nat) -> SnipWalk:
  match letter:
    case True{}: SnipRead{pre, SCon{c, word}, SCon{Chr{Snip.lower(c)}, low}, left}
    case False{}: Snip.word(T.Word.has(terms, String.reverse(low)), c, pre, word, left)

def Snip.take(+c: Char, +pre: String, +word: String, +post: String, left: Nat) -> SnipWalk:
  match left:
    case 0n: SnipDone{pre, word, post, True{}}
    case 1n+k: SnipHit{pre, word, SCon{c, post}, k}

def Snip.step(+terms: Map<&2, Bool>, +c: Char, w: SnipWalk) -> SnipWalk:
  match w:
    case SnipRead{_, _, _, 0n}: SnipMiss{}
    case SnipRead{+pre, +word, +low, 1n+left}: Snip.read(T.Word.char(Snip.lower(c)), c, terms, pre, word, low, left)
    case SnipHit{+pre, +word, +post, left}: Snip.take(c, pre, word, post, left)
    case SnipDone{pre, word, post, cut}: SnipDone{pre, word, post, cut}
    case SnipMiss{}: SnipMiss{}

# the text ended inside a word: the last word may be the match
def Snip.last(hit: Bool, +pre: String, +word: String) -> SnipWalk:
  match hit:
    case True{}: SnipDone{pre, String.reverse(word), "", False{}}
    case False{}: SnipMiss{}

def Snip.go(s: String, +terms: Map<&2, Bool>, w: SnipWalk) -> SnipWalk:
  match s w:
    case _ SnipDone{pre, word, post, cut}: SnipDone{pre, word, post, cut}
    case _ SnipMiss{}: SnipMiss{}
    case SNil{} SnipRead{+pre, +word, +low, _}: Snip.last(T.Word.has(terms, String.reverse(low)), pre, word)
    case SNil{} SnipHit{pre, word, post, _}: SnipDone{pre, word, post, False{}}
    case SCon{h, tail} SnipRead{pre, word, low, left}: Snip.go(tail, terms, Snip.step(terms, h, SnipRead{pre, word, low, left}))
    case SCon{h, tail} SnipHit{pre, word, post, left}: Snip.go(tail, terms, Snip.step(terms, h, SnipHit{pre, word, post, left}))

# Lines kept: a walk taking at most n characters and stopping before the
# second line break. acc: what it took, in the order opposite to the
# input's; why it stopped: 0 the input ended, 1 a line break, 2 the limit
type SnipTake is Data:
  SnipTaking{acc: String, n: Nat, nl: Bool}
  SnipTook{acc: String, why: Nat}

def Snip.taking(stop: Bool, +c: Char, +acc: String, +n: Nat, +nl: Bool) -> SnipTake:
  match stop:
    case True{}: SnipTook{acc, 1n}
    case False{}: SnipTaking{SCon{c, acc}, n, Bool.or(nl, Snip.nl(c))}

def Snip.take.step(+c: Char, t: SnipTake) -> SnipTake:
  match t:
    case SnipTaking{+acc, 0n, _}: SnipTook{acc, 2n}
    case SnipTaking{+acc, 1n+n, +nl}: Snip.taking(Bool.and(nl, Snip.nl(c)), c, acc, n, nl)
    case SnipTook{acc, why}: SnipTook{acc, why}

def Snip.lines(s: String, t: SnipTake) -> SnipTake:
  match s t:
    case _ SnipTook{acc, why}: SnipTook{acc, why}
    case SNil{} SnipTaking{acc, _, _}: SnipTook{acc, 0n}
    case SCon{h, tail} SnipTaking{acc, n, nl}: Snip.lines(tail, Snip.take.step(h, SnipTaking{acc, n, nl}))

def Snip.took(t: SnipTake) -> String:
  match t:
    case SnipTook{acc, _}: acc
    case SnipTaking{acc, _, _}: acc

def Snip.why(t: SnipTake) -> Nat:
  match t:
    case SnipTook{_, why}: why
    case SnipTaking{_, _, _}: 0n

def Snip.starts.nl(s: String) -> Bool:
  match s:
    case SNil{}: False{}
    case SCon{h, _}: Snip.nl(h)

# line breaks dropped from a text's start (nl: it starts with one)
def Snip.trim(s: String, nl: Bool) -> String:
  match s nl:
    case SCon{_, +tail} True{}: Snip.trim(tail, Snip.starts.nl(tail))
    case SCon{h, tail} False{}: SCon{h, tail}
    case SNil{} _: SNil{}

def Snip.ltrim(+s: String) -> String:
  Snip.trim(s, Snip.starts.nl(s))

def Snip.rtrim(+s: String) -> String:
  String.reverse(Snip.ltrim(String.reverse(s)))

# Pieces: the snippet as plain text and matches by turns, plain first and
# last ([plain, match, plain, ...]); a match is a word of the query's
type SnipMark is Data:
  SnipMark{plain: String, word: String, low: String, out: List<&2, String>}

def Snip.flush(hit: Bool, +plain: String, +word: String, out: List<&2, String>) -> SnipMark:
  match hit:
    case True{}: SnipMark{"", "", "", String.reverse(word) <> String.reverse(plain) <> out}
    case False{}: SnipMark{word ++ plain, "", "", out}

def Snip.flushed(+terms: Map<&2, Bool>, m: SnipMark) -> SnipMark:
  match m:
    case SnipMark{+plain, +word, +low, out}: Snip.flush(T.Word.has(terms, String.reverse(low)), plain, word, out)

def Snip.plain(+c: Char, m: SnipMark) -> SnipMark:
  match m:
    case SnipMark{plain, word, low, out}: SnipMark{SCon{c, plain}, word, low, out}

def Snip.mark.k(letter: Bool, +terms: Map<&2, Bool>, +c: Char, m: SnipMark) -> SnipMark:
  match letter m:
    case True{} SnipMark{plain, word, low, out}: SnipMark{plain, SCon{c, word}, SCon{Chr{Snip.lower(c)}, low}, out}
    case False{} _: Snip.plain(c, Snip.flushed(terms, m))

def Snip.marks.end(m: SnipMark) -> List<&2, String>:
  match m:
    case SnipMark{plain, _, _, out}: List.reverse(&2, String, String.reverse(plain) <> out)

def Snip.marks(s: String, +terms: Map<&2, Bool>, m: SnipMark) -> List<&2, String>:
  match s:
    case SNil{}: Snip.marks.end(Snip.flushed(terms, m))
    case SCon{+h, tail}: Snip.marks(tail, terms, Snip.mark.k(T.Word.char(Snip.lower(h)), terms, h, m))

def Snip.pieces(+terms: Map<&2, Bool>, s: String) -> List<&2, String>:
  Snip.marks(s, terms, SnipMark{"", "", "", Nil{}})

# "…" before the first piece, after the last
def Snip.dots(cut: Bool) -> String:
  Bool.pick(String, cut, "…", "")

def Snip.head(+dots: String, ps: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}: [dots]
    case p <> tail: (dots ++ p) <> tail

def Snip.head.end(ps: List<&2, String>, +dots: String) -> List<&2, String>:
  match ps:
    case Nil{}: [dots]
    case p <> tail: (p ++ dots) <> tail

def Snip.tail(ps: List<&2, String>, +dots: String) -> List<&2, String>:
  List.reverse(&2, String, Snip.head.end(List.reverse(&2, String, ps), dots))

def Snip.dress(+lead: Bool, +trail: Bool, ps: List<&2, String>) -> List<&2, String>:
  Snip.tail(Snip.head(Snip.dots(lead), ps), Snip.dots(trail))

# the snippet around a match: pre reversed, post reversed (walk: the
# text went on past it)
def Snip.around(+terms: Map<&2, Bool>, +pre: String, +word: String, +post: String, +walk: Bool) -> List<&2, String>:
  +before = Snip.lines(pre, SnipTaking{"", Snip.before(), False{}})
  +after = Snip.lines(String.reverse(post), SnipTaking{"", Snip.after(), False{}})
  Snip.dress(Nat.is_eq(Snip.why(before), 2n), Bool.or(Nat.is_eq(Snip.why(after), 2n), Bool.and(Nat.is_eq(Snip.why(after), 0n), walk)),
    Snip.pieces(terms, Snip.ltrim(Snip.took(before)) ++ word ++ Snip.rtrim(String.reverse(Snip.took(after)))))

# no match found (past the budget, or in no whole word): the text's start
def Snip.start(+terms: Map<&2, Bool>, +text: String) -> List<&2, String>:
  +lines = Snip.lines(text, SnipTaking{"", Snip.after(), False{}})
  Snip.dress(False{}, Nat.is_eq(Snip.why(lines), 2n), Snip.pieces(terms, Snip.rtrim(String.reverse(Snip.took(lines)))))

def Snip.of.k(w: SnipWalk, +terms: Map<&2, Bool>, +text: String) -> List<&2, String>:
  match w:
    case SnipDone{+pre, +word, +post, +cut}: Snip.around(terms, pre, word, post, cut)
    case _: Snip.start(terms, text)

# a result's snippet: its pieces, plain and lit by turns (terms: the
# query's words, thin.bend's Words.of)
def Snip.of(+terms: Map<&2, Bool>, +text: String) -> List<&2, String>:
  Snip.of.k(Snip.go(text, terms, SnipRead{"", "", "", Snip.budget()}), terms, text)

# Grouping
# --------
# a result as the grouping sees it: its row (thin.bend's ordinal key), its
# thread and the thread's project
type HitAt is Data:
  HitAt{id: String, thread: String, project: String}

def HitAt.id(h: HitAt) -> String:
  match h:
    case HitAt{id, _, _}: id

def Hits.put(keep: Bool, +h: HitAt, acc: List<&2, HitAt>) -> List<&2, HitAt>:
  match keep:
    case True{}: h <> acc
    case False{}: acc

# the results in thread th (want) or the others (not want), in order
def Hits.in.go(hs: List<&2, HitAt>, +th: String, +want: Bool, acc: List<&2, HitAt>) -> List<&2, HitAt>:
  match hs:
    case Nil{}: List.reverse(&2, HitAt, acc)
    case HitAt{+id, +t, +p} <> tail: Hits.in.go(tail, th, want, Hits.put(Bool.not(Bool.xor(String.eq(t, th), want)), HitAt{id, t, p}, acc))

def Hits.in(+hs: List<&2, HitAt>, +th: String) -> List<&2, HitAt>:
  Hits.in.go(hs, th, True{}, Nil{})

def Hits.out(+hs: List<&2, HitAt>, +th: String) -> List<&2, HitAt>:
  Hits.in.go(hs, th, False{}, Nil{})

# keys each once, in the order they first come (newest first)
type HitKeys is Data:
  HitKeys{seen: Map<&2, Bool>, keys: List<&2, String>}

def Hits.key.k(old: Bool, +k: String, seen: Map<&2, Bool>, keys: List<&2, String>) -> HitKeys:
  match old:
    case True{}: HitKeys{seen, keys}
    case False{}: HitKeys{Map.set(&2, Bool, seen, k, True{}), k <> keys}

def Hits.key(want: Bool, +k: String, s: HitKeys) -> HitKeys:
  match want s:
    case False{} _: s
    case True{} HitKeys{+seen, keys}: Hits.key.k(T.Word.has(seen, k), k, seen, keys)

def Hits.keys(s: HitKeys) -> List<&2, String>:
  match s:
    case HitKeys{_, keys}: List.reverse(&2, String, keys)

# the projects of the results
def Hits.projects.go(hs: List<&2, HitAt>, s: HitKeys) -> List<&2, String>:
  match hs:
    case Nil{}: Hits.keys(s)
    case HitAt{_, _, +p} <> tail: Hits.projects.go(tail, Hits.key(True{}, p, s))

def Hits.projects(+hs: List<&2, HitAt>) -> List<&2, String>:
  Hits.projects.go(hs, HitKeys{Map.new(&2, Bool), Nil{}})

# the threads of project pr among the results
def Hits.threads.go(hs: List<&2, HitAt>, +pr: String, s: HitKeys) -> List<&2, String>:
  match hs:
    case Nil{}: Hits.keys(s)
    case HitAt{_, +t, +p} <> tail: Hits.threads.go(tail, pr, Hits.key(String.eq(p, pr), t, s))

def Hits.threads(+hs: List<&2, HitAt>, +pr: String) -> List<&2, String>:
  Hits.threads.go(hs, pr, HitKeys{Map.new(&2, Bool), Nil{}})

# each thread's results, thread after thread
def Hits.by.threads(ts: List<&2, String>, +hs: List<&2, HitAt>) -> List<&2, HitAt>:
  match ts:
    case Nil{}: Nil{}
    case +t <> tail: List.append(&2, HitAt, Hits.in(hs, t), Hits.by.threads(tail, hs))

def Hits.by.projects(ps: List<&2, String>, +hs: List<&2, HitAt>) -> List<&2, HitAt>:
  match ps:
    case Nil{}: Nil{}
    case +p <> tail: List.append(&2, HitAt, Hits.by.threads(Hits.threads(hs, p), hs), Hits.by.projects(tail, hs))

# a page of results (newest first) in the order shown: those in the open
# thread (here) first, then the rest by project and by thread, a project
# and a thread where its newest result was; within a thread, newest
# first (test/hits_test.bend)
def Hits.order(+hs: List<&2, HitAt>, +here: String) -> List<&2, HitAt>:
  +rest = Hits.out(hs, here)
  List.append(&2, HitAt, Hits.in(hs, here), Hits.by.projects(Hits.projects(rest), rest))

# When a result was said: "14:05" today, "Mon 13 Oct 14:05" before, on the
# person's clock (stamp.bend)
def Hits.when(+st: M.State, +now: Nat, +at: Nat) -> String:
  +r = SCH.Sched.person(st, 0n, True{})
  STP.Stamp.clock(r, Nat.div(TZ.Tz.wall(r, now), 86400n), at)
