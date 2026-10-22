import Base
import ./thin-mail.bend as TM
import ./thin-view.bend as V
import ../core/client.bend as U
import ../core/model.bend as M
import ../core/json.bend as J
import ../core/presentation.bend as P
import ../core/thin-wire.bend as W
import ../core/thin.bend as T
import ../core/thin-live.bend as TL
import ../web/app.bend as A
import ../web/mail-actions.bend as MA
import ../mobile/hubs.bend as H
import ./thin-splice.bend as S
import ../core/i18n.bend as I

# Private, connection-owned UI state. It never enters a client snapshot or
# persisted event. Keep it behind a List in the connection context so the
# native ABI does not flatten a second full Ui into every Client argument.
# memo: the last full render's reply and what it was made of (at most
# one; thin-splice.bend writes only the live row into it until anything
# else changes or a second passes: Memo.splice)
type Memo is Data:
  Memo{key: String, clock: Nat, partial: Bool, result: J.Json}

type Actor is Data:
  Actor{ui: U.Ui, view: String, target: J.Json, pending: Map<&2, String>, reads: List<&2, J.Json>, dirty: Bool, memo: List<&2, Memo>}

type Rendered is Data:
  Rendered{actor: Actor, result: J.Json}

def Actor.ui( +a: Actor) -> U.Ui:
  match a:
    case Actor{u, _, _, _, _, _, _}: u

def Actor.scope(+target: J.Json) -> String:
  J.Json.show(V.Target.json(J.Json.get_str(target, "kind", ""), J.Json.get_str(target, "id", "")))

def Actor.pref.key(+key: String) -> Bool:
  Bool.or(String.starts_with(key, "fold:"), String.starts_with(key, "subs:"))

def Actor.pref.put( +keep: Bool, +prefs: Map<&2, String>, +key: String, +value: String) -> Map<&2, String>:
  match keep:
    case True{}: Map.set(&2, String, prefs, key, value)
    case False{}: prefs

def Actor.prefs( +xs: List<&2, Sigma<&2, &2, String, _ => String>>, +prefs: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}: prefs
    case (+key, value) <> tail: Actor.prefs(tail, Actor.pref.put(Actor.pref.key(key), prefs, key, value))

def Actor.draft.value( +value: Maybe<&2, J.Json>, +thread: String) -> J.Json:
  match value:
    case Some{J.Str{+text}}: V.Obj.put(J.Json.obj(Nil{}), thread, J.Str{text})
    case _: J.Json.obj(Nil{})

def Actor.draft.safe( +valid: Bool, +thread: String, +drafts: J.Json) -> J.Json:
  match valid:
    case False{}: J.Json.obj(Nil{})
    case True{}: Actor.draft.value(J.Json.get(drafts, thread), thread)

def Actor.local.key(+key: String) -> Bool:
  Nat.is_lt(U.Keys.pos(["@lang", "@lang.os", "@lang.tag", "@lang.preview", "@local.view", "@local.usage"], key), 6n)

def Actor.local.bounded(valid: Bool, key: String, text: String, out: J.Json) -> J.Json:
  match valid:
    case True{}: V.Obj.put(out, key, J.Str{text})
    case False{}: out

def Actor.local.field(valid: Bool, +key: String, value: J.Json, out: J.Json) -> J.Json:
  match valid value:
    case True{} J.Str{+text}: Actor.local.bounded(W.Wire.within(text, 128n), key, text, out)
    case _ _: out

def Actor.local.input(keys: List<&2, String>, +drafts: J.Json, out: J.Json) -> J.Json:
  match keys:
    case Nil{}: out
    case +key <> tail: Actor.local.input(tail, drafts, Actor.local.field(True{}, key, J.Json.get_obj(drafts, key), out))

def Actor.local.copy(xs: List<&2, Sigma<&2, &2, String, _ => String>>, dr: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}: dr
    case (+key, value) <> tail: Actor.local.copy(tail, Actor.pref.put(Actor.local.key(key), dr, key, value))

# Only ordinary current-thread draft, bounded device language/display choices
# and fold/subscription preferences may be
# supplied by the renderer. Typed mail stores, request markers, form fields
# and outbox are retained from this actor, never reconstructed from the wire.
def Actor.safe_input(+st: M.State, +p: J.Json) -> J.Json:
  +thread = V.Target.thread(st, J.Json.get_obj(p, "target"))
  +session = J.Json.get_obj(p, "session")
  +drafts = Actor.local.input(["@lang", "@lang.os", "@lang.tag", "@lang.preview", "@local.view", "@local.usage"], J.Json.get_obj(session, "drafts"), Actor.draft.safe(Bool.and(Bool.not(String.is_empty(thread)), Bool.not(String.starts_with(thread, "@"))), thread, J.Json.get_obj(session, "drafts")))
  V.Obj.put(p, "session", V.Obj.put(V.Obj.put(V.Obj.put(session, "drafts", drafts), "outbox", J.Json.obj(Nil{})), "info",
    J.Json.obj(P.Session.strings(Map.to_list(&2, String, Actor.prefs(Map.to_list(&2, String, P.Session.read.strings(J.Json.get_obj(session, "info"), Map.new(&2, String))), Map.new(&2, String)))))))

def Actor.scope.keep(+key: String) -> Bool:
  Bool.not(Bool.or(String.starts_with(key, "@f."), String.starts_with(key, "@mr.")))

def Actor.scope.clear( +xs: List<&2, Sigma<&2, &2, String, _ => String>>, +dr: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}: dr
    case (+key, value) <> tail: Actor.scope.clear(tail, Actor.pref.put(Actor.scope.keep(key), dr, key, value))

def Actor.scope.drafts( +same: Bool, +dr: Map<&2, String>) -> Map<&2, String>:
  match same:
    case True{}: dr
    case False{}: Actor.scope.clear(Map.to_list(&2, String, dr), Map.new(&2, String))

def Actor.selector(+dr: Map<&2, String>, +clean: Map<&2, String>) -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Map.set(&2, String, Actor.local.copy(Map.to_list(&2, String, clean), dr),
    "@bot", U.Str.get(clean, "@bot")), "@room", U.Str.get(clean, "@room")), "@remote", U.Str.get(clean, "@remote")), "@btab", U.Str.get(clean, "@btab")), "@sync", "1")

def Actor.draft.merge(+clean: Map<&2, String>, +dr: Map<&2, String>, +thread: String, +present: Bool) -> Map<&2, String>:
  match present:
    case True{}: Map.set(&2, String, Actor.selector(dr, clean), thread, U.Str.get(clean, thread))
    case False{}: Actor.selector(dr, clean)

def Actor.merge( +fresh: U.Ui, +old: U.Ui, +same: Bool, +thread: String, +present: Bool) -> U.Ui:
  match fresh old:
    case U.Ui{st, sel, clean, live, open, on, now, adding, panel, version, +rid2, want, _, _, nf2, _, _} U.Ui{_, _, dr, _, _, _, _, _, _, _, +rid, _, error, note, nf, outbox, stores}:
      U.Ui{st, sel, Actor.draft.merge(clean, Actor.scope.drafts(same, dr), thread, present), live, open, on, now, adding, panel, version,
        M.Time.max(rid, rid2), want, error, note, Actor.prefs(Map.to_list(&2, String, nf2), nf), outbox, stores}

def Actor.make(+u: U.Ui, +view: String, +target: J.Json) -> Actor:
  Actor{u, view, target, Map.new(&2, String), Nil{}, False{}, Nil{}}

def Actor.refresh( +xs: List<&2, Actor>, +web: Bool, +st: M.State, +info: J.Json, +live: J.Json, +now: Nat, +p: J.Json, +view: String) -> Actor:
  match xs:
    case Nil{}: Actor.make(V.Session.current(web, st, info, live, now, Actor.safe_input(st, p)), view, J.Json.get_obj(p, "target"))
    case Actor{old, oldView, +target, pending, reads, dirty, _} <> _:
      +input = Actor.safe_input(st, p)
      +thread = V.Target.thread(st, J.Json.get_obj(p, "target"))
      +fresh = V.Session.current(web, st, info, live, now, input)
      +same = Bool.and(String.eq(oldView, view), W.Target.same(target, J.Json.get_obj(p, "target")))
      Actor{V.Session.renderer(web, U.Ui.info_merge(Actor.merge(fresh, old, same, thread, Maybe.is_some(&2, J.Json, J.Json.get(J.Json.get_obj(J.Json.get_obj(input, "session"), "drafts"), thread))), info), p),
        view, J.Json.get_obj(p, "target"), Bool.pick(Map<&2, String>, same, pending, Map.new(&2, String)), Bool.pick(List<&2, J.Json>, same, reads, Nil{}), dirty, Nil{}}

# The hub's canonical projection is shared; each actor keeps its own pages,
# drafts, request slots and exact scope. Compare only the selected body's
# newest identity across authoritative states, never its text or full log.
def Actor.mail_cache(+a: Actor, previous: List<&2, Actor>, cache: TM.Cache) -> Actor:
  match a previous:
    case Actor{u, view, target, pending, reads, dirty, memo} Nil{}:
      Actor{TM.Cache.install(cache, u), view, target, pending, reads, dirty, memo}
    case Actor{u, view, target, pending, reads, dirty, memo} old <> _:
      Actor{TM.Cache.refresh(cache, u, Actor.ui(old)), view, target, pending, reads, dirty, memo}

def Actor.rpc(+command: J.Json) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(J.Json.get_str(command, "text", "")), J.Null{})

def Actor.read_only(+command: J.Json) -> Bool:
  +request = Actor.rpc(command)
  Bool.and(String.eq(J.Json.get_str(command, "type", ""), "send"), Bool.or(String.eq(J.Json.get_str(request, "m", ""), "mail.list"), Bool.or(String.eq(J.Json.get_str(request, "m", ""), "mail.read"), String.eq(J.Json.get_str(request, "m", ""), "mail.find"))))

def Actor.pending_put( +send: Bool, +command: J.Json, +pending: Map<&2, String>, +scope: String) -> Map<&2, String>:
  match send:
    case False{}: pending
    case True{}: Map.set(&2, String, pending, J.Json.as_raw(J.Json.get(Actor.rpc(command), "id"), ""), scope)

def Actor.note( +cs: List<&2, J.Json>, +pending: Map<&2, String>, +scope: String) -> Map<&2, String>:
  match cs:
    case Nil{}: pending
    case +c <> tail: Actor.note(tail, Actor.pending_put(String.eq(J.Json.get_str(c, "type", ""), "send"), c, pending, scope), scope)

def Actor.read_add( +read: Bool, +c: J.Json, +tail: List<&2, J.Json>) -> List<&2, J.Json>:
  match read:
    case False{}: tail
    case True{}: c <> tail

def Actor.reads( +cs: List<&2, J.Json>, +acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match cs:
    case Nil{}: List.reverse(&2, J.Json, acc)
    case +c <> tail: Actor.reads(tail, Actor.read_add(Actor.read_only(c), c, acc))

def Actor.accept( +a: Actor, +r: V.ActResult, +queue: Bool) -> Actor:
  match a r:
    case Actor{_, view, +target, pending, reads, dirty, memo} V.ActResult{u, +cs}:
      Actor{u, view, target, Actor.note(cs, pending, Actor.scope(target)),
        List.append(&2, J.Json, reads, Bool.pick(List<&2, J.Json>, queue, Actor.reads(cs, Nil{}), Nil{})), dirty, memo}

def Actor.mail_ready( +a: Actor) -> Actor:
  Actor.accept(a, V.ActResult.web(MA.MA.after(U.Mailv.sync(U.Act{Actor.ui(a), Nil{}}))), True{})

def Actor.origin( +a: Actor, +origin: String) -> Actor:
  match a:
    case Actor{u, view, target, pending, reads, dirty, memo}: Actor{U.Ui.info_set(u, "origin", origin), view, target, pending, reads, dirty, memo}

def Actor.mail_context_if( +shown: Bool, +u: U.Ui) -> J.Json:
  match shown:
    case False{}: J.Null{}
    case True{}: J.Json.obj([J.KV{"kind", J.Str{"mail-draft"}}, J.KV{"owner", J.Str{U.Mailv.owner(u)}}, J.KV{"origin", J.Str{U.Ui.info(u, "origin")}},
      J.KV{"conv", J.Str{U.Ui.scratch(u, "@mr.conv")}}, J.KV{"recipient", J.Str{U.Ui.scratch(u, "@mr.to")}}])

def Actor.mail_context(+u: U.Ui) -> J.Json:
  Actor.mail_context_if(Bool.and(U.Mailv.shown(u), Bool.not(String.is_empty(U.Ui.scratch(u, "@mr.conv")))), u)

def Actor.render(+a: Actor, +web: Bool, +st: M.State, +index: T.Index, +p: J.Json, +epoch: String, +now: Nat, +rows: List<&2, String>, +stream: TL.Stream) -> Rendered:
  +ready = Actor.mail_ready(Actor.origin(a, epoch))
  +action = V.Action.run(web, Actor.ui(ready), p, now)
  +updated = Actor.accept(ready, action, False{})
  Rendered{updated, V.Obj.put(V.Render.acted_live(action, web, st, index, p, epoch, now, rows, stream), "mailDraftContext", Actor.mail_context(Actor.ui(updated)))}

def Actor.pending_scope( +pending: Map<&2, String>, +id: String) -> String:
  U.Str.get(pending, id)

def Actor.receive( +web: Bool, +u: U.Ui, +key: String, +now: Nat, +o: J.Json) -> V.ActResult:
  match web:
    case True{}: V.ActResult.web(A.recv(u, o))
    case False{}: V.ActResult.mobile(H.Hubs.recv(H.Hubs{"", now, key, [H.Hub{key, u}]}, key, o), key)

def Actor.mark_dirty( +a: Actor) -> Actor:
  match a:
    case Actor{u, view, target, pending, reads, _, _}: Actor{u, view, target, pending, reads, True{}, Nil{}}

def Actor.now( +u: U.Ui) -> Nat:
  match u:
    case U.Ui{_, _, _, _, _, _, now, _, _, _, _, _, _, _, _, _, _}: now

def Actor.reply_if( +owned: Bool, +a: Actor, +web: Bool, +key: String, +o: J.Json) -> Actor:
  match owned:
    case False{}: a
    case True{}: Actor.mark_dirty(Actor.accept(a, Actor.receive(web, Actor.ui(a), key, Actor.now(Actor.ui(a)), o), True{}))

def Actor.reply(+a: Actor, +web: Bool, +key: String, +o: J.Json) -> Actor:
  match a:
    case Actor{u, view, +target, +pending, reads, dirty, memo}:
      +id = J.Json.as_raw(J.Json.get(o, "id"), "")
      Actor.reply_if(Bool.and(String.eq(J.Json.get_str(o, "t", ""), "reply"), String.eq(Actor.pending_scope(pending, id), Actor.scope(target))),
        Actor{u, view, target, Map.del(&2, String, pending, id), reads, dirty, memo}, web, key, o)

def Actor.clean( +a: Actor) -> Actor:
  match a:
    case Actor{u, view, target, pending, _, _, memo}: Actor{u, view, target, pending, Nil{}, False{}, memo}

def Actor.queued( +a: Actor) -> List<&2, J.Json>:
  match a:
    case Actor{_, _, _, _, reads, _, _}: reads

def Actor.dirty( +a: Actor) -> Bool:
  match a:
    case Actor{_, _, _, _, _, dirty, _}: dirty

def Actor.capacity( +a: Actor) -> Bool:
  match a:
    case Actor{_, _, _, pending, reads, _, _}: Bool.and(Nat.is_le(List.length(&2, String, Map.keys(&2, String, pending)), 128n), Nat.is_le(List.length(&2, J.Json, reads), 16n))

# A queued read is useful only to the exact surviving connection incarnation
# and view that produced it. Equal RPC ids on another socket grant nothing.
def Job.valid(+actualClient: U32, +jobClient: U32, +actualGen: U32, +jobGen: U32, +actualView: String, +jobView: String, +actualTarget: J.Json, +jobTarget: J.Json, +command: J.Json) -> Bool:
  Bool.and(U32.is_eq(actualClient, jobClient), Bool.and(U32.is_eq(actualGen, jobGen), Bool.and(String.eq(actualView, jobView),
    Bool.and(W.Target.same(actualTarget, jobTarget), Actor.read_only(command)))))

def Control.fresh(+id: Nat, +last: Nat) -> Bool:
  Bool.and(Nat.is_gt(id, Nat.mul(65536n, 2147483648n)), Bool.and(Nat.is_le(id, P.Session.nat.max()), Nat.is_gt(id, last)))

# The render memo (thin-splice.bend)
# ----------------------------------

def Actor.memos(xs: List<&2, Actor>) -> List<&2, Memo>:
  match xs:
    case Actor{_, _, _, _, _, _, memo} <> _: memo
    case Nil{}: Nil{}

def Actor.loc(xs: List<&2, Actor>) -> I.Loc:
  match xs:
    case Actor{u, _, _, _, _, _, _} <> _: U.Lang.loc(u)
    case Nil{}: I.En{}

def Actor.remember(+a: Actor, +m: Memo) -> Actor:
  match a:
    case Actor{u, view, target, pending, reads, dirty, _}: Actor{u, view, target, pending, reads, dirty, [m]}

# what a render is made of but its live row: the read model's sequence, the
# hub's info, the request but its revisions, the page anchor, the view, the
# renderer and the thread (a reply arriving clears the memo: Actor.reply_if)
def Memo.request(+p: J.Json) -> J.Json:
  match p:
    case J.Obj{+fs}: J.Obj{V.Obj.drop(V.Obj.drop(fs, "requestRevision"), "viewRevision")}
    case _: p

def Memo.key(+seq: Nat, +info: String, +p: J.Json, +anchor: Nat, +view: String, +web: Bool, +thread: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"seq", J.Json.nat(seq)}, J.KV{"info", J.Str{info}}, J.KV{"p", Memo.request(p)}, J.KV{"anchor", J.Json.nat(anchor)},
    J.KV{"view", J.Str{view}}, J.KV{"web", J.Flag{web}}, J.KV{"thread", J.Str{thread}}]))

def Memo.reply.k(+ok: Bool, +res: J.Json, +web: Bool, +stream: TL.Stream, +thread: String, +epoch: String, +target: J.Json, +lc: I.Loc) -> Maybe<&2, J.Json>:
  match ok:
    case True{}: S.Splice.reply(web, res, stream, thread, epoch, target, lc)
    case False{}: None{}

# the memo's reply with only the live row and window new, when nothing else
# can have changed: the same key, at most a second old (clock text), a live
# row now with the same chunk marker as then; None: render in full
def Memo.reply(ms: List<&2, Memo>, +update: Bool, +key: String, +clock: Nat, +web: Bool, +stream: TL.Stream, +thread: String, +epoch: String, +target: J.Json, +lc: I.Loc) -> Maybe<&2, J.Json>:
  match ms:
    case Nil{}: None{}
    case Memo{+k, +at, +partial, +res} <> _:
      Memo.reply.k(Bool.and(update, Bool.and(String.eq(k, key), Bool.and(Nat.is_ge(clock, at), Bool.and(Nat.is_lt(Nat.sub(clock, at), 1000n),
        Bool.and(Nat.is_gt(TL.Stream.size(stream), 0n), Bool.or(Bool.and(partial, S.Splice.partial(stream)), Bool.and(Bool.not(partial), Bool.not(S.Splice.partial(stream))))))))), res, web, stream, thread, epoch, target, lc)

# a full render remembered for the next ones (clock: ms)
def Rendered.remember(r: Rendered, +key: String, +clock: Nat, +stream: TL.Stream) -> Rendered:
  match r:
    case Rendered{+a, +res}: Rendered{Actor.remember(a, Memo{key, clock, S.Splice.partial(stream), res}), res}
