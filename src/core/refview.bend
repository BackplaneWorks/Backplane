import Base
import ./model.bend as M
import ./bot.bend as BT
import ./ref.bend as R
import ./far.bend as FR
import ./fm.bend as FM
import ./json.bend as J

# Drawing resolves only object maps and indexed settings, never timelines.
type Card is Data:
  Card{uri: String, kind: String, icon: String, title: String, sub: String, status: String, machine: String}

def Card.id(+r: R.Ref) -> String:
  match r:
    case R.Ref{_, +xs, _}: Maybe.default(&2, String, List.head(&2, String, xs), "")

def Card.project.title.k(+p: M.Project) -> String:
  match p:
    case M.Project{_, +title, _, _, _}: title

def Card.project.root.k(+p: M.Project) -> String:
  match p:
    case M.Project{_, _, +root, _, _}: root

def Card.project.live.k(+p: M.Project) -> Bool:
  match p:
    case M.Project{_, _, _, _, +gone}: Bool.not(gone)

def Card.thread.project(+t: M.Thread) -> String:
  match t:
    case M.Thread{_, +p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: p

def Card.thread.live(+t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +gone}: Bool.not(gone)

def Card.thread.title(+t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, +s, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: Bool.pick(String, String.is_empty(s), "New thread", s)

def Card.accept(+ok: Bool, +c: Card) -> Maybe<&2, Card>:
  match ok:
    case True{}: Some{c}
    case False{}: None{}

def Card.machine.label.remote(remote: Bool, st: M.State, id: String) -> String:
  match remote:
    case True{}: FR.Fr.name(st, id)
    case False{}: ""

def Card.machine.label(st: M.State, +r: R.Ref) -> String:
  Card.machine.label.remote(Bool.not(String.is_empty(R.Ref.remote(r))), st, Card.id(r))

def Card.make(+st: M.State, +r: R.Ref, +icon: String, +title: String, +sub: String, +status: String) -> Card:
  match r:
    case R.Ref{+k, +xs, +f}: Card{R.Ref.uri(R.Ref{k, xs, f}), k, icon, title, sub, status, Card.machine.label(st, R.Ref{k, xs, f})}

def Card.bot(+st: M.State, +r: R.Ref, +b: BT.Bot) -> Maybe<&2, Card>:
  Card.accept(BT.Bot.live(b), Card.make(st, r, "cpu", BT.Bot.name(b), "bot", Bool.pick(String, M.Work.turn.runs(M.Thread.turn(M.State.thread.get(st, BT.Bot.thread(b)))), "working", "idle")))

def Card.room(+st: M.State, +r: R.Ref, +b: BT.Room) -> Maybe<&2, Card>:
  Card.accept(BT.Room.live(b), Card.make(st, r, "message-square", BT.Room.name(b), "room", ""))

def Card.routine(+st: M.State, +r: R.Ref, +b: BT.Routine) -> Maybe<&2, Card>:
  Card.accept(BT.Routine.live(b), Card.make(st, r, "repeat", BT.Routine.name(b), BT.Bot.name(BT.World.bot(M.State.bots(st), BT.Routine.bot(b))), Bool.pick(String, BT.Routine.on(b), "on", "off")))

# A task's status as the board writes it (tboard.bend's Tb.status.norm,
# law card_task_status_agrees): not imported, since tboard imports the
# hub and every client would carry it
def Card.task.status.k(+l: String) -> String:
  Bool.pick(String, R.Ref.has(["doing", "blocked", "todo", "done", "dropped"], l), l, "todo")

def Card.task.status(+s: String) -> String:
  Card.task.status.k(String.to_lower(String.take(String.trim(s), 20n)))

# Read only this indexed item's two drawn fields; Tb.get would scan every
# item.
def Card.task.json(+st: M.State, +r: R.Ref, +o: Maybe<&2, J.Json>) -> Maybe<&2, Card>:
  match o:
    case None{}: None{}
    case Some{+j}: Some{Card.make(st, r, "circle-check", J.Json.get_str(j, "title", ""), "", Card.task.status(J.Json.get_str(j, "status", "todo")))}

def Card.work.find.go(+ls: List<&2, String>, +id: String, +found: String) -> String:
  match ls:
    case Nil{}: found
    case +l <> +t: Card.work.find.go(t, id, Bool.pick(String, Bool.and(String.is_empty(found), String.eq(M.Bg.field(l, 0n), id)), l, found))

def Card.work.find(+ls: List<&2, String>, +id: String) -> String:
  Card.work.find.go(ls, id, "")

def Card.work(+st: M.State, +r: R.Ref, +line: String) -> Maybe<&2, Card>:
  Card.accept(Bool.not(String.is_empty(line)), Card.make(st, r, "terminal", M.Bg.field(line, 3n), "", M.Bg.field(line, 1n)))

def Card.mailbox(+st: M.State, +r: R.Ref, +k: String, +b: BT.Bot) -> Maybe<&2, Card>:
  # mail lead: replace placeholders with mail model metadata here.
  Card.accept(BT.Bot.live(b), Card.make(st, r, "file-text", BT.Bot.name(b) ++ " " ++ k, "", ""))

def Card.part(+s: String, +sep: String) -> String:
  Bool.pick(String, String.is_empty(s), "", sep ++ s)

def Card.line(+c: Card) -> String:
  match c:
    case Card{_, _, _, +t, +s, +state, +m}: t ++ Card.part(s, " · ") ++ Card.part(state, " · ") ++ Card.part(m, " @")

# Length-free exact encoding: pct never contains the separator /.
def Card.sig(+c: Card) -> String:
  match c:
    case Card{+u, +k, +i, +t, +s, +state, +m}: String.join([R.Ref.pct(u), R.Ref.pct(k), R.Ref.pct(i), R.Ref.pct(t), R.Ref.pct(s), R.Ref.pct(state), R.Ref.pct(m)], "/")

# Cache entries retain their dependency keys. Reverse buckets retain
# URI generations: invalidation advances one generation, never walks or
# frees its URI fanout. Missing entries are indexed too.
type CardEntry is Data:
  CardEntry{filled: Bool, value: Maybe<&2, Card>, deps: List<&2, String>}

type Cards is Data:
  Cards{map: Map<&2, CardEntry>, reverse: Map<&2, Map<&2, Nat>>}

type CardGot is Data:
  CardGot{cards: Cards, value: Maybe<&2, Card>}

def Cards.empty() -> Cards:
  Cards{Map.new(&2, CardEntry), Map.new(&2, Map<&2, Nat>)}

def Cards.peek.valid(+ok: Bool, +v: Maybe<&2, Card>) -> Maybe<&2, Card>:
  match ok:
    case True{}: v
    case False{}: None{}

def Cards.setting(+key: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.starts_with(key, "board."), [String.drop(key, 6n)], Bool.pick(List<&2, String>, String.starts_with(key, "watch."), [String.drop(key, 6n)], Nil{}))

# Same event decisions as client Touch.of; precise ids replace its global
# '*' cases. Kept here to avoid a refview -> client -> refview cycle.
def Cards.touched(+ch: M.Change) -> List<&2, String>:
  match ch:
    case M.ThreadCreated{+id, _, _, _, _, _, _, _, _}: [id]
    case M.ThreadRenamed{+id, _, _}: [id]
    case M.ThreadSettled{+id, _, _}: [id]
    case M.ThreadUnsettled{+id, _}: [id]
    case M.ThreadArchived{+id, _, _}: [id]
    case M.ThreadPinned{+id, _, _}: [id]
    case M.ThreadMerged{+id, _, _}: [id]
    case M.MessagePosted{+id, _, _, _, _}: [id]
    case M.MessageFrom{+id, _, _, _, _, _}: [id]
    case M.ThreadDeleted{+id, _}: [id]
    case M.TurnChanged{+id, _, _}: [id]
    case M.PendingChanged{+id, _, _}: [id]
    case M.ProjectCreated{+id, _, _, _}: [id]
    case M.ProjectRemoved{+id, _}: [id]
    case M.BotSet{+id, _, _, _, _, _}: [id]
    case M.BotDeleted{+id, _}: [id]
    case M.RoomSet{+id, _, _, _}: [id]
    case M.RoomDeleted{+id, _}: [id]
    case M.RoutineSet{+id, _, _, _, _, _, _}: [id]
    case M.RoutineDeleted{+id, _}: [id]
    case M.WorkSet{+id, _, _, _}: [id]
    case M.SettingSet{+key, _, _}: Cards.setting(key)
    case _: Nil{}

type Trail is Data:
  Trail{back: List<&2, String>, cur: String, fwd: List<&2, String>}

def Trail.go.k(+same: Bool, +t: Trail, +uri: String) -> Trail:
  match same t:
    case True{} _: t
    case False{} Trail{+bs, +c, _}: Trail{List.take(&2, String, c <> bs, 50n), uri, Nil{}}

def Trail.go(+t: Trail, +uri: String) -> Trail:
  match t:
    case Trail{_, +c, _}: Trail.go.k(String.eq(c, uri), t, uri)

def Trail.back(+t: Trail) -> Trail:
  match t:
    case Trail{Nil{}, +c, +fs}: Trail{Nil{}, c, fs}
    case Trail{+b <> +bs, +c, +fs}: Trail{bs, b, c <> fs}

def Trail.fwd(+t: Trail) -> Trail:
  match t:
    case Trail{+bs, +c, Nil{}}: Trail{bs, c, Nil{}}
    case Trail{+bs, +c, +f <> +fs}: Trail{c <> bs, f, fs}

def Trail.cur(+t: Trail) -> String:
  match t:
    case Trail{_, +c, _}: c

def Trail.can_back(+t: Trail) -> Bool:
  match t:
    case Trail{+bs, _, _}: Bool.not(List.is_empty(&2, String, bs))

def Trail.can_fwd(+t: Trail) -> Bool:
  match t:
    case Trail{_, _, +fs}: Bool.not(List.is_empty(&2, String, fs))

def Card.cut.tail(+c: R.RefCut) -> String:
  match c:
    case R.RefCut{_, +t}: t

# A work id ends in -sequence; its thread can itself contain dashes/~.
def Card.work.thread.local(+id: String) -> String:
  String.reverse(Card.cut.tail(R.Ref.cut(String.reverse(String.drop(id, 2n)), '-', "")))

def Card.work.thread(+id: String) -> String:
  Bool.pick(String, FR.Fr.is(id), FR.Fr.link(id) ++ "~", "") ++ Card.work.thread.local(FR.Fr.local(id))

def Cards.deps.object(+st: M.State, +r: R.Ref) -> List<&2, String>:
  match r:
    case R.Ref{"thread", +id <> +tail, _}: [id, Card.thread.project(M.State.thread.get(st, id))]
    case R.Ref{"bot", +id <> +tail, _}: [id, BT.Bot.thread(BT.World.bot(M.State.bots(st), id))]
    case R.Ref{"routine", +id <> +tail, _}: [id, BT.Routine.bot(BT.World.routine(M.State.bots(st), id))]
    case R.Ref{"work", +id <> +tail, _}: [id, Card.work.thread(id)]
    case _: [Card.id(r)]

def Cards.result.cards(+g: CardGot) -> Cards:
  match g:
    case CardGot{+cs, _}: cs

def Cards.result.value(+g: CardGot) -> Maybe<&2, Card>:
  match g:
    case CardGot{_, +v}: v

def Card.shelf(+s: M.Shelf) -> Bool:
  match s:
    case M.Settled{}: True{}
    case _: False{}

def Card.status(+t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, +tu, +pending, +ar, +ov, _, _, _, _, _, _, _, _, _, _, _, _}:
      Bool.pick(String, ar, "archived", Bool.pick(String, Nat.is_gt(pending, 0n), "waiting", Bool.pick(String, M.Work.turn.runs(tu), "working", Bool.pick(String, Card.shelf(M.Shelf.of(t)), "settled", "idle"))))

def Card.last(r: R.Ref) -> String:
  match r:
    case R.Ref{_, xs, _}: Maybe.default(&2, String, List.last(&2, String, xs), "")

# Machine labels can change separately from an object's own events.
# A consumer applying Fr.mark invalidates the link id as well.
def Cards.deps(+st: M.State, +r: R.Ref) -> List<&2, String>:
  List.append(&2, String, Cards.deps.object(st, r), Bool.pick(List<&2, String>, FR.Fr.is(Card.id(r)), [FR.Fr.link(Card.id(r))], Nil{}))

def Card.project.got(r: Map<&2, M.Project> & M.Project) -> M.Project:
  (_, p) = r
  p

def Card.project(+st: M.State, +id: String) -> M.Project:
  match st:
    case M.State{+ps, _, _, _, _, _, _, _, _, _, _}:
      Card.project.got(Map.get(M.Project, M.Project.blank(id), ps, id))

def Card.project.title(+st: M.State, +id: String) -> String:
  Card.project.title.k(Card.project(st, id))

def Card.pr.sub(repo: Bool, st: M.State, id: String) -> String:
  match repo:
    case True{}: id
    case False{}: Card.project.title(st, id)

def Card.project.root(+st: M.State, +id: String) -> String:
  Card.project.root.k(Card.project(st, id))

def Card.project.live(+st: M.State, +id: String) -> Bool:
  Card.project.live.k(Card.project(st, id))

def Card.thread(+st: M.State, +r: R.Ref, +t: M.Thread) -> Maybe<&2, Card>:
  match r:
    case R.Ref{+k, +xs, +f}:
      Card.accept(Card.thread.live(t), Card.make(st, R.Ref{k, xs, f}, "message-square", Card.thread.title(t), Bool.pick(String, String.is_empty(f), "", "message · ") ++ Card.project.title(st, Card.thread.project(t)), Card.status(t)))

def Card.machine.has(r: Map<&2, M.Nav> & Bool) -> Bool:
  (_, found) = r
  found

def Card.machine.known(st: M.State, id: String) -> Bool:
  Card.machine.has(Map.has(&2, M.Nav, M.State.navs(st), FR.Fr.nav(id)))

def Card.of(+st: M.State, +r: R.Ref) -> Maybe<&2, Card>:
  match r:
    case R.Ref{"thread", +id <> +tail, +f}: Card.thread(st, R.Ref{"thread", [id], f}, M.State.thread.get(st, id))
    case R.Ref{"bot", +id <> +tail, _}: Card.bot(st, r, BT.World.bot(M.State.bots(st), id))
    case R.Ref{"room", +id <> +tail, _}: Card.room(st, r, BT.World.room(M.State.bots(st), id))
    case R.Ref{"project", +id <> +tail, _}: Card.accept(Card.project.live(st, id), Card.make(st, r, "folder", Card.project.title(st, id), "project", ""))
    case R.Ref{"routine", +id <> +tail, _}: Card.routine(st, r, BT.World.routine(M.State.bots(st), id))
    case R.Ref{"task", +id <> +tail, _}: Card.task.json(st, r, J.Json.parse(M.Setting.get(st, "board." ++ id, "")))
    case R.Ref{"work", +id <> +tail, _}: Card.work(st, r, Card.work.find(M.Work.watch(st, Card.work.thread(id)), FR.Fr.local(id)))
    case R.Ref{"pr", +repo <> +n <> +tail, _}: Some{Card.make(st, r, "git-fork", "PR #" ++ n, Card.pr.sub(String.contains(repo, "/"), st, repo), "")}
    case R.Ref{"file", +id <> +path, _}: Card.accept(Card.project.live(st, id), Card.make(st, r, "file", FM.Fm.last(String.join(path, "/")), Card.project.title(st, id), FM.Fm.kind(String.join(path, "/"))))
    case R.Ref{"machine", +id <> +tail, _}: Card.accept(Card.machine.known(st, id), Card.make(st, r, "box", FR.Fr.name(st, id ++ "~x"), "", ""))
    case R.Ref{"person", _, _}: Some{Card.make(st, r, "circle-check", "You", "", "")}
    case R.Ref{"mailroom", +id <> +tail, _}: Card.mailbox(st, r, "mailroom", BT.World.bot(M.State.bots(st), id))
    case R.Ref{"charter", +id <> +tail, _}: Card.mailbox(st, r, "charter", BT.World.bot(M.State.bots(st), id))
    case R.Ref{+k, _, _}: Card.accept(R.Ref.has(["conv", "mail", "decision"], k), Card.make(st, r, "file-text", k, Card.last(r), ""))

def Card.bare.k(+ok: Bool, +st: M.State, +r: R.Ref) -> Maybe<&2, Card>:
  match ok:
    case True{}: Card.of(st, r)
    case False{}: None{}

def Card.bare(+st: M.State, +r: R.Ref) -> Maybe<&2, Card>:
  match r:
    case R.Ref{+k, _, _}: Card.bare.k(R.Ref.has(["thread", "bot", "project", "room", "routine", "task"], k), st, r)

def Cards.bucket.got(r: Map<&2, Map<&2, Nat>> & Map<&2, Nat>) -> Map<&2, Nat>:
  (_, b) = r
  b

def Cards.bucket(+rev: Map<&2, Map<&2, Nat>>, +id: String) -> Map<&2, Nat>:
  Cards.bucket.got(Map.get(Map<&2, Nat>, Map.new(&2, Nat), rev, id))

def Cards.number(r: Map<&2, Nat> & Nat) -> Nat:
  (_, n) = r
  n

def Cards.entry(r: Map<&2, CardEntry> & CardEntry) -> CardEntry:
  (_, e) = r
  e

# Empty URI is reserved inside a reverse bucket for its generation.
# Every indexed URI is nonempty; generation zero means absent.
# Map itself contains Nat: host exports need the existing identity-
# unmarshal handling, or the cache must stay inside Bend.
def Cards.epoch(bucket: Map<&2, Nat>) -> Nat:
  Cards.number(Map.get(Nat, 0n, bucket, ""))

def Cards.register(bucket: Map<&2, Nat>, uri: String, +gen: Nat) -> Map<&2, Nat>:
  +g = Bool.pick(Nat, Nat.is_eq(gen, 0n), 1n, gen)
  Map.set(&2, Nat, Map.set(&2, Nat, bucket, "", g), uri, g)

def Cards.index(+ids: List<&2, String>, +rev: Map<&2, Map<&2, Nat>>, +uri: String) -> Map<&2, Map<&2, Nat>>:
  match ids:
    case Nil{}: rev
    case +id <> +t:
      +bucket = Cards.bucket(rev, id)
      Cards.index(t, Map.set(&2, Map<&2, Nat>, rev, id, Cards.register(bucket, uri, Cards.epoch(bucket))), uri)

def Cards.fill.parsed(+m: Maybe<&2, R.Ref>, +st: M.State, +map: Map<&2, CardEntry>, +rev: Map<&2, Map<&2, Nat>>, +uri: String) -> CardGot:
  match m:
    case None{}: CardGot{Cards{Map.set(&2, CardEntry, map, uri, CardEntry{True{}, None{}, Nil{}}), rev}, None{}}
    case Some{+r}:
      +v = Card.of(st, r)
      +deps = Cards.deps(st, r)
      CardGot{Cards{Map.set(&2, CardEntry, map, uri, CardEntry{True{}, v, deps}), Cards.index(deps, rev, uri)}, v}

def Cards.get.hit(+hit: Bool, +e: CardEntry, +st: M.State, +map: Map<&2, CardEntry>, +rev: Map<&2, Map<&2, Nat>>, +uri: String) -> CardGot:
  match hit e:
    case True{} CardEntry{_, +v, _}: CardGot{Cards{map, rev}, v}
    case False{} _: Cards.fill.parsed(R.Ref.parse(uri), st, map, rev, uri)

def Cards.valid.bucket(+bucket: Map<&2, Nat>, uri: String) -> Bool:
  +g = Cards.epoch(bucket)
  Bool.and(Nat.is_gt(g, 0n), Nat.is_eq(Cards.number(Map.get(Nat, 0n, bucket, uri)), g))

def Cards.valid(+ids: List<&2, String>, +rev: Map<&2, Map<&2, Nat>>, +uri: String) -> Bool:
  match ids:
    case Nil{}: True{}
    case +id <> +t: Bool.and(Cards.valid.bucket(Cards.bucket(rev, id), uri), Cards.valid(t, rev, uri))

def Cards.peek.entry(+e: CardEntry, +rev: Map<&2, Map<&2, Nat>>, +uri: String) -> Maybe<&2, Card>:
  match e:
    case CardEntry{+filled, +v, +deps}: Cards.peek.valid(Bool.and(filled, Cards.valid(deps, rev, uri)), v)

def Cards.get.entry(+e: CardEntry, +st: M.State, +map: Map<&2, CardEntry>, +rev: Map<&2, Map<&2, Nat>>, +uri: String) -> CardGot:
  match e:
    case CardEntry{+filled, _, +deps}: Cards.get.hit(Bool.and(filled, Cards.valid(deps, rev, uri)), e, st, map, rev, uri)

def Cards.peek(+cs: Cards, +uri: String) -> Maybe<&2, Card>:
  match cs:
    case Cards{+map, +rev}:
      +e = Cards.entry(Map.get(CardEntry, CardEntry{False{}, None{}, Nil{}}, map, uri))
      Cards.peek.entry(e, rev, uri)

def Cards.get(+cs: Cards, +st: M.State, +uri: String) -> CardGot:
  match cs:
    case Cards{+map, +rev}:
      +e = Cards.entry(Map.get(CardEntry, CardEntry{False{}, None{}, Nil{}}, map, uri))
      Cards.get.entry(e, st, map, rev, uri)

def Cards.forget.epoch(gen: Nat, bucket: Map<&2, Nat>, rev: Map<&2, Map<&2, Nat>>, id: String) -> Map<&2, Map<&2, Nat>>:
  match gen:
    case 0n: rev
    case 1n+k: Map.set(&2, Map<&2, Nat>, rev, id, Map.set(&2, Nat, bucket, "", Nat.add(2n, k)))

def Cards.forget(+rev: Map<&2, Map<&2, Nat>>, +id: String) -> Map<&2, Map<&2, Nat>>:
  +bucket = Cards.bucket(rev, id)
  Cards.forget.epoch(Cards.epoch(bucket), bucket, rev, id)

def Cards.drop.go(+ids: List<&2, String>, +cs: Cards) -> Cards:
  match ids cs:
    case Nil{} _: cs
    case +id <> +t Cards{+map, +rev}: Cards.drop.go(t, Cards{map, Cards.forget(rev, id)})

def Cards.drop(+cs: Cards, +ids: List<&2, String>) -> Cards:
  Cards.drop.go(ids, cs)
