import Base
import ../core/client.bend as U
import ../core/model.bend as M
import ../core/mail.bend as ML

# One private mailbox projection per hub, not per request or renderer.
# Bootstrap walks canonical conversations once, never the historical log.
# A successful commit revisits only its affected conversation identities.
type Cache is Data:
  Cache{index: U.MailvIndex, revision: Nat}

def Cache.empty() -> Cache:
  Cache{U.Mailv.index.empty(), 0n}

def Cache.index(c: Cache) -> U.MailvIndex:
  match c:
    case Cache{i, _}: i

def Cache.revision(c: Cache) -> Nat:
  match c:
    case Cache{_, r}: r

def Cache.ui(+st: M.State, index: U.MailvIndex) -> U.Ui:
  +u = U.Ui.adopt.raw(U.Ui.init(0n), st)
  U.Mailv.put(u, U.Mailv.index.set(U.Mailv.data(u), index))

def Cache.seed(ids: List<&2, String>, u: U.Ui) -> U.MailvIndex:
  match ids:
    case Nil{}: U.Mailv.index(u)
    case id <> tail: Cache.seed(tail, U.Mailv.index.conv(u, id))

def Cache.of(+st: M.State) -> Cache:
  Cache{Cache.seed(Map.keys(&2, ML.Conv, ML.Box.convs(M.State.mail(st))), Cache.ui(st, U.Mailv.index.empty())), M.State.seq(st)}

def Cache.relevant(c: M.Change) -> Bool:
  Bool.not(String.is_empty(U.Mailv.change_conv(c)))

def Cache.any(cs: List<&2, M.Change>) -> Bool:
  match cs:
    case Nil{}: False{}
    case c <> tail: Bool.or(Cache.relevant(c), Cache.any(tail))

def Cache.apply(cs: List<&2, M.Change>, u: U.Ui) -> U.MailvIndex:
  match cs:
    case Nil{}: U.Mailv.index(u)
    case c <> tail: Cache.apply(tail, U.Mailv.index.update(c, u))

def Cache.committed_if(changed: Bool, +cs: List<&2, M.Change>, +st: M.State, +cache: Cache) -> Cache:
  match changed:
    case False{}: cache
    case True{}: Cache{Cache.apply(cs, Cache.ui(st, Cache.index(cache))), M.State.seq(st)}

def Cache.committed(+cs: List<&2, M.Change>, st: M.State, cache: Cache) -> Cache:
  Cache.committed_if(Cache.any(cs), cs, st, cache)

# Installing a newer projection changes only the derived index and read
# invalidation markers. Requests, typed pages/bodies, drafts and outbox stay
# private to their existing actor. An outstanding old list keeps its slot;
# its captured old token cannot suppress the later current refresh.
def Cache.body_changed(changed: Bool, u: U.Ui) -> U.Ui:
  match changed:
    case False{}: u
    case True{}: U.Ui.scratch_set(u, "@mr.read.dirty", "1")

def Cache.install_if(same: Bool, +cache: Cache, +u: U.Ui) -> U.Ui:
  match same:
    case True{}: u
    case False{}:
      +version = Nat.show(Cache.revision(cache))
      U.Ui.scratch_set(U.Ui.scratch_set(U.Mailv.put(u, U.Mailv.index.set(U.Mailv.data(u), Cache.index(cache))), "@mr.canonical.revision", version), "@mr.dirty", "canonical:" ++ version)

def Cache.install(+cache: Cache, +u: U.Ui) -> U.Ui:
  Cache.install_if(String.eq(U.Ui.scratch(u, "@mr.canonical.revision"), Nat.show(Cache.revision(cache))), cache, u)

def Cache.refresh(+cache: Cache, +u: U.Ui, +old: U.Ui) -> U.Ui:
  +conv = U.Ui.scratch(u, "@mr.conv")
  Cache.body_changed(Bool.and(Bool.not(String.is_empty(conv)), Bool.not(String.eq(
    ML.Mail.id(ML.Box.newest(M.State.mail(U.Ui.state(old)), conv)),
    ML.Mail.id(ML.Box.newest(M.State.mail(U.Ui.state(u)), conv))))), Cache.install(cache, u))


# ---- the cache on demand ----
# Cache.of walks every conversation: the hub must not do it before it
# listens. Later keeps the state the hub booted with and, in order, each
# commit since with the state after it; Lazy.force gives what the eager
# boot (Cache.of) followed by each Cache.committed would have held (laws
# thin_mail_lazy_*).
type Pending is Data:
  Pending{cs: List<&2, M.Change>, st: M.State}

type Lazy is Data:
  Built{cache: Cache}
  Later{st: M.State, done: List<&2, Pending>}

def Lazy.of(st: M.State) -> Lazy:
  Later{st, Nil{}}

def Lazy.replay(ps: List<&2, Pending>, c: Cache) -> Cache:
  match ps:
    case Nil{}: c
    case Pending{cs, st} <> tail: Lazy.replay(tail, Cache.committed(cs, st, c))

def Lazy.committed(cs: List<&2, M.Change>, st: M.State, l: Lazy) -> Lazy:
  match l:
    case Built{c}: Built{Cache.committed(cs, st, c)}
    case Later{base, done}: Later{base, List.append(&2, Pending, done, [Pending{cs, st}])}

def Lazy.force(l: Lazy) -> Cache:
  match l:
    case Built{c}: c
    case Later{st, done}: Lazy.replay(done, Cache.of(st))

def Lazy.ready(l: Lazy) -> Bool:
  match l:
    case Built{_}: True{}
    case Later{_, _}: False{}

# `c` is Cache.of(the boot state), built away from the hub: the commits it
# missed are applied on arrival; a cache already built is kept
def Lazy.warm(c: Cache, l: Lazy) -> Lazy:
  match l:
    case Built{b}: Built{b}
    case Later{_, done}: Built{Lazy.replay(done, c)}
