import Base
import ../../src/core/json.bend as J
import ../../src/core/client.bend as U
import ../../src/mobile/hubs.bend as H
import ../../src/mobile/session.bend as MS
import ../../src/core/native-sidebar.bend as NS
import ../../src/mobile/thin.bend as MT
import ../../src/mobile/folded-actions.bend as FA
import ../../src/core/model.bend as M
import ../../src/core/project-home.bend as PH
import ../../src/core/bot.bend as B
import ../../src/core/thin.bend as T
import ../../src/core/thinq.bend as Q

# bp-policy: the slim backend's policy process, in machine code. It folds the
# hub's wire frames incrementally into Backplane's own client state (the same
# U.Ui.recv every client runs) and draws the screen a native client shows
# (the same MS.Session call Render.mobile makes in src/server/thin-view.bend).
# Base IO only: one loopback TCP connection, one request per line, one reply
# per line. No custom effects, no JS.
#
#   F <frame json>      fold one wire frame (log, changes, delta, info, ...)   -> ok
#   V <request json>    view.open/update/action params; optional p.now (ms since epoch: native IO.now()
#                       is not the wall clock) and p.window (the host's window object, as Render.native_screen takes it)  -> view json
#   C <ms>              set the wall clock (ms since epoch; the model reads seconds) the folded state reads              -> ok
#   P                   ping                                                    -> pong
#   Q                   close
#
# A request is ASCII (the host escapes non-ASCII as \uXXXX); a reply is one
# line of JSON. The state never crosses the socket: frames in, one picture out.

# ---- a page ----------------------------------------------------------------
# V's optional p.page = {thread, before, below, above, rows: [[10 string cells]...]}: the rows Rust read out of its
# own index (the cells of policy.bend: seq ord id thread src kind role akind text at). The thread's timeline is drawn
# from the 32-row page only, the way Render.ui draws a view.page: the shared state minus its entries, plus the page.

def Pol.cell(+x: J.Json) -> String:
  match x:
    case J.Str{+t}: t
    case _: ""

def Pol.cells(+xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case +x <> +t: Pol.cell(x) <> Pol.cells(t)

def Pol.cellrows(+xs: List<&2, J.Json>) -> List<&2, List<&2, String>>:
  match xs:
    case Nil{}: Nil{}
    case +x <> +t: Pol.cells(J.Json.elems(x)) <> Pol.cellrows(t)

def Pol.ents(ids: List<&2, String>, +idx: T.Index, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match ids:
    case Nil{}: acc
    case id <> rest: Pol.ents(rest, idx, T.Row.preview(T.Index.row(idx, Q.Q.num(id))) <> acc)

def Pol.empty(+st: M.State) -> M.State:
  match st:
    case M.State{+ps, +ts, _, +n, +tk, +settings, +queued, +asks, _, +nav, +bw}:
      M.State{ps, ts, Nil{}, n, tk, settings, queued, asks, Map.new(&2, M.Device), nav,
        [M.Ext{M.Ext.world(M.Ext.of(bw)), Map.new(&2, List<&2, M.Entry>), M.Ext.mail(M.Ext.of(bw)), M.Ext.usage(M.Ext.of(bw))}]}

def Pol.add(+es: List<&2, M.Entry>, +st: M.State) -> M.State:
  match es:
    case Nil{}: st
    case +e <> +t: Pol.add(t, M.State.post(st, e))

def Pol.paged(+u: U.Ui, +th: String, +idx: T.Index, +before: Nat) -> U.Ui:
  U.Ui.adopt.raw(U.Ui.scratch_set(U.Ui.scratch_set(U.Ui.scratch_set(u, "@more." ++ th, "1"), "@away." ++ th, ""), "@pill", ""),
    Pol.add(Pol.ents(T.Index.page(idx, th, before, 32n), idx, Nil{}), Pol.empty(U.Ui.state(u))))

def Pol.page.k(+u: U.Ui, +th: String, +before: Nat, +below: Nat, +above: Nat, +rows: List<&2, List<&2, String>>) -> U.Ui:
  Pol.paged(u, th, T.Index.window.above(T.Index.window(Q.Q.rows(rows), th, below, T.Index.empty()), th, above), before)

def Pol.page(+u: U.Ui, +page: J.Json) -> U.Ui:
  Pol.page.k(u, J.Json.get_str(page, "thread", ""), J.Json.get_nat(page, "before", 0n), J.Json.get_nat(page, "below", 0n), J.Json.get_nat(page, "above", 0n), Pol.cellrows(J.Json.elems(J.Json.get_obj(page, "rows"))))

def Pol.pick(+has: Bool, +u: U.Ui, +page: J.Json) -> U.Ui:
  match has:
    case True{}: Pol.page(u, page)
    case False{}: u

# ---- the picture -----------------------------------------------------------

def Pol.target.k(+empty: Bool, +sel: String) -> J.Json:
  match empty:
    case True{}: J.Json.obj([J.KV{"kind", J.Str{"recent"}}, J.KV{"id", J.Str{""}}])
    case False{}: J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{sel}}])

def Pol.target(+session: J.Json) -> J.Json:
  +sel = J.Json.get_str(session, "sel", "")
  Pol.target.k(String.is_empty(sel), sel)

def Pol.same(+a: J.Json, +b: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(a, "kind", ""), J.Json.get_str(b, "kind", "")), String.eq(J.Json.get_str(a, "id", ""), J.Json.get_str(b, "id", "")))

def Pol.is_obj(+j: J.Json) -> Bool:
  match j:
    case J.Obj{_}: True{}
    case _: False{}

# Render.native_screen of thin-view.bend: partial rows, folded actions, thread.earlier = window.remaining
def Pol.earlier(+th: J.Json, +window: J.Json) -> J.Json:
  match th:
    case J.Null{}: J.Null{}
    case +t: NS.Obj.put(t, "earlier", J.Json.nat(J.Json.get_nat(window, "remaining", 0n)))

def Pol.native(+screen: J.Json, +window: J.Json) -> J.Json:
  +partial = FA.Folded.screen(MT.Partial.screen(screen, window))
  NS.Obj.put(partial, "thread", Pol.earlier(J.Json.get_obj(partial, "thread"), window))

# with the host's window the screen is the hub's native screen; without it, the raw screen
def Pol.screen(+has: Bool, +screen: J.Json, +window: J.Json) -> J.Json:
  match has:
    case True{}: Pol.native(screen, window)
    case False{}: screen

# result.overlay: the {key, name, j} object with j drawn the same way
def Pol.overlay(+has: Bool, +shown: J.Json, +window: J.Json) -> J.Json:
  NS.Obj.put(shown, "j", Pol.screen(has, J.Json.get_obj(shown, "j"), window))

def Pol.out.k(+same: Bool, +has: Bool, +r: J.Json, +next: J.Json, +window: J.Json) -> J.Json:
  match same:
    case True{}:
      J.Json.obj([J.KV{"remoteReady", J.Flag{True{}}}, J.KV{"shown", Pol.screen(has, J.Json.get_obj(r, "screen"), window)}, J.KV{"overlay", Pol.overlay(has, J.Json.get_obj(r, "shown"), window)}, J.KV{"session", J.Json.get_obj(r, "session")}, J.KV{"cmds", J.Json.get_obj(r, "cmds")}])
    case False{}:
      J.Json.obj([J.KV{"remoteReady", J.Flag{False{}}}, J.KV{"nextTarget", next}, J.KV{"session", J.Json.get_obj(r, "session")}, J.KV{"cmds", J.Json.get_obj(r, "cmds")}])

def Pol.out(+same: Bool, +r: J.Json, +next: J.Json, +window: J.Json) -> J.Json:
  Pol.out.k(same, Pol.is_obj(window), r, next, window)

def Pol.view.r(+r: J.Json, +want: J.Json, +window: J.Json) -> J.Json:
  +next = Pol.target(J.Json.get_obj(r, "session"))
  Pol.out(Pol.same(next, want), r, next, window)

# the session the client sent adopted onto the shared state, the action run,
# the screen drawn
# p.now is milliseconds since the epoch (the host's wall clock); the model's clock is seconds. Without it, the process's own.
def Pol.clock(+ms: Nat, +own: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(ms, 0n), own, Nat.div(ms, 1000n))

# Target.bind of thin-view.bend: the target's selection and the bot/room views (these set the scratch keys @bot, @bot.want)
def Pol.bind.kind(+kind: String, +u: U.Ui, +id: String) -> U.Ui:
  match kind:
    case "project": U.Bots.views(U.Bots.sel.set(u, PH.Home.owner(id)), "", "", "")
    case "thread": U.Bots.views(U.Bots.sel.set(u, id), "", "", "")
    case "bot": U.Bots.views(U.Bots.sel.set(u, U.Bots.chat.of(U.Ui.state(u), id, B.Bot.thread(B.World.bot(M.State.bots(U.Ui.state(u)), id)))), id, "", "")
    case "room": U.Bots.views(u, "", id, "")
    case _: U.Bots.views(U.Bots.sel.set(u, ""), "", "", "")

def Pol.bind(+u: U.Ui, +target: J.Json) -> U.Ui:
  Pol.bind.kind(J.Json.get_str(target, "kind", ""), u, J.Json.get_str(target, "id", ""))

# The sidebar's page memory (offsets, the context it holds for, the cursor key and budget) lives in four Ui info keys that Ns.prepare and the
# page action read and write. The host keeps the Ui per request and not per view, so it carries them: V takes `local` {key: text} and puts each
# non-empty one into the Ui before Ns.prepare; the reply's `local` is the same four keys read after the action.
def Pol.local.k1() -> String:
  "local.native.sidebar.pages"

def Pol.local.k2() -> String:
  "local.native.sidebar.context"

def Pol.local.k3() -> String:
  "local.native.sidebar.key"

def Pol.local.k4() -> String:
  "local.native.sidebar.budget"

def Pol.local.put.k(+v: String, +u: U.Ui, +k: String) -> U.Ui:
  Bool.pick(U.Ui, String.is_empty(v), u, U.Ui.info_set(u, k, v))

def Pol.local.put.one(+u: U.Ui, +loc: J.Json, +k: String) -> U.Ui:
  Pol.local.put.k(J.Json.get_str(loc, k, ""), u, k)

def Pol.local.put(+u: U.Ui, +loc: J.Json) -> U.Ui:
  Pol.local.put.one(Pol.local.put.one(Pol.local.put.one(Pol.local.put.one(u, loc, Pol.local.k1()), loc, Pol.local.k2()), loc, Pol.local.k3()), loc, Pol.local.k4())

def Pol.local.get(+u: U.Ui) -> J.Json:
  J.Json.obj([J.KV{Pol.local.k1(), J.Json.str(U.Ui.info(u, Pol.local.k1()))}, J.KV{Pol.local.k2(), J.Json.str(U.Ui.info(u, Pol.local.k2()))},
    J.KV{Pol.local.k3(), J.Json.str(U.Ui.info(u, Pol.local.k3()))}, J.KV{Pol.local.k4(), J.Json.str(U.Ui.info(u, Pol.local.k4()))}])

def Pol.local.of(+a: H.HAct, +key: String) -> J.Json:
  match a:
    case H.HAct{+hs, _, _}: Pol.local.get(H.Hubs.ui(hs, key))

def Pol.view.out(+r: J.Json, +ua: H.HAct, +key: String) -> J.Json:
  NS.Obj.put(r, "local", Pol.local.of(ua, key))

# The sidebar's page action is not a client action: the JS hub's actor answers it with Ns.accept on its Ui and no commands (thin-actor.bend
# Actor.native.action). The host masks the reply's session (Actor.native.page_result.page) from the request's own session.
def Pol.act(ok: Bool, isPage: Bool, +u: U.Ui, +key: String, +cid: String, +now: Nat, +action: String, +value: String) -> H.HAct:
  match ok:
    case True{}: MS.Session.act(NS.Ns.accept(u, value), key, cid, now, "", "")
    case False{}: MS.Session.act(u, key, cid, now, Bool.pick(String, isPage, "", action), value)

# a page action that is not offered or whose token does not hold is refused as a whole (Hub.thin.act: INVALID_REQUEST): nothing runs, and the
# reply says so (`refused`: true, beside `local`) for the host to answer with the error
def Pol.refuse(refused: Bool, +r: J.Json) -> J.Json:
  match refused:
    case True{}: NS.Obj.put(r, "refused", J.Flag{True{}})
    case False{}: r

def Pol.view(+u0: U.Ui, +now0: Nat, +p: J.Json) -> J.Json:
  +now = Pol.clock(J.Json.get_nat(p, "now", 0n), now0)
  +key = J.Json.get_str(p, "hub", "")
  +cid = J.Json.get_str(p, "cid", "")
  +u1 = U.Ui.scratch_set(U.Ui.online(U.Ui.set_cid(Pol.bind(MS.Session.adopt(U.Ui.tick(u0, now), J.Json.get_obj(p, "session")), J.Json.get_obj(p, "target")), cid), True{}), "@sync", "1")
  +u = NS.Ns.prepare(Pol.local.put(Pol.pick(Pol.is_obj(J.Json.get_obj(p, "page")), u1, J.Json.get_obj(p, "page")), J.Json.get_obj(p, "local")), False{}, p, J.Json.get_str(p, "epoch", ""), J.Json.get_str(p, "viewId", ""), J.Json.get_nat(p, "revision", 0n), J.Json.get_str(p, "nativeKey", ""))
  +isPage = String.eq(J.Json.get_str(p, "action", ""), "native-sidebar-page")
  +ok = Bool.and(Bool.and(isPage, NS.Ns.offered(False{}, p)), NS.Ns.valid(u, J.Json.get_str(p, "value", "")))
  +ua = Pol.act(ok, isPage, u, key, cid, now, J.Json.get_str(p, "action", ""), J.Json.get_str(p, "value", ""))
  Pol.view.out(Pol.refuse(Bool.and(isPage, Bool.not(ok)), Pol.view.r(MS.Session.result(ua, key), J.Json.get_obj(p, "target"), J.Json.get_obj(p, "window"))), ua, key)

# ---- the fold --------------------------------------------------------------

def Pol.fold.k(m: Maybe<&2, J.Json>, +u: U.Ui) -> U.Ui:
  match m:
    case Some{j}: U.Ui.recv(u, j)
    case None{}: u

def Pol.fold(+u: U.Ui, +text: String) -> U.Ui:
  Pol.fold.k(J.Json.parse(text), u)

# ---- one request -----------------------------------------------------------

def Pol.tail(+line: String) -> String:
  String.drop(line, 2n)

def Pol.op(+line: String) -> String:
  String.take(line, 1n)

type Step is Data:
  Step{u: U.Ui, reply: String, stop: Bool}

def Pol.step.k(+op: String, +u: U.Ui, +line: String, +now: Nat) -> Step:
  match op:
    case "F": Step{Pol.fold(u, Pol.tail(line)), "ok", False{}}
    case "V": Step{u, J.Json.show(Pol.view(u, now, Maybe.default(&2, J.Json, J.Json.parse(Pol.tail(line)), J.Null{}))), False{}}
    case "C": Step{U.Ui.tick(u, Pol.clock(Maybe.default(&2, Nat, Nat.read(Pol.tail(line)), 0n), now)), "ok", False{}}
    case "P": Step{u, "pong", False{}}
    case "Q": Step{u, "bye", True{}}
    case _: Step{u, "err", False{}}

def Pol.step(+u: U.Ui, +line: String, +now: Nat) -> Step:
  Pol.step.k(Pol.op(line), u, line, now)

# ---- the loop --------------------------------------------------------------

def Pol.fuel() -> Nat:
  U32.to_nat(4294967295)

def Pol.copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: x <> Pol.copy(t)

def Pol.arg(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

def Pol.next(+stop: Bool, s: Socket, +u: U.Ui, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  match stop:
    case True{}: Socket.close(s)
    case False{}: k(s, u, "")

def Pol.sent(r: Socket & Result<&1, &1, (U32 & String) & String, Unit>, +stop: Bool, +u: U.Ui, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  (s, _) = r
  Pol.next(stop, s, u, k)

def Pol.answer(+st: Step, s: Socket, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  match st:
    case Step{+u, +reply, +stop}:
      IO.bind(Socket & Result<&1, &1, (U32 & String) & String, Unit>, Unit, TCP.send(s, reply ++ "\n"),
        r => Pol.sent(r, stop, u, k))

# a complete request: answer it, then go on with the state it left
def Pol.chunk(+whole: Bool, s: Socket, +u: U.Ui, +buf: String, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  match whole:
    case False{}: k(s, u, buf)
    case True{}:
      do IO<Unit>:
        +now : Nat <- IO.now()
        Pol.answer(Pol.step(u, String.trim(buf), now), s, k)

def Pol.got.k(res: Result<&1, &1, U32 & String, Maybe<&1, String>>, s: Socket, +u: U.Ui, +buf: String, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  match res:
    case Fail{_}: Socket.close(s)
    case Done{None{}}: Socket.close(s)
    case Done{Some{+chunk}}: Pol.chunk(String.ends_with(chunk, "\n"), s, u, buf ++ chunk, k)

def Pol.got(r: Socket & Result<&1, &1, U32 & String, Maybe<&1, String>>, +u: U.Ui, +buf: String, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  Pol.got.k(res, s, u, buf, k)

def Pol.loop(fuel: Nat, s: Socket, +u: U.Ui, +buf: String) -> IO(Unit):
  match fuel:
    case 0n: Socket.close(s)
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, Maybe<&1, String>>, Unit, TCP.recv(s, 1048576),
        r => Pol.got(r, u, buf, s2 => u2 => b2 => Pol.loop(f, s2, u2, b2)))

def Pol.accepted(r: Listener & Result<&1, &1, U32 & String, Socket>, +now: Nat) -> IO(Unit):
  (l, res) = r
  match res:
    case Done{s}:
      do IO<Unit>:
        Listener.close(l)
        Pol.loop(Pol.fuel(), s, U.Ui.init(now), "")
    case Fail{(code, msg)}:
      IO.die(Unit, code, msg)

def Pol.main(+xs: List<&2, String>, +now: Nat) -> IO(Unit):
  do IO<Unit>:
    l : Listener <- IO.try(Listener, TCP.listen("127.0.0.1", U32.from_nat(Maybe.default(&2, Nat, Nat.read(Pol.arg(xs, 1n)), 0n))))
    IO.print("bp-policy listening")
    IO.bind(Listener & Result<&1, &1, U32 & String, Socket>, Unit, TCP.accept(l), r => Pol.accepted(r, now))

def main() -> IO(Unit):
  do IO<Unit>:
    a : List<&1, String> <- IO.args()
    +xs : List<&2, String> = Pol.copy(a)
    +now : Nat <- IO.now()
    Pol.main(xs, now)
