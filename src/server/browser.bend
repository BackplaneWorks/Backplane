import Base
import ./host.bend as H
import ../core/json.bend as J
import ../core/utf8.bend as U
import ../core/desktop-browser.bend as DB
import ./desktop-broker.bend as DR

# The browser Backplane hosts for its agents: one backplane-browser helper
# (headless Chromium, tools/browser), started on the first request and
# spoken to in JSON lines. Requests carry ids; a reply goes back to the
# channel its request came with.
#
# Pages are keyed by an owner (a bot's id for a bot's thread, else the
# thread's id, so each thread's preview_* tools drive a page of their
# own; "" is the shared page), all in one persistent Chrome profile at
# <home>/browser-profile, so logins last. Every page's JPEG frames (which
# every client's Browser tab shows) go to <home>/shots/<owner>.jpg (the
# helper's raw RGB frames of the shared page are never turned on: nothing
# reads them), and each new one is announced on
# the loop's event channel as the helper's line
# {"event":"frame","page":<owner>,"n":<n>,...}; Browser.shot turns that
# into the hub's {"t":"shot","bot":<owner>,"n":<n>}.

type BMsg is Data:
  BRefresh{}
  # Private authenticated Electron channel; never an ordinary UI broadcast.
  BRemote{client: U32, kind: String, text: String, out: Chan(String), epoch: String, lease: String, owner: String, known: Bool, drive: String, recording: Bool}
  BPolicy{owner: String, known: Bool, drive: String, recording: Bool}
  BDeadline{id: String}
  BCaptureCommit{candidate: J.Json, permit: Chan(J.Json), released: Chan(Unit)}
  BCall{op: String, args: J.Json, reply: Chan(String)}
  BLine{text: String}
  BDead{}

type Pend is Data:
  Pend{id: String, reply: Chan(String)}

# the helper's input (None until it runs), the requests waiting for a
# reply, the next id, and the helper's path and Backplane's home
type Br is Data:
  Br{input: Maybe<&2, Chan(String)>, pend: List<&2, Pend>, next: Nat, exe: String, home: String}

# the browser of the Backplane at this home
def Br.at(+exe: String, +home: String) -> Br:
  Br{None{}, Nil{}, 1n, exe, home}

# the Chrome profile, the JPEG frames
def Br.profile(+home: String) -> String:
  home ++ "/browser-profile"

def Br.shots(+home: String) -> String:
  home ++ "/shots"

# an owner's JPEG frame file name, as the helper writes it: [A-Za-z0-9_-]
# kept, anything else '_', "" is "shared"
def Br.safe.ok(+u: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(u, 48), U32.is_le(u, 57)), Bool.and(U32.is_ge(u, 65), U32.is_le(u, 90))),
    Bool.or(Bool.and(U32.is_ge(u, 97), U32.is_le(u, 122)), Bool.or(U32.is_eq(u, 45), U32.is_eq(u, 95))))

def Br.safe.c(c: Char) -> Char:
  match c:
    case Chr{+u}:
      Chr{Bool.pick(U32, Br.safe.ok(u), u, 95)}

def Br.safe.go(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{c, t}:
      SCon{Br.safe.c(c), Br.safe.go(t)}

def Br.safe(+owner: String) -> String:
  Bool.pick(String, String.is_empty(owner), "shared", Br.safe.go(owner))

# where the helper writes an owner's newest JPEG frame
def Br.shot_file(+home: String, +owner: String) -> String:
  Br.shots(home) ++ "/" ++ Br.safe(owner) ++ ".jpg"

def Browser.forever() -> Nat:
  4000000000n

def Browser.fail(+error: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{error}}]))

def Browser.say(+reply: Chan(String), +text: String) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, String, Unit> <- Chan.send(String, reply, text)
    return Unit{}

# Reading the helper
# ------------------

def Browser.lines(ls: List<&2, String>, +q: Chan(BMsg)) -> IO(Unit):
  match ls:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case l <> t:
      do IO<Unit>:
        ok : Result<&1, &1, BMsg, Unit> <- Chan.send(BMsg, q, BLine{l})
        Browser.lines(t, q)

def Browser.end(s: Socket, +pid: U32, +q: Chan(BMsg)) -> IO(Unit):
  do IO<Unit>:
    Socket.close(s)
    code : U32 <- H.Proc.wait(pid)
    ok : Result<&1, &1, BMsg, Unit> <- Chan.send(BMsg, q, BDead{})
    return Unit{}

def Browser.fed(r: U.Lines & List<&2, String>, s: Socket, +q: Chan(BMsg), k: Socket -> U.Lines -> IO(Unit)) -> IO(Unit):
  (carry, ls) = r
  do IO<Unit>:
    Browser.lines(ls, q)
    k(s, carry)

def Browser.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, carry: U.Lines, +pid: U32, +q: Chan(BMsg), k: Socket -> U.Lines -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      Browser.end(s, pid, q)
    case Done{bs}:
      Browser.fed(U.Lines.feed(carry, bs), s, q, k)
    case Fail{_}:
      Browser.end(s, pid, q)

def Browser.reader(fuel: Nat, s: Socket, carry: U.Lines, +pid: U32, +q: Chan(BMsg)) -> IO(Unit):
  match fuel:
    case 0n:
      Browser.end(s, pid, q)
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536),
        r => Browser.got(r, carry, pid, q, s2 => c2 => Browser.reader(f, s2, c2, pid, q)))

def Browser.wrote(r: Socket & Result<&1, &1, U32 & String, Unit>, k: Socket -> IO(Unit)) -> IO(Unit):
  (s, _) = r
  k(s)

def Browser.write(m: Maybe<&1, String>, s: Socket, k: Socket -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{line}:
      IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Unit, H.Sock.send_text(s, line ++ "\n"), r => Browser.wrote(r, k))
    case None{}:
      Socket.close(s)

def Browser.writer(fuel: Nat, s: Socket, +input: Chan(String)) -> IO(Unit):
  match fuel:
    case 0n:
      Socket.close(s)
    case 1n+f:
      IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, input), m => Browser.write(m, s, s2 => Browser.writer(f, s2, input)))

# Starting the helper
# -------------------

def Browser.duped(r: Socket & Result<&1, &1, U32 & String, Socket>, +pid: U32, +q: Chan(BMsg)) -> IO(Maybe<&2, Chan(String)>):
  (rd, res) = r
  match res:
    case Done{wr}:
      do IO<Maybe<&2, Chan(String)>>:
        +input : Chan(String) <- Chan.new(String, 256)
        IO.spawn(Unit, Browser.writer(Browser.forever(), wr, input))
        IO.spawn(Unit, Browser.reader(Browser.forever(), rd, U.Lines.new(), pid, q))
        return Some{input}
    case Fail{_}:
      do IO<Maybe<&2, Chan(String)>>:
        Socket.close(rd)
        return None{}

def Browser.spawned.ok(started: U32 & Socket, +q: Chan(BMsg)) -> IO(Maybe<&2, Chan(String)>):
  (+pid, s) = started
  IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Maybe<&2, Chan(String)>, H.Sock.dup(s), r => Browser.duped(r, pid, q))

def Browser.spawned(r: Result<&1, &1, U32 & String, U32 & Socket>, +q: Chan(BMsg)) -> IO(Maybe<&2, Chan(String)>):
  match r:
    case Done{started}:
      Browser.spawned.ok(started, q)
    case Fail{_}:
      IO.pure(Maybe<&2, Chan(String)>, None{})

def Browser.start(+exe: String, +home: String, +q: Chan(BMsg)) -> IO(Maybe<&2, Chan(String)>):
  do IO<Maybe<&2, Chan(String)>>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Br.shots(home))
    r : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn(exe, ["--profile", Br.profile(home), "--jpeg-dir", Br.shots(home)], "/", 0)
    Browser.spawned(r, q)

# the running helper's input, starting it if needed
def Browser.ensure(m: Maybe<&2, Chan(String)>, +exe: String, +home: String, +q: Chan(BMsg)) -> IO(Maybe<&2, Chan(String)>):
  match m:
    case Some{c}:
      IO.pure(Maybe<&2, Chan(String)>, Some{c})
    case None{}:
      Browser.start(exe, home, q)

# The actor
# ---------

# the request line: the tool's arguments with the id and op beside them
def Browser.request(+id: Nat, +op: String, args: J.Json) -> String:
  match args:
    case J.Obj{fs}:
      J.Json.show(J.Obj{J.Field{"id", J.Num{Nat.show(id)}, J.Field{"op", J.Str{op}, fs}}})
    case _:
      J.Json.show(J.Obj{J.Field{"id", J.Num{Nat.show(id)}, J.Field{"op", J.Str{op}, J.End{}}}})

def Browser.sent(m: Maybe<&2, Chan(String)>, +line: String, +id: Nat, +reply: Chan(String), b: Br) -> IO(Br):
  match m b:
    case Some{+c} Br{_, ps, n, exe, home}:
      do IO<Br>:
        ok : Result<&1, &1, String, Unit> <- Chan.send(String, c, line)
        return Br{Some{c}, Pend{Nat.show(id), reply} <> ps, n, exe, home}
    case None{} Br{_, ps, n, exe, home}:
      do IO<Br>:
        Browser.say(reply, Browser.fail("the Backplane browser could not start (is backplane-browser installed next to Backplane?)"))
        return Br{None{}, ps, n, exe, home}

def Browser.call(b: Br, +q: Chan(BMsg), +op: String, args: J.Json, +reply: Chan(String)) -> IO(Br):
  match b:
    case Br{m, ps, +n, +exe, +home}:
      IO.bind(Maybe<&2, Chan(String)>, Br, Browser.ensure(m, exe, home, q),
        m2 => Browser.sent(m2, Browser.request(n, op, args), n, reply, Br{None{}, ps, 1n+n, exe, home}))

type Took is Data:
  Took{reply: Maybe<&2, Chan(String)>, rest: List<&2, Pend>}

def Took.keep(p: Pend, t: Took) -> Took:
  match t:
    case Took{r, ps}:
      Took{r, p <> ps}

def Pend.hit(ps: List<&2, Pend>, +id: String) -> Bool:
  match ps:
    case Pend{pid, _} <> _:
      String.eq(pid, id)
    case Nil{}:
      False{}

# the waiting request with this id, taken out of the list
def Pend.take(ps: List<&2, Pend>, hit: Bool, +id: String) -> Took:
  match ps hit:
    case Pend{_, reply} <> t True{}:
      Took{Some{reply}, t}
    case p <> +t False{}:
      Took.keep(p, Pend.take(t, Pend.hit(t, id), id))
    case Nil{} _:
      Took{None{}, Nil{}}

def Browser.deliver.k(t: Took, +text: String) -> IO(List<&2, Pend>):
  match t:
    case Took{Some{reply}, rest}:
      do IO<List<&2, Pend>>:
        Browser.say(reply, text)
        return rest
    case Took{None{}, rest}:
      IO.pure(List<&2, Pend>, rest)

# answer the request with this id; the rest keep waiting
def Browser.deliver(+ps: List<&2, Pend>, +id: String, +text: String) -> IO(List<&2, Pend>):
  Browser.deliver.k(Pend.take(ps, Pend.hit(ps, id), id), text)

def Browser.has_page(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{J.Str{_}}:
      True{}
    case _:
      False{}

# a helper line announcing a page's new JPEG frame:
# {"event":"frame","page":<owner>,"n":<n>,"path":...} (the shared page's
# raw frame events carry no page)
def Browser.is_shot(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "event", ""), "frame"), Browser.has_page(J.Json.get(o, "page")))

# a helper line about an owner's page that the hub hears: its JPEG frame, a
# step the recorder saw (record), a navigation or a load (docs/teach.md);
# and its tabs
def Browser.is_page_event(+o: J.Json) -> Bool:
  Bool.or(Bool.and(Browser.has_page(J.Json.get(o, "page")), Bool.or(String.eq(J.Json.get_str(o, "event", ""), "frame"),
    Bool.or(String.eq(J.Json.get_str(o, "event", ""), "record"), Bool.or(String.eq(J.Json.get_str(o, "event", ""), "navigated"),
      String.eq(J.Json.get_str(o, "event", ""), "load"))))),
    String.eq(J.Json.get_str(o, "event", ""), "tabs"))

# a page-frame event to the event channel, sent off the actor's thread so
# a slow reader never holds the browser up
def Browser.event(hit: Bool, +ev: Chan(String), +text: String) -> IO(Unit):
  match hit:
    case True{}:
      IO.spawn(Unit, Browser.say(ev, text))
    case False{}:
      IO.pure(Unit, Unit{})

def Browser.line.k(b: Br, +ev: Chan(String), +text: String, +o: J.Json) -> IO(Br):
  match b:
    case Br{m, ps, n, exe, home}:
      do IO<Br>:
        Browser.event(Browser.is_page_event(o), ev, text)
        ps2 : List<&2, Pend> <- Browser.deliver(ps, J.Json.as_raw(J.Json.get(o, "id"), ""), text)
        return Br{m, ps2, n, exe, home}

# a line from the helper: a reply (by id), a page's frame event (to the
# event channel), a record, navigated or load event of a page (the same), or another event (ignored here)
def Browser.line(b: Br, +ev: Chan(String), +text: String) -> IO(Br):
  Browser.line.k(b, ev, text, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))

def Browser.fail_all(ps: List<&2, Pend>) -> IO(Unit):
  match ps:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case Pend{_, reply} <> t:
      do IO<Unit>:
        Browser.say(reply, Browser.fail("the browser exited"))
        Browser.fail_all(t)

# the helper went away: fail what waits; the next request starts it again
def Browser.dead(b: Br, +ev: Chan(String)) -> IO(Br):
  match b:
    case Br{_, ps, n, exe, home}:
      do IO<Br>:
        Browser.fail_all(ps)
        return Br{None{}, Nil{}, n, exe, home}

def Browser.step(m: Maybe<&1, BMsg>, b: Br, +q: Chan(BMsg), +ev: Chan(String), k: Br -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{BRefresh{}}: k(b)
    case Some{BRemote{_, _, _, +out, _, _, _, _, _, _}}:
      do IO<Unit>:
        Browser.say(out, "{\"v\":1,\"op\":\"error\",\"error\":\"desktop browser mode is disabled\"}")
        Chan.close(String, out)
        k(b)
    case Some{BPolicy{_, _, _, _}}: k(b)
    case Some{BDeadline{_}}: k(b)
    case Some{BCaptureCommit{_, +permit, released}}:
      do IO<Unit>:
        sent : Result<&1, &1, J.Json, Unit> <- Chan.send(J.Json, permit, J.Null{})
        k(b)
    case Some{BCall{op, args, reply}}:
      IO.bind(Br, Unit, Browser.call(b, q, op, args, reply), k)
    case Some{BLine{t}}:
      IO.bind(Br, Unit, Browser.line(b, ev, t), k)
    case Some{BDead{}}:
      IO.bind(Br, Unit, Browser.dead(b, ev), k)
    case None{}:
      IO.pure(Unit, Unit{})

# the browser actor: requests on q, helper lines back on q; every page's
# new-JPEG event line goes to ev (see Browser.shot)
def Browser.loop(fuel: Nat, +q: Chan(BMsg), +ev: Chan(String), b: Br) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, BMsg>, Unit, Chan.recv(BMsg, q), m => Browser.step(m, b, q, ev, b2 => Browser.loop(f, q, ev, b2)))

# the hub's message for a page-frame event line:
# {"t":"shot","bot":<owner>,"n":<n>}
def Browser.shot.k(+o: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"shot"}}, J.KV{"bot", J.Str{J.Json.get_str(o, "page", "")}}, J.KV{"n", J.Num{J.Json.as_raw(J.Json.get(o, "n"), "0")}}]))

def Browser.shot(+line: String) -> String:
  Browser.shot.k(Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}))

def Browser.drain.k(m: Maybe<&1, String>, k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{_}:
      k(Unit{})
    case None{}:
      IO.pure(Unit, Unit{})

# reads and drops the event channel, for a hub that shows no page frames
def Browser.drain(fuel: Nat, +ev: Chan(String)) -> IO(Unit):
  match fuel:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, String>, Unit, Chan.recv(String, ev), m => Browser.drain.k(m, u => Browser.drain(f, ev)))

# the request's fields but "page": the owner is set by the caller only
def Browser.unpaged(fs: J.Json) -> J.Json:
  match fs:
    case J.Field{"page", _, t}:
      Browser.unpaged(t)
    case J.Field{"__desktopActor", _, t}:
      Browser.unpaged(t)
    case J.Field{k, v, t}:
      J.Field{k, v, Browser.unpaged(t)}
    case _:
      fs

# the arguments with "page" set to this owner (and any page they carried
# dropped, so tool arguments cannot reach another owner's page)
def Browser.paged(+page: String, args: J.Json) -> J.Json:
  match args:
    case J.Obj{fs}:
      J.Obj{J.Field{"page", J.Str{page}, Browser.unpaged(fs)}}
    case _:
      J.Obj{J.Field{"page", J.Str{page}, J.End{}}}

# ask the browser on an owner's page ("" is the shared page); answers the
# helper's reply line
def Browser.ask_on(+q: Chan(BMsg), +page: String, +op: String, args: J.Json) -> IO(String):
  do IO<String>:
    +reply : Chan(String) <- Chan.new(String, 1)
    ok : Result<&1, &1, BMsg, Unit> <- Chan.send(BMsg, q, BCall{op, Browser.paged(page, args), reply})
    m : Maybe<&1, String> <- Chan.recv(String, reply)
    return Maybe.default(&1, String, m, Browser.fail("no reply"))

# Only the real preview dispatcher can stamp agent provenance; tool arguments
# cannot override it or the owning page. The broker rechecks current drive.
def Browser.ask_agent_on(+q: Chan(BMsg), +page: String, +op: String, args: J.Json) -> IO(String):
  do IO<String>:
    +reply : Chan(String) <- Chan.new(String, 1)
    sent : Result<&1, &1, BMsg, Unit> <- Chan.send(BMsg, q, BCall{op, DB.Desktop.put(Browser.paged(page, args), "__desktopActor", J.Str{"agent"}), reply})
    m : Maybe<&1, String> <- Chan.recv(String, reply)
    return Maybe.default(&1, String, m, Browser.fail("no reply"))

def Browser.remote_forward(m: Maybe<&1, BMsg>, +remote: Chan(DR.Message), k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{BRefresh{}}:
      do IO<Unit>:
        sent : Result<&1, &1, DR.Message, Unit> <- Chan.send(DR.Message, remote, DR.Refresh{})
        k(Unit{})
    case Some{BRemote{client, kind, text, out, e, l, owner, known, drive, recording}}:
      do IO<Unit>:
        sent : Result<&1, &1, DR.Message, Unit> <- Chan.send(DR.Message, remote, DR.Link{client, kind, text, out, e, l, owner, known, drive, recording})
        k(Unit{})
    case Some{BPolicy{owner, known, drive, recording}}:
      do IO<Unit>:
        sent : Result<&1, &1, DR.Message, Unit> <- Chan.send(DR.Message, remote, DR.Policy{owner, known, drive, recording})
        k(Unit{})
    case Some{BCaptureCommit{candidate, permit, released}}:
      do IO<Unit>:
        sent : Result<&1, &1, DR.Message, Unit> <- Chan.send(DR.Message, remote, DR.CaptureCommit{candidate, permit, released})
        k(Unit{})
    case Some{BCall{op, args, reply}}:
      do IO<Unit>:
        sent : Result<&1, &1, DR.Message, Unit> <- Chan.send(DR.Message, remote, DR.Call{op, args, reply})
        k(Unit{})
    case Some{_}: k(Unit{})
    case None{}: Chan.close(DR.Message, remote)

def Browser.remote_loop(fuel: Nat, +q: Chan(BMsg), +remote: Chan(DR.Message)) -> IO(Unit):
  match fuel:
    case 0n: Chan.close(DR.Message, remote)
    case 1n+n:
      IO.bind(Maybe<&1, BMsg>, Unit, Chan.recv(BMsg, q), m => Browser.remote_forward(m, remote, _ => Browser.remote_loop(n, q, remote)))

def Browser.run(desktop: Bool, +q: Chan(BMsg), ev: Chan(String), b: Br, +version: String) -> IO(Unit):
  match desktop:
    case False{}: Browser.loop(Browser.forever(), q, ev, b)
    case True{}:
      do IO<Unit>:
        +remote : Chan(DR.Message) <- Chan.new(DR.Message, 256)
        IO.spawn(Unit, DR.Broker.loop(Browser.forever(), remote, version, ev, DR.Broker.empty()))
        Browser.remote_loop(Browser.forever(), q, remote)

# ask the browser on the shared page; answers the helper's reply line
def Browser.ask(+q: Chan(BMsg), +op: String, args: J.Json) -> IO(String):
  Browser.ask_on(q, "", op, args)
