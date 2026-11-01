import Base
import ./i18n.bend as I
import ./msgs.bend as MSG

# A draft too long for every view request (hub view protocol H). A page's
# view request carries the selected thread's draft so the hub can derive
# what it shows from it (the slash rows, the Give label, the reference
# popup). Over Bound.whole() bytes the page (thin-host.js's
# boundedDraft, this file's twin) sends only its first Bound.head() and
# last Bound.tail() code points, joined by " … " when the cut middle held
# a space or line break and by "…" when it did not, so every word the hub
# reads at either end is the same, and marks it in the drafts as
# "@draft.bound": "<utf8 bytes> <hash>" ("" otherwise). Send carries the
# whole text when it is within Bound.send() bytes; a draft the hub holds
# bounded is never sent (client.bend's Ui.send) and never adopted by the
# page, which keeps (and saves) its own full text. Over Bound.send() the
# composer says so (Bound.say). Laws draft_bound_*.

def Bound.whole() -> Nat:
  65536n

def Bound.head() -> Nat:
  16384n

def Bound.tail() -> Nat:
  16384n

# a message's bound (thin.js's viewCommand bounds a send's text the same)
def Bound.send() -> Nat:
  131072n

# the separators of Refs.token's words (refs.bend's Refs.sep)
def Bound.sep(+c: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(c, 32), U32.is_eq(c, 10)), Bool.or(U32.is_eq(c, 9), U32.is_eq(c, 13)))

def Bound.spaced(s: String, found: Bool) -> Bool:
  match s found:
    case _ True{}: True{}
    case SNil{} False{}: False{}
    case SCon{Chr{+c}, t} False{}: Bound.spaced(t, Bound.sep(c))

def Bound.join(spaced: Bool) -> String:
  match spaced:
    case True{}: " … "
    case False{}: "…"

# the draft as a request carries it, kept head and tail code points (for a
# draft longer than both; the page's twin does the same at full size)
def Bound.text(+s: String, +head: Nat, +tail: Nat) -> String:
  +n = String.length(s)
  String.take(s, head) ++ Bound.join(Bound.spaced(String.take(String.drop(s, head), Nat.sub(Nat.sub(n, head), tail)), False{})) ++ String.drop(s, Nat.sub(n, tail))

# the marker's byte count (its digits up to the first space; 0 for none)
def Bound.digit(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))

def Bound.digits.go(s: String, +n: Nat, stop: Bool) -> Nat:
  match s stop:
    case _ True{}: n
    case SNil{} False{}: n
    case SCon{Chr{+c}, t} False{}:
      Bound.digits.go(t, Bool.pick(Nat, Bound.digit(c), Nat.add(Nat.mul(n, 10n), U32.to_nat(U32.sub(c, 48))), n), Bool.not(Bound.digit(c)))

def Bound.bytes(+marker: String) -> Nat:
  Bound.digits.go(marker, 0n, False{})

# whether the draft this view holds is bounded (a marker is there)
def Bound.is(+marker: String) -> Bool:
  Bool.not(String.is_empty(marker))

# kilobytes, rounded up
def Bound.kb(+n: Nat) -> Nat:
  Nat.div(Nat.add(n, 1023n), 1024n)

def Bound.say.k(over: Bool, +l: I.Loc, +bytes: Nat) -> String:
  match over:
    case True{}: MSG.T.composer.draft.limit(l, Nat.show(Bound.kb(bytes)), Nat.show(Bound.kb(Bound.send())))
    case False{}: ""

# what the composer says of a bounded draft: "" within a message's bound,
# else its size and the bound
def Bound.say(+l: I.Loc, +marker: String) -> String:
  Bound.say.k(Nat.is_gt(Bound.bytes(marker), Bound.send()), l, Bound.bytes(marker))

# why Send does nothing with a bounded draft (one within the bound is sent
# whole by the page, so a bounded one is always over it)
def Bound.refuse(+l: I.Loc, +marker: String) -> String:
  MSG.T.composer.draft.limit(l, Nat.show(Bound.kb(Bound.bytes(marker))), Nat.show(Bound.kb(Bound.send())))
