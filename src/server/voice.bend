import Base
import ./host.bend as H
import ./proc.bend as P
import ./botnet.bend as BN
import ./kicad.bend as K
import ../core/json.bend as J
import ../core/utf8.bend as U
import ../core/codec.bend as C
import ../core/voice.bend as V
import ../core/vdict.bend as VD

# Dictation's IO (docs/voice.md): the OpenAI key and the dictionary kept in
# the home, a live session of the desktop's microphone run by the
# backplane-voice helper (tools/voice, GPT-Live-Transcribe), and a Pebble
# ring's recording transcribed. Nothing here talks to the hub: a live
# session's messages go out on a channel.
#
# The key is a secret (<home>/secrets/openai.key, 0600). The helper reads
# it from the file, so it is never in an argv, an environment, the log or a
# message; clients only ever see it masked. The dictionary
# (<home>/voice/dictionary.txt, a term a line) is the helper's keywords.

def Vo.key(+home: String) -> String:
  home ++ "/secrets/openai.key"

def Vo.dir(+home: String) -> String:
  home ++ "/voice"

def Vo.dict(+home: String) -> String:
  home ++ "/voice/dictionary.txt"

# the terms as the helper's keywords (the cloud's), written at each start
def Vo.keywords(+home: String) -> String:
  home ++ "/voice/keywords.txt"

def Vo.pid(+home: String) -> String:
  home ++ "/voice/live.pid"

def Vo.forever() -> Nat:
  4000000000n

def Vo.exe() -> IO(String):
  K.Helper.find("backplane-voice")

# Key and dictionary
# ------------------

# the key as a client may see it: its first three and last four characters
def Vo.mask(+k: String) -> String:
  Bool.pick(String, Nat.is_lt(String.length(k), 12n), Bool.pick(String, String.is_empty(k), "", "set"),
    String.take(k, 3n) ++ "…" ++ String.drop(k, Nat.sub(String.length(k), 4n)))

# an OpenAI key: "sk-" and no spaces or control characters (core
# voice.bend's, which the web's Save checks too)
def Vo.key.ok(+k: String) -> Bool:
  V.Voice.openai.ok(k)

def Vo.write.opened(r: Result<&1, &1, U32 & String, File>, text: String) -> IO(Bool):
  BN.Bf.write.opened(r, text)

def Vo.write(+path: String, text: String) -> IO(Bool):
  do IO<Bool>:
    f : Result<&1, &1, U32 & String, File> <- File.open(path, "w")
    Vo.write.opened(f, text)

def Vo.status.of(+k: String, +d: String) -> List<&2, J.KV>:
  [J.KV{"voiceKey", J.Str{Vo.mask(String.trim(k))}}, J.KV{"voiceDict", J.Str{V.Voice.dict.show(V.Voice.dict.parse(d))}}]

# what a client shows in Settings: the key masked, the dictionary
def Vo.status(+home: String) -> IO(List<&2, J.KV>):
  do IO<List<&2, J.KV>>:
    +k : String <- BN.Bf.text(Vo.key(home))
    +d : String <- BN.Bf.text(Vo.dict(home))
    return Vo.status.of(k, d)

def Vo.set_key.k(ok: Bool, +home: String, +key: String) -> IO(String):
  match ok:
    case True{}:
      do IO<String>:
        BN.Bf.dir(home ++ "/secrets")
        w : Bool <- BN.Bf.secret(Vo.key(home), key ++ "\n")
        return Bool.pick(String, w, "", "error: the key could not be written")
    case False{}:
      IO.pure(String, "error: that is not an OpenAI API key (they start with sk-)")

# keep the key ("" when kept, else why not)
def Vo.set_key(+home: String, +key: String) -> IO(String):
  Vo.set_key.k(Vo.key.ok(String.trim(key)), home, String.trim(key))

def Vo.unkey(+home: String) -> IO(String):
  do IO<String>:
    BN.Bf.rm(Vo.key(home))
    return ""

# the dictionary as typed: a term a line, tidied (V.Voice.dict.parse)
def Vo.set_dict(+home: String, +text: String) -> IO(String):
  do IO<String>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Vo.dir(home))
    w : Bool <- Vo.write(Vo.dict(home), V.Voice.dict.show(V.Voice.dict.parse(text)) ++ "\n")
    return Bool.pick(String, w, "", "error: the dictionary could not be written")

# one term more (a word the person clarified)
def Vo.add(+home: String, +term: String) -> IO(String):
  do IO<String>:
    +d : String <- BN.Bf.text(Vo.dict(home))
    Vo.set_dict(home, V.Voice.dict.show(V.Voice.dict.add(V.Voice.dict.parse(d), term)))

# Live sessions
# -------------
# One at a time. Its pid is kept in <home>/voice/live.pid so a stop (from
# any client) can reach it; a pid is signalled only while `ps` says it is
# still backplane-voice, so a stale file never names another process.

# {"t":"voice","thread":..,"ev":..} for the client that started it:
# ev live (with the recorder and the microphone it opened), part (words so
# far), text (a finished segment, with the words to clarify, and "firm"
# when it opens with a dictionary term), level (0..100,
# a few times a second), hint (no-audio, silent, ok), stopping, error,
# idle (always last)
def Vo.msg(+thread: String, +ev: String, kvs: List<&2, J.KV>) -> String:
  J.Json.show(J.Json.obj(J.KV{"t", J.Str{"voice"}} <> J.KV{"thread", J.Str{thread}} <> J.KV{"ev", J.Str{ev}} <> kvs))

def Vo.err(+thread: String, +e: String) -> String:
  Vo.msg(thread, "error", [J.KV{"error", J.Str{e}}])

# The session
# -----------
# What a start asks for (bothub.bend's Vreq adds the dictionary and the
# settings to every bots.voice request): the engine ("local" or "openai",
# chosen by Vo.engine), the local model, where the audio comes from ("hub":
# this machine's microphone; "client": a client's, uploaded to /voice/in
# under `key`) and the dictionary.
type VoReq is Data:
  VoReq{engine: String, model: String, src: String, key: String, d: VD.VDict}

def VoReq.engine(+r: VoReq) -> String:
  match r:
    case VoReq{e, _, _, _, _}:
      e

def VoReq.model(+r: VoReq) -> String:
  match r:
    case VoReq{_, m, _, _, _}:
      m

def VoReq.src(+r: VoReq) -> String:
  match r:
    case VoReq{_, _, s, _, _}:
      s

def VoReq.key(+r: VoReq) -> String:
  match r:
    case VoReq{_, _, _, k, _}:
      k

def VoReq.d(+r: VoReq) -> VD.VDict:
  match r:
    case VoReq{_, _, _, _, d}:
      d

def VoReq.timed(+r: VoReq) -> Bool:
  String.eq(VoReq.engine(r), "local")

# the reader's state: the settling segment, the cloud's text so far, the
# segment's corrected words and guesses so far, the tail last sent
type VoSt is Data:
  VoSt{set: VD.VSet, acc: String, seg: List<&2, VD.VW>, ask: List<&2, V.Unsure>, tl: String, fxs: List<&2, VD.VFx>}

def Vo.st0() -> VoSt:
  VoSt{VD.Vs.empty(), "", Nil{}, Nil{}, "", Nil{}}

# what one helper line gives: messages for the client and the state after
type VoOut is Data:
  VoOut{msgs: List<&2, String>, st: VoSt}

def Vo.one(+m: String, st: VoSt) -> VoOut:
  VoOut{Bool.pick(List<&2, String>, String.is_empty(m), Nil{}, [m]), st}

# ev words: "first" when nothing of the segment was committed before, the
# words committed now (never to change; corrected), the tail
# (may still change), "firm" when the committed words open with a term,
# the segment's fixes so far, and on its end "final" with the words to clarify
def Vo.words.msg(+thread: String, +item: String, +first: Bool, +commit: String, +tail: String, +firm: Bool, +fixes: J.Json, +fin: Bool, +unsure: J.Json) -> String:
  Vo.msg(thread, "words", [J.KV{"item", J.Str{item}}, J.KV{"first", J.Flag{first}}, J.KV{"commit", J.Str{commit}}, J.KV{"tail", J.Str{tail}}, J.KV{"firm", J.Flag{firm}},
    J.KV{"fixes", fixes}, J.KV{"final", J.Flag{fin}}, J.KV{"unsure", unsure}])

def Vo.words.k(quiet: Bool, +m: String, st: VoSt) -> VoOut:
  match quiet:
    case True{}:
      VoOut{Nil{}, st}
    case False{}:
      VoOut{[m], st}

def Vo.words.go(+thread: String, +item: String, +d: VD.VDict, +fin: Bool, +fx: VD.VFix, +tail: String, +set: VD.VSet, +acc: String, +seg: List<&2, VD.VW>, +ask: List<&2, V.Unsure>, +tl: String, +fxs: List<&2, VD.VFx>) -> VoOut:
  Vo.words.k(Bool.and(Bool.not(fin), Bool.and(List.is_empty(&2, VD.VW, VD.VFix.words(fx)), String.eq(tail, tl))),
    Vo.words.msg(thread, item, List.is_empty(&2, VD.VW, seg), VD.Vdc.text(VD.VFix.words(fx)), tail, V.Voice.firm(VD.Vdc.terms(d), VD.Vdc.text(VD.VFix.words(fx))), VD.Vdc.fixes.json(List.append(&2, VD.VFx, fxs, VD.VFix.fixes(fx))), fin,
      Bool.pick(J.Json, fin, V.Voice.unsure.json(VD.Vdc.asks(d, Nil{}, VD.VFix{List.append(&2, VD.VW, seg, VD.VFix.words(fx)), Nil{}, List.append(&2, V.Unsure, ask, VD.VFix.ask(fx))})), J.Json.arr(Nil{}))),
    Bool.pick(VoSt, fin, VoSt{set, "", Nil{}, Nil{}, "", Nil{}}, VoSt{set, acc, List.append(&2, VD.VW, seg, VD.VFix.words(fx)), List.append(&2, V.Unsure, ask, VD.VFix.ask(fx)), tail, List.append(&2, VD.VFx, fxs, VD.VFix.fixes(fx))}))

# a settled step into a message: the committed words corrected, the tail
# corrected for display; nothing when nothing changed
def Vo.words(+thread: String, +item: String, +d: VD.VDict, fin: Bool, +x: VD.VStep, st: VoSt) -> VoOut:
  match st:
    case VoSt{_, +acc, +seg, +ask, +tl, +fxs}:
      Vo.words.go(thread, item, d, fin, VD.Vdc.fix(d, VD.VStep.commit(x)), VD.Vdc.text(VD.VFix.words(VD.Vdc.fix(d, VD.VStep.tail(x)))), VD.VStep.st(x), acc, seg, ask, tl, fxs)

def Vo.set(+st: VoSt) -> VD.VSet:
  match st:
    case VoSt{s, _, _, _, _, _}:
      s

def Vo.acc(+st: VoSt) -> String:
  match st:
    case VoSt{_, a, _, _, _, _}:
      a

# the cloud's deltas for item, added to what came before for it
def Vo.acc.for(+st: VoSt, +item: String, +text: String) -> String:
  Bool.pick(String, String.eq(VD.Vs.item(Vo.set(st)), item), Vo.acc(st) ++ text, text)

def Vo.with.acc(st: VoSt, +a: String) -> VoSt:
  match st:
    case VoSt{s, _, g, k, t, f}:
      VoSt{s, a, g, k, t, f}

def Vo.delta(+thread: String, +item: String, +a: String, +d: VD.VDict, +st: VoSt) -> VoOut:
  Vo.words(thread, item, d, False{}, VD.Vs.hyp(False{}, Vo.set(st), item, VD.Vdc.words.text(a), 0n, VD.Vdc.waits(d)), Vo.with.acc(st, a))

def Vo.done.ws(timed: Bool, +o: J.Json) -> List<&2, VD.VW>:
  match timed:
    case True{}:
      VD.Vdc.words.json(J.Json.get_obj(o, "words"))
    case False{}:
      VD.Vdc.words.text(J.Json.get_str(o, "text", ""))

def Vo.step.k(i: Nat, +o: J.Json, +thread: String, +r: VoReq, +st: VoSt) -> VoOut:
  match i:
    case 0n:
      Vo.one(Vo.msg(thread, "live", [J.KV{"recorder", J.Str{J.Json.get_str(o, "recorder", "")}}, J.KV{"device", J.Str{J.Json.get_str(o, "device", "")}},
        J.KV{"label", J.Str{J.Json.get_str(o, "label", "")}}, J.KV{"engine", J.Str{VoReq.engine(r)}}]), st)
    case 1n:
      Vo.delta(thread, J.Json.get_str(o, "item", ""), Vo.acc.for(st, J.Json.get_str(o, "item", ""), J.Json.get_str(o, "text", "")), VoReq.d(r), st)
    case 2n:
      Vo.words(thread, J.Json.get_str(o, "item", ""), VoReq.d(r), True{}, VD.Vs.done(VoReq.timed(r), Vo.set(st), J.Json.get_str(o, "item", ""), Vo.done.ws(VoReq.timed(r), o)), st)
    case 3n:
      Vo.one(Vo.msg(thread, "stopping", Nil{}), st)
    case 4n:
      Vo.one(Vo.err(thread, J.Json.get_str(o, "error", "dictation failed")), st)
    case 5n:
      Vo.one(Vo.msg(thread, "level", [J.KV{"v", J.Num{Nat.show(J.Json.get_nat(o, "v", 0n))}}]), st)
    case 6n:
      Vo.one(Vo.msg(thread, "hint", [J.KV{"hint", J.Str{J.Json.get_str(o, "hint", "")}}]), st)
    case 7n:
      Vo.words(thread, J.Json.get_str(o, "item", ""), VoReq.d(r), False{},
        VD.Vs.hyp(True{}, Vo.set(st), J.Json.get_str(o, "item", ""), VD.Vdc.words.json(J.Json.get_obj(o, "words")), J.Json.get_nat(o, "end", 0n), VD.Vdc.waits(VoReq.d(r))), st)
    case 8n:
      Vo.one(Vo.msg(thread, "loaded", [J.KV{"ms", J.Num{Nat.show(J.Json.get_nat(o, "ms", 0n))}}]), st)
    case _:
      VoOut{Nil{}, st}

def Vo.step.m(m: Maybe<&2, J.Json>, +thread: String, +r: VoReq, st: VoSt) -> VoOut:
  match m:
    case Some{+o}:
      Vo.step.k(C.Codec.pos(["ready", "delta", "done", "stopping", "error", "level", "hint", "hyp", "loaded"], J.Json.get_str(o, "event", "")), o, thread, r, st)
    case None{}:
      VoOut{Nil{}, st}

# one of the helper's lines: messages for the client, and the state after
def Vo.step(+line: String, +thread: String, +r: VoReq, st: VoSt) -> VoOut:
  Vo.step.m(J.Json.parse(line), thread, r, st)

def Vo.send(+out: Chan(String), +text: String) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, String, Unit> <- Chan.send(String, out, text)
    return Unit{}

def Vo.sends(ms: List<&2, String>, +out: Chan(String)) -> IO(Unit):
  match ms:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +m <> t:
      do IO<Unit>:
        Vo.send(out, m)
        Vo.sends(t, out)

def Vo.said(o: VoOut, +out: Chan(String), k: VoSt -> IO(VoSt)) -> IO(VoSt):
  match o:
    case VoOut{ms, st}:
      do IO<VoSt>:
        Vo.sends(ms, out)
        k(st)

def Vo.say(ls: List<&2, String>, +out: Chan(String), +thread: String, +r: VoReq, st: VoSt) -> IO(VoSt):
  match ls:
    case Nil{}:
      IO.pure(VoSt, st)
    case l <> t:
      Vo.said(Vo.step(l, thread, r, st), out, st2 => Vo.say(t, out, thread, r, st2))

def Vo.unpid.k(same: Bool, +home: String) -> IO(Unit):
  match same:
    case True{}:
      BN.Bf.rm(Vo.pid(home))
    case False{}:
      IO.pure(Unit, Unit{})

# forget the pid file when it still names this session
def Vo.unpid(+home: String, +pid: U32) -> IO(Unit):
  do IO<Unit>:
    +t : String <- BN.Bf.text(Vo.pid(home))
    Vo.unpid.k(String.eq(String.trim(t), U32.show(pid)), home)

def Vo.end(s: Socket, +pid: U32, +home: String, +out: Chan(String), +thread: String, +r: VoReq) -> IO(Unit):
  do IO<Unit>:
    Socket.close(s)
    H.Sock.unkeep(VoReq.key(r))
    code : U32 <- H.Proc.wait(pid)
    Vo.unpid(home, pid)
    Vo.send(out, Vo.msg(thread, "idle", Nil{}))

def Vo.fed(r: U.Lines & List<&2, String>, s: Socket, +out: Chan(String), +thread: String, +rq: VoReq, st: VoSt, k: Socket -> U.Lines -> VoSt -> IO(Unit)) -> IO(Unit):
  (carry, ls) = r
  IO.bind(VoSt, Unit, Vo.say(ls, out, thread, rq, st), st2 => k(s, carry, st2))

def Vo.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, carry: U.Lines, +pid: U32, +home: String, +out: Chan(String), +thread: String,
    +rq: VoReq, st: VoSt, k: Socket -> U.Lines -> VoSt -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      Vo.end(s, pid, home, out, thread, rq)
    case Done{bs}:
      Vo.fed(U.Lines.feed(carry, bs), s, out, thread, rq, st, k)
    case Fail{_}:
      Vo.end(s, pid, home, out, thread, rq)

def Vo.reader(fuel: Nat, s: Socket, carry: U.Lines, +pid: U32, +home: String, +out: Chan(String), +thread: String, +rq: VoReq, st: VoSt) -> IO(Unit):
  match fuel:
    case 0n:
      Vo.end(s, pid, home, out, thread, rq)
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536),
        r => Vo.got(r, carry, pid, home, out, thread, rq, st, s2 => c2 => st2 => Vo.reader(f, s2, c2, pid, home, out, thread, rq, st2)))

# a client's audio goes into the helper's stdin: a second handle on it is
# kept under the session's key for /voice/in
def Vo.keep.d(r: Socket & Result<&1, &1, U32 & String, Socket>, +key: String) -> IO(Socket):
  (s, res) = r
  match res:
    case Done{w}:
      do IO<Socket>:
        H.Sock.keep(w, key)
        return s
    case Fail{_}:
      IO.pure(Socket, s)

def Vo.keep(client: Bool, s: Socket, +key: String) -> IO(Socket):
  match client:
    case True{}:
      IO.bind(Socket & Result<&1, &1, U32 & String, Socket>, Socket, H.Sock.dup(s), r => Vo.keep.d(r, key))
    case False{}:
      IO.pure(Socket, s)

def Vo.started.ok(started: U32 & Socket, +home: String, +out: Chan(String), +thread: String, +rq: VoReq) -> IO(Unit):
  (+pid, s) = started
  do IO<Unit>:
    w : Bool <- Vo.write(Vo.pid(home), U32.show(pid))
    s2 : Socket <- Vo.keep(String.eq(VoReq.src(rq), "client"), s, VoReq.key(rq))
    Vo.reader(Vo.forever(), s2, U.Lines.new(), pid, home, out, thread, rq, Vo.st0())

def Vo.fail(+out: Chan(String), +thread: String, +e: String) -> IO(Unit):
  do IO<Unit>:
    Vo.send(out, Vo.err(thread, e))
    Vo.send(out, Vo.msg(thread, "idle", Nil{}))

def Vo.started(r: Result<&1, &1, U32 & String, U32 & Socket>, +home: String, +out: Chan(String), +thread: String, +rq: VoReq) -> IO(Unit):
  match r:
    case Done{started}:
      Vo.started.ok(started, home, out, thread, rq)
    case Fail{_}:
      Vo.fail(out, thread, "backplane-voice did not start (is it installed next to Backplane?)")

def Vo.alive.k(+cmd: String, +pid: U32) -> U32:
  Bool.pick(U32, String.contains(cmd, "backplane-voice"), pid, 0)

def Vo.alive.ran(r: P.Ran, +pid: U32) -> U32:
  match r:
    case P.Ran{0, +out}:
      Vo.alive.k(out, pid)
    case _:
      0

def Vo.alive.p(m: Maybe<&2, U32>) -> IO(U32):
  match m:
    case Some{+pid}:
      do IO<U32>:
        r : P.Ran <- P.Proc.run("ps", ["-o", "args=", "-p", U32.show(pid)], "/")
        return Vo.alive.ran(r, pid)
    case None{}:
      IO.pure(U32, 0)

# the live session's pid, 0 when none runs
def Vo.alive(+home: String) -> IO(U32):
  do IO<U32>:
    +t : String <- BN.Bf.text(Vo.pid(home))
    Vo.alive.p(U32.read(String.trim(t)))

# Engines
# -------

# where the local engine's runtime and models are
def Vo.local(+home: String) -> String:
  home ++ "/voice/local"

def Vo.model.ok(+m: String) -> String:
  Bool.pick(String, String.eq(m, "parakeet-v3"), "parakeet-v3", "parakeet-v2")

# the model is downloaded whole (the helper writes ready-<model> last)
def Vo.ready(+home: String, +model: String) -> IO(Bool):
  IO.bind(String, Bool, BN.Bf.text(Vo.local(home) ++ "/ready-" ++ Vo.model.ok(model)), t => IO.pure(Bool, String.eq(String.trim(t), "ok")))

# the engine a setting (voice.engine: auto, local, cloud) picks: local when
# asked for, or for auto when the model is downloaded; else the cloud
def Vo.engine.of(+want: String, ready: Bool) -> String:
  match ready:
    case True{}:
      Bool.pick(String, String.eq(want, "cloud"), "openai", "local")
    case False{}:
      Bool.pick(String, String.eq(want, "local"), "local", "openai")

# the helper's arguments
def Vo.src.args(+rq: VoReq, +device: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.eq(VoReq.src(rq), "client"), ["--source", "stdin", "--rate", "16000"],
    Bool.pick(List<&2, String>, String.is_empty(device), Nil{}, ["--device", device]))

def Vo.live.args(+home: String, +rq: VoReq, +device: String) -> List<&2, String>:
  List.append(&2, String,
    Bool.pick(List<&2, String>, VoReq.timed(rq), ["live", "--engine", "local", "--local-dir", Vo.local(home), "--model", Vo.model.ok(VoReq.model(rq))],
      ["live", "--key-file", Vo.key(home), "--keywords-file", Vo.keywords(home)]),
    Vo.src.args(rq, device))

def Vo.live.run(+exe: String, +home: String, +thread: String, +device: String, +rq: VoReq, +out: Chan(String)) -> IO(Unit):
  do IO<Unit>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Vo.dir(home))
    w : Bool <- Vo.write(Vo.keywords(home), String.join(V.Voice.keywords(VD.Vdc.terms(VoReq.d(rq))), "\n"))
    r : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn(exe, Vo.live.args(home, rq, device), "/", 0)
    Vo.started(r, home, out, thread, rq)

def Vo.live.c(copy: Bool, +exe: String, +home: String, +thread: String, +device: String, +rq: VoReq, +out: Chan(String)) -> IO(Unit):
  match copy:
    case True{}:
      Vo.fail(out, thread, "a hub on a copied log (BACKPLANE_COPY=1) starts no dictation")
    case False{}:
      Vo.live.run(exe, home, thread, device, rq, out)

def Vo.live.go(+exe: String, +home: String, +thread: String, +device: String, +rq: VoReq, +out: Chan(String)) -> IO(Unit):
  IO.bind(Bool, Unit, P.Proc.copy(), copy => Vo.live.c(copy, exe, home, thread, device, rq, out))

def Vo.live.k(busy: Bool, keyed: Bool, +exe: String, +home: String, +thread: String, +device: String, +rq: VoReq, +out: Chan(String)) -> IO(Unit):
  match busy keyed:
    case True{} _:
      Vo.fail(out, thread, "already listening: stop the other dictation first")
    case False{} False{}:
      Vo.fail(out, thread, "set up dictation in Settings, Voice: download the local model or add an OpenAI API key")
    case False{} True{}:
      Vo.live.go(exe, home, thread, device, rq, out)

# a live session for `thread`: the helper hears this machine's microphone
# (`device`, "" for the default) or a client's audio, until it is stopped
# (Vo.stop, or the client's last upload); its messages go out on `out`,
# the last one {"ev":"idle"}
def Vo.live(+home: String, +thread: String, +device: String, +rq: VoReq, +out: Chan(String)) -> IO(Unit):
  do IO<Unit>:
    +exe : String <- Vo.exe()
    pid : U32 <- Vo.alive(home)
    +k : String <- BN.Bf.text(Vo.key(home))
    Vo.live.k(U32.is_gt(pid, 0), Bool.or(VoReq.timed(rq), Bool.not(String.is_empty(String.trim(k)))), exe, home, thread, device, rq, out)

# the request's engine resolved: the model's readiness read, then the setting
def Vo.req.k(ready: Bool, +want: String, +model: String, +src: String, +key: String, +d: VD.VDict) -> VoReq:
  VoReq{Vo.engine.of(want, ready), model, src, key, d}

def Vo.req.d(+home: String, +want: String, +model: String, +src: String, +key: String, +d: VD.VDict) -> IO(VoReq):
  IO.bind(Bool, VoReq, Vo.ready(home, model), ok => IO.pure(VoReq, Vo.req.k(ok, want, model, src, key, d)))

# the dictionary.txt terms (bots' voice_dictionary_add, older hubs) join
# the entries the request carried
def Vo.req(+home: String, +want: String, +model: String, +src: String, +key: String, d: VD.VDict) -> IO(VoReq):
  IO.bind(String, VoReq, BN.Bf.text(Vo.dict(home)), t => Vo.req.d(home, want, model, src, key, VD.Vdc.lines(V.Voice.dict.parse(t), d)))

def Vo.devices.m(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{+o}:
      Bool.pick(String, String.eq(J.Json.get_str(o, "event", ""), "devices"), J.Json.show(o), "")
    case None{}:
      ""

def Vo.devices.keep(+acc: String, +l: String) -> String:
  Bool.pick(String, String.is_empty(acc), Vo.devices.m(J.Json.parse(l)), acc)

def Vo.devices.go(ls: List<&2, String>, +acc: String) -> String:
  match ls:
    case Nil{}:
      Bool.pick(String, String.is_empty(acc), "{\"default\":\"\",\"devices\":[]}", acc)
    case +l <> t:
      Vo.devices.go(t, Vo.devices.keep(acc, l))

def Vo.devices.of(ls: List<&2, String>) -> String:
  Vo.devices.go(ls, "")

def Vo.devices.ran(r: P.Ran) -> String:
  match r:
    case P.Ran{_, +out}:
      Vo.devices.of(String.lines(out))

# the microphones the helper finds, as its JSON line ({"default", "devices":
# [{"name", "label"}]})
def Vo.devices() -> IO(String):
  do IO<String>:
    +exe : String <- Vo.exe()
    r : P.Ran <- P.Proc.run(exe, ["devices"], "/")
    return Vo.devices.ran(r)

def Vo.stop.k(none: Bool, +pid: U32) -> IO(Bool):
  match none:
    case True{}:
      IO.pure(Bool, False{})
    case False{}:
      do IO<Bool>:
        H.Proc.kill(pid, 15)
        return True{}

def Vo.stop.p(+pid: U32) -> IO(Bool):
  Vo.stop.k(U32.is_le(pid, 1), pid)

# stop the live session: it sends what it heard, then idle (False when
# none was running)
def Vo.stop(+home: String) -> IO(Bool):
  IO.bind(U32, Bool, Vo.alive(home), p => Vo.stop.p(p))

# Recordings
# ----------

def Vo.done.m(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{+o}:
      Bool.pick(String, String.eq(J.Json.get_str(o, "event", ""), "done"), String.trim(J.Json.get_str(o, "text", "")), "")
    case None{}:
      ""

def Vo.done.keep(+acc: String, +l: String) -> String:
  Bool.pick(String, String.is_empty(acc), Vo.done.m(J.Json.parse(l)), acc)

# the first "done" line's text
def Vo.done.go(ls: List<&2, String>, +acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case +l <> t:
      Vo.done.go(t, Vo.done.keep(acc, l))

def Vo.done.of(ls: List<&2, String>) -> String:
  Vo.done.go(ls, "")

def Vo.file.text(r: P.Ran) -> String:
  match r:
    case P.Ran{_, +out}:
      Vo.done.of(String.lines(out))

# What the last request carried (engine, model, the dictionary): kept for
# a ring's recordings, which come with no client
def Vo.prefs(+home: String) -> String:
  home ++ "/voice/prefs.json"

def Vo.prefs.keep(+home: String, +p: J.Json) -> IO(Unit):
  do IO<Unit>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Vo.dir(home))
    w : Bool <- Vo.write(Vo.prefs(home), J.Json.show(J.Json.obj([J.KV{"engine", J.Str{J.Json.get_str(p, "engine", "auto")}}, J.KV{"model", J.Str{J.Json.get_str(p, "model", "")}},
      J.KV{"entries", J.Json.get_obj(p, "entries")}, J.KV{"learned", J.Json.get_obj(p, "learned")}])))
    return Unit{}

def Vo.prefs.read(+home: String) -> IO(J.Json):
  IO.bind(String, J.Json, BN.Bf.text(Vo.prefs(home)), t => IO.pure(J.Json, Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{})))

def Vo.file.args(+home: String, +rq: VoReq, +path: String, +ct: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, VoReq.timed(rq), ["file", path, "--content-type", ct, "--engine", "local", "--local-dir", Vo.local(home), "--model", Vo.model.ok(VoReq.model(rq))],
    ["file", path, "--content-type", ct, "--key-file", Vo.key(home), "--keywords-file", Vo.keywords(home)])

def Vo.file.k(can: Bool, +exe: String, +home: String, +rq: VoReq, +path: String, +ct: String) -> IO(String):
  match can:
    case True{}:
      do IO<String>:
        r : P.Ran <- P.Proc.run(exe, Vo.file.args(home, rq, path, ct), "/")
        return Vo.file.text(r)
    case False{}:
      IO.pure(String, "")

# a recording's words, corrected with the dictionary ("" with no engine, no
# helper or no words): `path` holds the audio, or a whole multipart form
# whose content type `ct` names
def Vo.file(+home: String, +rq: VoReq, +path: String, +ct: String) -> IO(String):
  do IO<String>:
    +exe : String <- Vo.exe()
    +k : String <- BN.Bf.text(Vo.key(home))
    +t : String <- Vo.file.k(Bool.or(VoReq.timed(rq), Bool.not(String.is_empty(String.trim(k)))), exe, home, rq, path, ct)
    return VD.Vdc.text(VD.VFix.words(VD.Vdc.fix(VoReq.d(rq), VD.Vdc.words.text(t))))

def Vo.bytes.wrote(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Bool):
  BN.Bf.wrote(r)

def Vo.bytes.opened(r: Result<&1, &1, U32 & String, File>, bs: List<&2, U32>) -> IO(Bool):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Bool, File.write_bytes(f, bs), Vo.bytes.wrote)
    case Fail{_}:
      IO.pure(Bool, False{})

# bytes into a file only its owner reads
def Vo.bytes(+path: String, bs: List<&2, U32>) -> IO(Bool):
  do IO<Bool>:
    f : Result<&1, &1, U32 & String, File> <- File.open(path, "w")
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(path, 384)
    Vo.bytes.opened(f, bs)

# A Pebble ring's form with its recording (`raw`, the body's bytes; `ct`
# its content type): the recording is transcribed here with the
# dictionary, and the form comes back with parts put before the ring's own
# (RG.Ring.part takes the first of a name): `transcription` the words
# heard, `unsure` the words to clarify (V.Voice.note), `heard` the model.
# With no key or no words, the form is as it came.
def Vo.ring.form(+body: String, +heard: String, +note: String, +model: String) -> String:
  Bool.pick(String, String.is_empty(heard), body,
    "--backplane\r\nContent-Disposition: form-data; name=\"transcription\"\r\n\r\n" ++ heard ++
    "\r\n--backplane\r\nContent-Disposition: form-data; name=\"unsure\"\r\n\r\n" ++ note ++
    "\r\n--backplane\r\nContent-Disposition: form-data; name=\"heard\"\r\n\r\n" ++ model ++ "\r\n" ++ body)

def Vo.ring.model(+rq: VoReq) -> String:
  Bool.pick(String, VoReq.timed(rq), Vo.model.ok(VoReq.model(rq)), "gpt-live-transcribe")

def Vo.ring.rq(+home: String, +path: String, +ct: String, +body: String, +rq: VoReq) -> IO(String):
  do IO<String>:
    +heard : String <- Vo.file(home, rq, path, ct)
    BN.Bf.rm(path)
    return Vo.ring.form(body, heard, V.Voice.note(VD.Vdc.asks(VoReq.d(rq), Nil{}, VD.VFix{VD.Vdc.words.text(heard), Nil{}, Nil{}})), Vo.ring.model(rq))

def Vo.ring.p(+home: String, +path: String, +ct: String, +body: String, +p: J.Json) -> IO(String):
  IO.bind(VoReq, String, Vo.req(home, J.Json.get_str(p, "engine", "auto"), J.Json.get_str(p, "model", ""), "hub", "", VD.Vdc.of.json(p)), rq => Vo.ring.rq(home, path, ct, body, rq))

def Vo.ring.k(+home: String, +path: String, +ct: String, +body: String) -> IO(String):
  IO.bind(J.Json, String, Vo.prefs.read(home), p => Vo.ring.p(home, path, ct, body, p))

def Vo.ring.w(ok: Bool, +home: String, +path: String, +ct: String, +body: String) -> IO(String):
  match ok:
    case True{}:
      Vo.ring.k(home, path, ct, body)
    case False{}:
      IO.pure(String, body)

def Vo.ring(+home: String, +nonce: String, +ct: String, +body: String, raw: List<&2, U32>) -> IO(String):
  do IO<String>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Vo.dir(home))
    ok : Bool <- Vo.bytes(Vo.dir(home) ++ "/ring-" ++ nonce ++ ".form", raw)
    Vo.ring.w(ok, home, Vo.dir(home) ++ "/ring-" ++ nonce ++ ".form", ct, body)
