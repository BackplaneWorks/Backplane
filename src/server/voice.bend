import Base
import ./host.bend as H
import ./proc.bend as P
import ./botnet.bend as BN
import ./kicad.bend as K
import ../core/json.bend as J
import ../core/utf8.bend as U
import ../core/codec.bend as C
import ../core/voice.bend as V

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

def Vo.text(+thread: String, +item: String, +t: String, +ds: List<&2, String>) -> String:
  Vo.msg(thread, "text", [J.KV{"item", J.Str{item}}, J.KV{"text", J.Str{t}}, J.KV{"firm", J.Flag{V.Voice.firm(ds, t)}},
    J.KV{"unsure", V.Voice.unsure.json(V.Voice.unsure(ds, Nil{}, t))}])

def Vo.line.k(i: Nat, +o: J.Json, +thread: String, +ds: List<&2, String>) -> String:
  match i:
    case 0n:
      Vo.msg(thread, "live", [J.KV{"recorder", J.Str{J.Json.get_str(o, "recorder", "")}}, J.KV{"device", J.Str{J.Json.get_str(o, "device", "")}},
        J.KV{"label", J.Str{J.Json.get_str(o, "label", "")}}])
    case 1n:
      Vo.msg(thread, "part", [J.KV{"item", J.Str{J.Json.get_str(o, "item", "")}}, J.KV{"text", J.Str{J.Json.get_str(o, "text", "")}}])
    case 2n:
      Vo.text(thread, J.Json.get_str(o, "item", ""), String.trim(J.Json.get_str(o, "text", "")), ds)
    case 3n:
      Vo.msg(thread, "stopping", Nil{})
    case 4n:
      Vo.err(thread, J.Json.get_str(o, "error", "dictation failed"))
    case 5n:
      Vo.msg(thread, "level", [J.KV{"v", J.Num{Nat.show(J.Json.get_nat(o, "v", 0n))}}])
    case 6n:
      Vo.msg(thread, "hint", [J.KV{"hint", J.Str{J.Json.get_str(o, "hint", "")}}])
    case _:
      ""

def Vo.line.m(m: Maybe<&2, J.Json>, +thread: String, +ds: List<&2, String>) -> String:
  match m:
    case Some{+o}:
      Vo.line.k(C.Codec.pos(["ready", "delta", "done", "stopping", "error", "level", "hint"], J.Json.get_str(o, "event", "")), o, thread, ds)
    case None{}:
      ""

# one of the helper's lines as a message for the client ("" for none)
def Vo.line(+line: String, +thread: String, +ds: List<&2, String>) -> String:
  Vo.line.m(J.Json.parse(line), thread, ds)

def Vo.send(+out: Chan(String), +text: String) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, String, Unit> <- Chan.send(String, out, text)
    return Unit{}

def Vo.put(+text: String, +out: Chan(String)) -> IO(Unit):
  Bool.pick(IO(Unit), String.is_empty(text), IO.pure(Unit, Unit{}), Vo.send(out, text))

def Vo.say(ls: List<&2, String>, +out: Chan(String), +thread: String, +ds: List<&2, String>) -> IO(Unit):
  match ls:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case l <> t:
      do IO<Unit>:
        Vo.put(Vo.line(l, thread, ds), out)
        Vo.say(t, out, thread, ds)

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

def Vo.end(s: Socket, +pid: U32, +home: String, +out: Chan(String), +thread: String) -> IO(Unit):
  do IO<Unit>:
    Socket.close(s)
    code : U32 <- H.Proc.wait(pid)
    Vo.unpid(home, pid)
    Vo.send(out, Vo.msg(thread, "idle", Nil{}))

def Vo.fed(r: U.Lines & List<&2, String>, s: Socket, +out: Chan(String), +thread: String, +ds: List<&2, String>, k: Socket -> U.Lines -> IO(Unit)) -> IO(Unit):
  (carry, ls) = r
  do IO<Unit>:
    Vo.say(ls, out, thread, ds)
    k(s, carry)

def Vo.got(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, carry: U.Lines, +pid: U32, +home: String, +out: Chan(String), +thread: String,
    +ds: List<&2, String>, k: Socket -> U.Lines -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  match res:
    case Done{Nil{}}:
      Vo.end(s, pid, home, out, thread)
    case Done{bs}:
      Vo.fed(U.Lines.feed(carry, bs), s, out, thread, ds, k)
    case Fail{_}:
      Vo.end(s, pid, home, out, thread)

def Vo.reader(fuel: Nat, s: Socket, carry: U.Lines, +pid: U32, +home: String, +out: Chan(String), +thread: String, +ds: List<&2, String>) -> IO(Unit):
  match fuel:
    case 0n:
      Vo.end(s, pid, home, out, thread)
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536),
        r => Vo.got(r, carry, pid, home, out, thread, ds, s2 => c2 => Vo.reader(f, s2, c2, pid, home, out, thread, ds)))

def Vo.started.ok(started: U32 & Socket, +home: String, +out: Chan(String), +thread: String, +ds: List<&2, String>) -> IO(Unit):
  (+pid, s) = started
  do IO<Unit>:
    w : Bool <- Vo.write(Vo.pid(home), U32.show(pid))
    Vo.reader(Vo.forever(), s, U.Lines.new(), pid, home, out, thread, ds)

def Vo.fail(+out: Chan(String), +thread: String, +e: String) -> IO(Unit):
  do IO<Unit>:
    Vo.send(out, Vo.err(thread, e))
    Vo.send(out, Vo.msg(thread, "idle", Nil{}))

def Vo.started(r: Result<&1, &1, U32 & String, U32 & Socket>, +home: String, +out: Chan(String), +thread: String, +ds: List<&2, String>) -> IO(Unit):
  match r:
    case Done{started}:
      Vo.started.ok(started, home, out, thread, ds)
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

# the helper's arguments: a microphone named, else the default one
def Vo.live.args(+home: String, +device: String) -> List<&2, String>:
  List.append(&2, String, ["live", "--key-file", Vo.key(home), "--keywords-file", Vo.dict(home)], Bool.pick(List<&2, String>, String.is_empty(device), Nil{}, ["--device", device]))

def Vo.live.run(+exe: String, +home: String, +thread: String, +device: String, +out: Chan(String)) -> IO(Unit):
  do IO<Unit>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Vo.dir(home))
    +d : String <- BN.Bf.text(Vo.dict(home))
    r : Result<&1, &1, U32 & String, U32 & Socket> <- H.Proc.spawn(exe, Vo.live.args(home, device), "/", 0)
    Vo.started(r, home, out, thread, V.Voice.dict.parse(d))

def Vo.live.c(copy: Bool, +exe: String, +home: String, +thread: String, +device: String, +out: Chan(String)) -> IO(Unit):
  match copy:
    case True{}:
      Vo.fail(out, thread, "a hub on a copied log (BACKPLANE_COPY=1) starts no dictation")
    case False{}:
      Vo.live.run(exe, home, thread, device, out)

def Vo.live.go(+exe: String, +home: String, +thread: String, +device: String, +out: Chan(String)) -> IO(Unit):
  IO.bind(Bool, Unit, P.Proc.copy(), copy => Vo.live.c(copy, exe, home, thread, device, out))

def Vo.live.k(busy: Bool, keyed: Bool, +exe: String, +home: String, +thread: String, +device: String, +out: Chan(String)) -> IO(Unit):
  match busy keyed:
    case True{} _:
      Vo.fail(out, thread, "already listening: stop the other dictation first")
    case False{} False{}:
      Vo.fail(out, thread, "add your OpenAI API key in Settings, Voice")
    case False{} True{}:
      Vo.live.go(exe, home, thread, device, out)

# a live session for `thread`: the helper records the desktop's microphone
# (`device`, "" for the default) until it is stopped (Vo.stop); its
# messages go out on `out`, the last one {"ev":"idle"}
def Vo.live(+home: String, +thread: String, +device: String, +out: Chan(String)) -> IO(Unit):
  do IO<Unit>:
    +exe : String <- Vo.exe()
    pid : U32 <- Vo.alive(home)
    +k : String <- BN.Bf.text(Vo.key(home))
    Vo.live.k(U32.is_gt(pid, 0), Bool.not(String.is_empty(String.trim(k))), exe, home, thread, device, out)

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

def Vo.file.k(keyed: Bool, +exe: String, +home: String, +path: String, +ct: String) -> IO(String):
  match keyed:
    case True{}:
      do IO<String>:
        r : P.Ran <- P.Proc.run(exe, ["file", path, "--content-type", ct, "--key-file", Vo.key(home), "--keywords-file", Vo.dict(home)], "/")
        return Vo.file.text(r)
    case False{}:
      IO.pure(String, "")

# a recording's words ("" with no key, no helper or no words): `path` holds
# the audio, or a whole multipart form whose content type `ct` names
def Vo.file(+home: String, +path: String, +ct: String) -> IO(String):
  do IO<String>:
    +exe : String <- Vo.exe()
    +k : String <- BN.Bf.text(Vo.key(home))
    Vo.file.k(Bool.not(String.is_empty(String.trim(k))), exe, home, path, ct)

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

def Vo.ring.dict(+home: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    +d : String <- BN.Bf.text(Vo.dict(home))
    return V.Voice.dict.parse(d)

# A Pebble ring's form with its recording (`raw`, the body's bytes; `ct`
# its content type): the recording is transcribed here with the
# dictionary, and the form comes back with parts put before the ring's own
# (RG.Ring.part takes the first of a name): `transcription` the words
# heard, `unsure` the words to clarify (V.Voice.note), `heard` the model.
# With no key or no words, the form is as it came.
def Vo.ring.form(+body: String, +heard: String, +note: String) -> String:
  Bool.pick(String, String.is_empty(heard), body,
    "--backplane\r\nContent-Disposition: form-data; name=\"transcription\"\r\n\r\n" ++ heard ++
    "\r\n--backplane\r\nContent-Disposition: form-data; name=\"unsure\"\r\n\r\n" ++ note ++
    "\r\n--backplane\r\nContent-Disposition: form-data; name=\"heard\"\r\n\r\ngpt-live-transcribe\r\n" ++ body)

def Vo.ring.k(+home: String, +path: String, +ct: String, +body: String) -> IO(String):
  do IO<String>:
    +heard : String <- Vo.file(home, path, ct)
    BN.Bf.rm(path)
    ds : List<&2, String> <- Vo.ring.dict(home)
    return Vo.ring.form(body, heard, V.Voice.note(V.Voice.unsure(ds, Nil{}, heard)))

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
