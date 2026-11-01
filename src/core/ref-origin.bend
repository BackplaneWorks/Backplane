import Base
import ./ref.bend as R
import ./links.bend as L

# Read projection only. Canonical messages and raw far items stay in the
# writer's id space. Translate explicit thread/project URIs once while
# folding that writer's mirror, using the same direction as Fr.sender.
def Origin.id(+id: String, +link: String) -> String:
  Bool.pick(String, String.contains(id, "~"),
    Bool.pick(String, Bool.and(String.starts_with(id, link ++ "~"), Nat.is_gt(String.length(id), 1n+String.length(link))), String.drop(id, 1n+String.length(link)), id), link ++ "~" ++ id)

def Origin.uri.id(+next: String, +kind: String, +id: String, frag: String, +original: String) -> String:
  Bool.pick(String, String.eq(next, id), original, R.Ref.uri(R.Ref{kind, [next], frag}))

def Origin.uri.ref(r: R.Ref, +link: String, +original: String) -> String:
  match r:
    case R.Ref{+kind, +id <> Nil{}, frag}:
      Bool.pick(String, Bool.or(String.eq(kind, "thread"), String.eq(kind, "project")), Origin.uri.id(Origin.id(id, link), kind, id, frag, original), original)
    case _: original

def Origin.uri.parsed(m: Maybe<&2, R.Ref>, link: String, +original: String) -> String:
  match m:
    case None{}: original
    case Some{r}: Origin.uri.ref(r, link, original)

def Origin.uri(+uri: String, +link: String) -> String:
  Origin.uri.parsed(R.Ref.parse(uri), link, uri)

def Origin.trim(tr: L.Tr, +link: String, acc: String) -> String:
  match tr:
    case L.Tr{_, +url, moved}: String.append(moved, String.append(String.reverse(Origin.uri(url, link)), acc))

# Parentheses already delimit URI tokens, so trimming needs no repeated
# parenthesis counts on long punctuation runs.
def Origin.tr.cut(done: Bool, c: Char) -> Bool:
  match done:
    case True{}: False{}
    case False{}: L.Link.punct(Char.to_u32(c))

def Origin.tr(s: String, +st: L.Tr) -> L.Tr:
  match s:
    case SNil{}: st
    case SCon{+c, tail}: Origin.tr(tail, L.Link.tr.k(Origin.tr.cut(L.Link.tr.done(st), c), c, st))

def Origin.flush(url: Bool, token: String, +link: String, acc: String) -> String:
  match url:
    case False{}: acc
    case True{}: Origin.trim(Origin.tr(token, L.Tr{False{}, "", ""}), link, acc)

# Backtick runs fence both inline and fenced code. Preserve every literal
# byte there, and retain punctuation/labels/foreign links everywhere.
def Origin.mode(+code: Nat, +ticks: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(ticks, 0n), code, Bool.pick(Nat, Nat.is_eq(code, 0n), ticks, Bool.pick(Nat, Nat.is_eq(code, ticks), 0n, code)))

def Origin.boundary(acc: String) -> Bool:
  match acc:
    case SNil{}: True{}
    case SCon{+c, _}:
      Bool.not(Bool.or(Bool.or(Char.is_alpha(c), Char.is_digit(c)), Bool.or(Char.is_eq(c, '/'), Bool.or(Char.is_eq(c, ':'), Bool.or(Char.is_eq(c, '_'), Char.is_eq(c, '%'))))))

def Origin.stops(+c: Char) -> Bool:
  Bool.or(L.Link.stops(c), Bool.or(Bool.or(Char.is_eq(c, '('), Char.is_eq(c, ')')), Bool.or(Bool.or(Char.is_eq(c, '['), Char.is_eq(c, ']')), Char.is_eq(c, '\''))))

type Scan is Data:
  Scan{token: String, acc: String, code: Nat, ticks: Nat, url: Bool}

def Origin.text(start: Bool, c: Char, acc: String, code: Nat) -> Scan:
  match start:
    case True{}: Scan{SCon{c, ""}, acc, code, 0n, True{}}
    case False{}: Scan{"", SCon{c, acc}, code, 0n, False{}}

def Origin.url(stop: Bool, +c: Char, token: String, acc: String, +link: String, code: Nat) -> Scan:
  match stop:
    case False{}: Scan{SCon{c, token}, acc, code, 0n, True{}}
    case True{}: Scan{"", SCon{c, Origin.flush(True{}, token, link, acc)}, code, 0n, False{}}

# whether a reference starts at c: the cheap test first. Building
# SCon{c, tail} to ask String.starts_with of it (a string as long as the
# rest of the text, at every character) and reading the head of the
# accumulator (a rope grown a character at a time) made a scan quadratic:
# a hub folding its mirrors at boot peaked 5 GB above its own state
def Origin.start.at(+mode: Nat, acc: String, at: Bool) -> Bool:
  match at:
    case True{}: Bool.and(Nat.is_eq(mode, 0n), Origin.boundary(acc))
    case False{}: False{}

def Origin.start.b(b: Bool, +mode: Nat, acc: String, +tail: String) -> Bool:
  match b:
    case True{}: Origin.start.at(mode, acc, String.starts_with(tail, "ackplane://"))
    case False{}: False{}

def Origin.start(+c: Char, +tail: String, +acc: String, +mode: Nat) -> Bool:
  Origin.start.b(Char.is_eq(c, 'b'), mode, acc, tail)

def Origin.step(tick: Bool, +c: Char, +tail: String, +link: String, st: Scan) -> Scan:
  match tick st:
    case True{} Scan{token, acc, code, ticks, url}:
      Scan{"", SCon{c, Origin.flush(url, token, link, acc)}, code, 1n+ticks, False{}}
    case False{} Scan{token, acc, code, ticks, True{}}:
      Origin.url(Origin.stops(c), c, token, acc, link, Origin.mode(code, ticks))
    case False{} Scan{_, +acc, +code, +ticks, False{}}:
      Origin.text(Origin.start(c, tail, acc, Origin.mode(code, ticks)), c, acc, Origin.mode(code, ticks))

def Origin.scan(s: String, +link: String, st: Scan) -> String:
  match s st:
    case SNil{} Scan{token, acc, _, _, url}: String.reverse(Origin.flush(url, token, link, acc))
    case SCon{+c, +tail} _: Origin.scan(tail, link, Origin.step(Char.is_eq(c, '`'), c, tail, link, st))

def Origin.text_links(valid: Bool, s: String, link: String) -> String:
  match valid:
    case False{}: s
    case True{}: Origin.scan(s, link, Scan{"", "", 0n, 0n, False{}})

def Origin.show(+text: String, +link: String) -> String:
  Origin.text_links(Bool.and(R.Ref.link(link), String.contains(text, "backplane://")), text, link)
