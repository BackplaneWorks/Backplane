import Base
import ./host.bend as H
import ./proc.bend as P
import ./botnet.bend as BN
import ../core/json.bend as J
import ../core/utf8.bend as U
import ../core/http.bend as W
import ../core/far.bend as FR
import ../core/fview.bend as FV
import ../core/mech.bend as MC

# Viewers for threads elsewhere (IO; core/fview.bend, docs/bots.md
# "Viewers elsewhere").
#
# On the owning hub: view.ver lists the files the viewers want of a thread's
# folder (the project's KiCad files, its small models, and the files a
# client names) with a digest of the list, and when the client holds that
# digest already it waits for the list to change (at most a while); view.get
# is one file's bytes. Both only read, and only inside the folder.
#
# On the hub the client is on: Fvs.sync brings the files that changed into
# its cache (<home>/farfs/<link>/<the owner's folder>), a few at a time,
# and Fvs.mech does the same for a part listing's renders.

def Fvs.now() -> IO(Nat):
  do IO<Nat>:
    t : U32 <- H.Clock.epoch()
    return U32.to_nat(t)

# the longest the owner holds a view.ver that has nothing new, in seconds
def Fvs.wait() -> Nat:
  24n

# Replies
# -------

def Fvs.err(+why: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"0"}}, J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{why}}]))

def Fvs.ok(r: List<&2, J.KV>) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"0"}}, J.KV{"ok", J.Flag{True{}}}, J.KV{"result", J.Json.obj(r)}]))

def Fvs.json(+text: String) -> List<&2, U32>:
  W.Http.response("200 OK", "application/json", U.Utf8.encode(text), "")

# The owning hub
# --------------

# what the scripts share: the folder (argument 1), whether a path is a
# plain file inside it (a symlink, or a folder that leads out, is not),
# and stat in the machine's own dialect
def Fvs.lib() -> String:
  "cd \"$1\" 2>/dev/null || { echo 'no such folder'; exit 2; }\nrr=$(pwd -P)\nok() { [ -f \"./$1\" ] && [ ! -L \"./$1\" ] && d=$(cd \"$(dirname \"./$1\")\" 2>/dev/null && pwd -P) && case \"$d/\" in \"$rr\"/*) r=${d#\"$rr\"}; case \"$r/\" in */.*) return 1;; esac; return 0;; esac; return 1; }\nif stat -c %s . >/dev/null 2>&1; then SC=-c; SFMT='%n|%s|%.9Y'; else SC=-f; SFMT='%N|%z|%Fm'; fi\n"

# the list: the files asked for, the project's own choice, KiCad's files
# (four folders down), the small models (five), none hidden, none in a
# backup, at most 400, each "path<TAB>size<TAB>mtime" and none over the cap
def Fvs.list.sh() -> String:
  "mk() {\n  { printf '%s\\n' \"$W\" | while IFS= read -r f; do case \"$f\" in .backplane.json) ;; .*|*/.*|\"\") continue;; esac; ok \"$f\" && stat $SC \"$SFMT\" \"./$f\"; done\n"
  ++ "    [ -f ./.backplane.json ] && stat $SC \"$SFMT\" ./.backplane.json\n"
  ++ "    find . -maxdepth 4 \\( -path '*/.*' -o -name node_modules -o -name '*-backups' \\) -prune -o -type f \\( -iname '*.kicad_pcb' -o -iname '*.kicad_sch' -o -iname '*.kicad_pro' -o -name sym-lib-table -o -name fp-lib-table \\) ! -name '_autosave-*' -exec stat $SC \"$SFMT\" {} + 2>/dev/null | sort\n"
  ++ "    find . -maxdepth 5 \\( -path '*/.*' -o -name node_modules -o -name '*-backups' -o -path './mech' \\) -prune -o -type f \\( -iname '*.step' -o -iname '*.stp' -o -iname '*.wrl' -o -iname '*.glb' \\) -size -4096k -exec stat $SC \"$SFMT\" {} + 2>/dev/null | sort\n"
  ++ "  } | awk -F'|' '{n=split($0,a,\"|\"); nm=a[1]; for(i=2;i<n-1;i++) nm=nm \"|\" a[i]; sub(/^\\.\\//,\"\",nm); printf \"%s\\t%s\\t%s\\n\", nm, a[n-1], a[n]}' | awk -F'\\t' -v c=" ++ Nat.show(FV.Fv.cap()) ++ " '!seen[$1]++ && $2+0<=c' | head -n 400\n}\n"

def Fvs.ver.sh() -> String:
  Fvs.lib() ++ Fvs.list.sh() ++ "W=$4; V=$2; n=0\nwhile :; do\n  out=$(mk); v=$(printf '%s' \"$out\" | cksum | cut -d' ' -f1)\n  if [ \"$v\" != \"$V\" ] || [ \"$n\" -ge \"$3\" ]; then break; fi\n  sleep 2; n=$((n+2))\ndone\nprintf '%s\\n%s\\n' \"$v\" \"$out\"\n"

# one file: its size and where it really is
def Fvs.get.sh() -> String:
  Fvs.lib() ++ "case \"$2\" in .backplane.json) ;; .*|*/.*|\"\") echo 'no such file'; exit 1;; esac\nok \"$2\" || { echo 'no such file'; exit 1; }\ns=$(wc -c < \"./$2\" | tr -d ' ')\nd=$(cd \"$(dirname \"./$2\")\" && pwd -P)\nprintf '%s\\t%s' \"$s\" \"$d/$(basename \"$2\")\"\n"

def Fvs.ver.done(r: P.Ran, +root: String, +thread: String) -> List<&2, U32>:
  match r:
    case P.Ran{0, +out}:
      Fvs.json(Fvs.ok([J.KV{"root", J.Str{root}}, J.KV{"thread", J.Str{thread}}, J.KV{"ver", J.Str{String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(out)), ""))}},
        J.KV{"files", J.Str{String.join(List.drop(&2, String, String.lines(out), 1n), "\n")}}]))
    case P.Ran{_, _}:
      W.Http.text("404 Not Found", "no such folder")

# view.ver: the list of what the viewers want of the thread's folder; it
# answers at once when its digest is not `have`, else after the list
# changes or Fvs.wait() seconds
def Fvs.ver(+root: String, +thread: String, +want: String, +have: String) -> IO(List<&2, U32>):
  IO.bind(P.Ran, List<&2, U32>, P.Proc.run("sh", ["-c", Fvs.ver.sh(), "sh", root, have, Nat.show(Fvs.wait()), want], "/"), r => IO.pure(List<&2, U32>, Fvs.ver.done(r, root, thread)))

def Fvs.field(ps: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, ps, i), "")

def Fvs.nat(+s: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(String.trim(s)), 0n)

# A file to stream rather than bytes: the answer is a mark, a 0 byte (no
# HTTP response starts with one) then "<size>\t<path>", which the
# connection (server.bend's Conn.bot.answer) turns into the head and the
# file sent from disk a piece at a time (H.Sock.send_path). The bytes never
# pass through a Bend list or the hub's memory.
def Stream.mark(+size: Nat, +path: String) -> List<&2, U32>:
  0 <> U.Utf8.encode(Nat.show(size) ++ "\t" ++ path)

# a mark's size and path (its text after the 0 byte; a path may hold tabs)
def Stream.size(+text: String) -> Nat:
  Fvs.nat(Fvs.field(String.split(text, '\t'), 0n))

def Stream.path(+text: String) -> String:
  String.join(List.drop(&2, String, String.split(text, '\t'), 1n), "\t")

def Fvs.get.read.k(big: Bool, empty: Bool, +size: Nat, +path: String) -> IO(List<&2, U32>):
  match big empty:
    case True{} _:
      IO.pure(List<&2, U32>, W.Http.text("413 Payload Too Large", "too large to send"))
    case False{} True{}:
      IO.pure(List<&2, U32>, W.Http.response.c("200 OK", "application/octet-stream", Nil{}, "no-store", ""))
    case False{} False{}:
      IO.pure(List<&2, U32>, Stream.mark(size, path))

def Fvs.get.read(+size: Nat, +path: String) -> IO(List<&2, U32>):
  Fvs.get.read.k(Nat.is_gt(size, FV.Fv.cap()), Nat.is_eq(size, 0n), size, path)

def Fvs.get.done(r: P.Ran) -> IO(List<&2, U32>):
  match r:
    case P.Ran{0, +out}:
      Fvs.get.read(Fvs.nat(Fvs.field(String.split(out, '\t'), 0n)), String.trim(Fvs.field(String.split(out, '\t'), 1n)))
    case P.Ran{_, _}:
      IO.pure(List<&2, U32>, W.Http.text("404 Not Found", "no such file"))

# view.get: one file's bytes (a 404 when it is not a plain file inside the
# folder, a 413 over Fv.cap())
def Fvs.get(+root: String, +rel: String) -> IO(List<&2, U32>):
  IO.bind(P.Ran, List<&2, U32>, P.Proc.run("sh", ["-c", Fvs.get.sh(), "sh", root, rel], "/"), Fvs.get.done)

# The hub the client is on
# ------------------------

# a signed request to the owning hub; each carries a nonce, so two alike in
# one second are not taken for a replay
def Fvs.call(+home: String, +link: String, +url: String, +method: String, +p: List<&2, J.KV>, +out: String, +secs: Nat, +max: Nat) -> IO(BN.Resp):
  do IO<BN.Resp>:
    now : Nat <- Fvs.now()
    +n : String <- BN.Bn.random(4n)
    BN.Bn.to_peer.long(home, url, link, now, "/far/rpc",
      J.Json.show(J.Json.obj([J.KV{"id", J.Num{"0"}}, J.KV{"m", J.Str{method}}, J.KV{"p", J.Json.obj(List.append(&2, J.KV, p, [J.KV{"n", J.Str{n}}]))}])), out, secs, max)

def Fvs.code(r: BN.Resp) -> Nat:
  match r:
    case BN.Resp{c, _}:
      c

def Fvs.body(r: BN.Resp) -> String:
  match r:
    case BN.Resp{_, b}:
      b

# the answer for a request that did not come back 200
def Fvs.fail(r: BN.Resp) -> String:
  match r:
    case BN.Resp{+c, +b}:
      Fvs.err(Bool.pick(String, Nat.is_eq(c, 0n), "that machine is away", "that machine did not take it: " ++ String.take(b, 120n)))

def Fvs.lines.k(blank: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match blank:
    case True{}:
      rest
    case False{}:
      l <> rest

def Fvs.lines.go(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Fvs.lines.k(String.is_empty(String.trim(l)), l, Fvs.lines.go(t))

# the non-blank lines of a text
def Fvs.lines(+text: String) -> List<&2, String>:
  Fvs.lines.go(String.lines(text))

# the path a list line names (a line is "path<TAB>size<TAB>mtime")
def Fvs.rel(+line: String) -> String:
  Fvs.field(String.split(line, '\t'), 0n)

def Fvs.idx.go(ls: List<&2, String>, m: Map<&2, String>) -> Map<&2, String>:
  match ls:
    case Nil{}:
      m
    case +l <> t:
      Fvs.idx.go(t, Map.set(&2, String, m, Fvs.rel(l), l))

def Fvs.same.k(r: Map<&2, String> & String, +line: String) -> Bool:
  (_, v) = r
  String.eq(v, line)

# whether the map holds this very line (the same file, size and time)
def Fvs.same(+m: Map<&2, String>, +line: String) -> Bool:
  Fvs.same.k(Map.get(String, "", m, Fvs.rel(line)), line)

# what the cache holds: the lines of the list it was brought up to
def Fvs.idx(+cache: String) -> IO(Map<&2, String>):
  do IO<Map<&2, String>>:
    +t : String <- BN.Bf.text(cache ++ "/.farver")
    return Fvs.idx.go(Fvs.lines(t), Map.new(&2, String))

def Fvs.todo.k(need: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match need:
    case True{}:
      l <> rest
    case False{}:
      rest

# the lines the cache lacks (or holds as another version), safe paths only
def Fvs.todo(ls: List<&2, String>, +m: Map<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Fvs.todo.k(Bool.and(FV.Fv.file.ok(Fvs.rel(l)), Bool.not(Fvs.same(m, l))), l, Fvs.todo(t, m))

def Fvs.pull.kept(ok: Bool, +rel: String, +tmp: String) -> IO(String):
  match ok:
    case True{}:
      IO.pure(String, rel ++ "\t" ++ tmp)
    case False{}:
      IO.bind(P.Ran, String, P.Proc.run("rm", ["-f", tmp], "/"), r => IO.pure(String, ""))

def Fvs.pull(+home: String, +link: String, +url: String, +thread: String, +rel: String, +cache: String) -> IO(String):
  do IO<String>:
    +n : String <- BN.Bn.random(4n)
    +tmp : String = cache ++ "/.farpart-" ++ n
    r : BN.Resp <- Fvs.call(home, link, url, "view.get", [J.KV{"thread", J.Str{thread}}, J.KV{"path", J.Str{rel}}], tmp, FV.Fv.secs(), Nat.add(FV.Fv.cap(), 1048576n))
    Fvs.pull.kept(Nat.is_eq(Fvs.code(r), 200n), rel, tmp)

def Fvs.pair.k(blank: Bool, +p: String, rest: List<&2, String>) -> List<&2, String>:
  match blank:
    case True{}:
      rest
    case False{}:
      p <> rest

# a lane: each line's file in turn; the lines that came
def Fvs.lane(ls: List<&2, String>, +home: String, +link: String, +url: String, +thread: String, +cache: String) -> IO(List<&2, String>):
  match ls:
    case Nil{}:
      IO.pure(List<&2, String>, Nil{})
    case +l <> t:
      do IO<List<&2, String>>:
        +p : String <- Fvs.pull(home, link, url, thread, Fvs.rel(l), cache)
        rest : List<&2, String> <- Fvs.lane(t, home, link, url, thread, cache)
        return Fvs.pair.k(String.is_empty(p), p, rest)

def Fvs.lane.job(ls: List<&2, String>, +home: String, +link: String, +url: String, +thread: String, +cache: String, +out: Chan(List<&2, String>)) -> IO(Unit):
  do IO<Unit>:
    +r : List<&2, String> <- Fvs.lane(ls, home, link, url, thread, cache)
    ok : Result<&1, &1, List<&2, String>, Unit> <- Chan.send(List<&2, String>, out, r)
    return Unit{}

def Fvs.pick.k(mine: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match mine:
    case True{}:
      l <> rest
    case False{}:
      rest

# every fourth line from the kth
def Fvs.pick(ls: List<&2, String>, +k: Nat, +i: Nat) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Fvs.pick.k(Nat.is_eq(Nat.mod(i, 4n), k), l, Fvs.pick(t, k, 1n+i))

def Fvs.got(m: Maybe<&1, List<&2, String>>) -> List<&2, String>:
  match m:
    case Some{+xs}:
      xs
    case None{}:
      Nil{}

# the files in four lanes at once; the lines that came
def Fvs.fetch(+todo: List<&2, String>, +home: String, +link: String, +url: String, +thread: String, +cache: String) -> IO(List<&2, String>):
  do IO<List<&2, String>>:
    +out : Chan(List<&2, String>) <- Chan.new(List<&2, String>, 4)
    IO.spawn(Unit, Fvs.lane.job(Fvs.pick(todo, 1n, 0n), home, link, url, thread, cache, out))
    IO.spawn(Unit, Fvs.lane.job(Fvs.pick(todo, 2n, 0n), home, link, url, thread, cache, out))
    IO.spawn(Unit, Fvs.lane.job(Fvs.pick(todo, 3n, 0n), home, link, url, thread, cache, out))
    +a : List<&2, String> <- Fvs.lane(Fvs.pick(todo, 0n, 0n), home, link, url, thread, cache)
    b : Maybe<&1, List<&2, String>> <- Chan.recv(List<&2, String>, out)
    c : Maybe<&1, List<&2, String>> <- Chan.recv(List<&2, String>, out)
    d : Maybe<&1, List<&2, String>> <- Chan.recv(List<&2, String>, out)
    return List.append(&2, String, a, List.append(&2, String, Fvs.got(b), List.append(&2, String, Fvs.got(c), Fvs.got(d))))

def Fvs.at.k(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

def Fvs.at(+m: Map<&2, String>, +rel: String) -> String:
  Fvs.at.k(Map.get(String, "", m, rel))

def Fvs.ready.k(has: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match has:
    case True{}:
      l <> rest
    case False{}:
      rest

# the wanted paths the cache holds
def Fvs.ready(ls: List<&2, String>, +m: Map<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Fvs.ready.k(Bool.not(String.is_empty(Fvs.rel(Fvs.at(m, l)))), l, Fvs.ready(t, m))

# the lock a cache's steps take one at a time (a folder, as macOS has no
# flock; one left by a killed step is taken after 15 s)
def Fvs.lock.sh() -> String:
  "c=$1; mkdir -p \"$c\"; n=0\nwhile ! mkdir \"$c/.farlock\" 2>/dev/null; do n=$((n+1)); if [ $n -ge 150 ]; then rm -rf \"$c/.farlock\"; mkdir \"$c/.farlock\" 2>/dev/null; break; fi; sleep 0.1; done\n"
  ++ "trap 'rmdir \"$c/.farlock\" 2>/dev/null' EXIT\nT=$(printf '\\t')\n"

# A sync round takes a number as its list arrives (Fvs.ticket), so the
# rounds of one cache are ordered by when their lists were made, whatever
# the files' times say.
def Fvs.ticket.sh() -> String:
  Fvs.lock.sh() ++ "k=$(cat \"$c/.farnext\" 2>/dev/null || echo 0); k=$((k+1)); echo \"$k\" > \"$c/.farnext\"; echo \"$k\"\n"

def Fvs.ticket.out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, +out}:
      String.trim(out)

def Fvs.ticket(+cache: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", Fvs.ticket.sh(), "sh", cache], "/"), r => IO.pure(String, Fvs.ticket.out(r)))

# Installing what came: one step at a time per cache. A round older than
# the last installed one (its ticket, $4, is lower) installs nothing, so
# two syncs racing never roll a file back or delete what a newer one
# brought; a KiCad
# file the owner no longer lists is removed (every list names all of them:
# a model or a file a client named stays, as another client's list need
# not name it). Prints the lines
# the cache now holds (its new .farver).
def Fvs.install.sh() -> String:
  Fvs.lock.sh()
  ++ "last=$(cat \"$c/.fargen\" 2>/dev/null || echo 0)\nif [ \"$4\" -lt \"$last\" ]; then printf '%s\\n' \"$3\" | cut -f2 | while IFS= read -r tmp; do [ -n \"$tmp\" ] && rm -f \"$tmp\"; done; cat \"$c/.farver\" 2>/dev/null; exit 0; fi\necho \"$4\" > \"$c/.fargen\"\n"
  ++ "{ echo '#'; printf '%s\\n' \"$2\"; } > \"$c/.farnew\"; touch \"$c/.farver\"; echo '#' > \"$c/.farins\"\n"
  ++ "printf '%s\\n' \"$3\" | while IFS=\"$T\" read -r rel tmp; do\n  [ -n \"$rel\" ] || continue\n"
  ++ "  mkdir -p \"$(dirname \"$c/$rel\")\" && mv -f \"$tmp\" \"$c/$rel\" && printf '%s\\n' \"$rel\" >> \"$c/.farins\"\ndone\n"
  ++ "{ echo '#'; cat \"$c/.farver\"; } > \"$c/.farold\"\n"
  ++ "awk -F'\\t' 'FNR==1{f++; next} f==1{I[$1]=1; next} f==2{N[$1]=$0; next} f==3{if($1!=\"\") O[$1]=$0; next}\n"
  ++ "  END{for(r in I) if(r in N) print N[r]; for(r in O) if(!(r in I)){ l=tolower(r); if(r in N) print O[r]; else if(l ~ /\\.(kicad_pcb|kicad_sch|kicad_pro)$/ || l ~ /(^|\\/)(sym-lib-table|fp-lib-table)$/) print \"DEL\" FS r; else print O[r]}}' \"$c/.farins\" \"$c/.farnew\" \"$c/.farold\" > \"$c/.farmid\"\n"
  ++ "grep \"^DEL$T\" \"$c/.farmid\" | cut -f2- | while IFS= read -r rel; do rm -f \"$c/$rel\"; done\n"
  ++ "grep -v \"^DEL$T\" \"$c/.farmid\" > \"$c/.farver.tmp\"; mv -f \"$c/.farver.tmp\" \"$c/.farver\"; rm -f \"$c/.farmid\" \"$c/.farold\" \"$c/.farnew\" \"$c/.farins\"\ncat \"$c/.farver\"\n"

def Fvs.install.out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, +out}:
      out

def Fvs.install(+cache: String, +files: String, +got: List<&2, String>, +ticket: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", Fvs.install.sh(), "sh", cache, files, String.join(got, "\n"), ticket], "/"), r => IO.pure(String, Fvs.install.out(r)))

# (a sync that missed a file says so with no version, so the client asks
# again at once and the next sync brings what is missing; it waits a
# little first, so a machine that keeps failing is not asked in a loop)
def Fvs.sync.ver(+failed: Nat, +ver: String) -> String:
  Bool.pick(String, Nat.is_eq(failed, 0n), ver, "")

def Fvs.sync.pause(+failed: Nat) -> IO(Unit):
  Bool.pick(IO(Unit), Nat.is_eq(failed, 0n), IO.pure(Unit, Unit{}), IO.sleep(3000))

# far.view: the thread's files the viewers want, kept in the cache. The
# owner's list is asked for with the digest this client holds; when that
# changes (or after a while: same) the files that differ are brought in
# and the answer says what the cache holds now, for the client to go on.
def Fvs.sync.done(+home: String, +link: String, +thread: String, +cache: String, +want: String, +ver: String, +files: String, +old: Map<&2, String>, +todo: List<&2, String>, +got: List<&2, String>, +ticket: String) -> IO(String):
  do IO<String>:
    +now : String <- Fvs.install(cache, files, got, ticket)
    +keep : List<&2, String> = Fvs.lines(now)
    +failed : Nat = Nat.sub(List.length(&2, String, todo), List.length(&2, String, got))
    Fvs.sync.pause(failed)
    return Fvs.ok([J.KV{"farView", J.Flag{True{}}}, J.KV{"thread", J.Str{FR.Fr.id(link, thread)}}, J.KV{"root", J.Str{cache}}, J.KV{"ver", J.Str{Fvs.sync.ver(failed, ver)}}, J.KV{"same", J.Flag{False{}}},
      J.KV{"want", J.Str{want}}, J.KV{"ready", J.Str{String.join(Fvs.ready(Fvs.lines(want), Fvs.idx.go(keep, Map.new(&2, String))), "\n")}},
      J.KV{"changed", J.Json.nat(List.length(&2, String, got))}, J.KV{"failed", J.Json.nat(failed)}])

def Fvs.sync.fetch(+home: String, +link: String, +url: String, +thread: String, +cache: String, +want: String, +ver: String, +files: String) -> IO(String):
  do IO<String>:
    +old : Map<&2, String> <- Fvs.idx(cache)
    +todo : List<&2, String> = Fvs.todo(Fvs.lines(files), old)
    +ticket : String <- Fvs.ticket(cache)
    +got : List<&2, String> <- Fvs.fetch(todo, home, link, url, thread, cache)
    Fvs.sync.done(home, link, thread, cache, want, ver, files, old, todo, got, ticket)

def Fvs.sync.at(+cache: String, +ver: String, +files: String, +home: String, +link: String, +url: String, +thread: String, +want: String, +have: String) -> IO(String):
  Bool.pick(IO(String), String.is_empty(cache), IO.pure(String, Fvs.err("that folder cannot be kept here")),
    Bool.pick(IO(String), Bool.and(String.eq(ver, have), Bool.not(String.is_empty(have))),
      IO.pure(String, Fvs.ok([J.KV{"farView", J.Flag{True{}}}, J.KV{"thread", J.Str{FR.Fr.id(link, thread)}}, J.KV{"root", J.Str{cache}}, J.KV{"ver", J.Str{ver}}, J.KV{"same", J.Flag{True{}}}, J.KV{"want", J.Str{want}}])),
      Fvs.sync.fetch(home, link, url, thread, cache, want, ver, files)))

def Fvs.sync.res(+res: J.Json, +home: String, +link: String, +url: String, +thread: String, +want: String, +have: String) -> IO(String):
  Fvs.sync.at(FV.Fv.cache(home, link, J.Json.get_str(res, "root", "")), J.Json.get_str(res, "ver", ""), J.Json.get_str(res, "files", ""), home, link, url, thread, want, have)

def Fvs.sync.body(m: Maybe<&2, J.Json>, +home: String, +link: String, +url: String, +thread: String, +want: String, +have: String) -> IO(String):
  match m:
    case Some{+o}:
      Bool.pick(IO(String), J.Json.get_bool(o, "ok", False{}), Fvs.sync.res(J.Json.get_obj(o, "result"), home, link, url, thread, want, have),
        IO.pure(String, Fvs.err(J.Json.get_str(o, "error", "that machine did not take it"))))
    case None{}:
      IO.pure(String, Fvs.err("invalid reply from that machine"))

def Fvs.sync.got(+r: BN.Resp, +home: String, +link: String, +url: String, +thread: String, +want: String, +have: String) -> IO(String):
  Bool.pick(IO(String), Nat.is_eq(Fvs.code(r), 200n), Fvs.sync.body(J.Json.parse(Fvs.body(r)), home, link, url, thread, want, have), IO.pure(String, Fvs.fail(r)))

def Fvs.sync.ask(+home: String, +link: String, +url: String, +thread: String, +want: String, +have: String) -> IO(String):
  IO.bind(BN.Resp, String, Fvs.call(home, link, url, "view.ver", [J.KV{"thread", J.Str{thread}}, J.KV{"want", J.Str{want}}, J.KV{"have", J.Str{have}}], "", 40n, 1048576n),
    r => Fvs.sync.got(r, home, link, url, thread, want, have))

# A sync that failed (the machine away, an older hub that does not know
# view.ver, a folder it cannot keep) still answers as far.view, saying
# down, so the client's ask is over and it asks again later (Fv.due).
def Fvs.down(+link: String, +thread: String, +want: String, +why: String) -> String:
  Fvs.ok([J.KV{"farView", J.Flag{True{}}}, J.KV{"thread", J.Str{FR.Fr.id(link, thread)}}, J.KV{"want", J.Str{want}}, J.KV{"down", J.Str{"1"}}, J.KV{"error", J.Str{why}}])

def Fvs.sync.out(m: Maybe<&2, J.Json>, +text: String, +link: String, +thread: String, +want: String) -> String:
  match m:
    case Some{+o}:
      Bool.pick(String, J.Json.get_bool(o, "ok", False{}), text, Fvs.down(link, thread, want, J.Json.get_str(o, "error", "that machine did not take it")))
    case None{}:
      Fvs.down(link, thread, want, "invalid reply from that machine")

def Fvs.sync.text(+t: String, +link: String, +thread: String, +want: String) -> String:
  Fvs.sync.out(J.Json.parse(t), t, link, thread, want)

def Fvs.sync(+home: String, +link: String, +url: String, +thread: String, +want: String, +have: String) -> IO(String):
  IO.bind(String, String, Fvs.sync.ask(home, link, url, thread, want, have), t => IO.pure(String, Fvs.sync.text(t, link, thread, want)))

# mech.list: the owner's part listing, with its renders brought into the
# cache first, so a picture is there when a client asks for it
def Fvs.shots.k(png: Bool, +p: String, rest: List<&2, String>) -> List<&2, String>:
  match png:
    case True{}:
      p <> rest
    case False{}:
      rest

def Fvs.shots(fs: List<&2, MC.MFile>) -> List<&2, String>:
  match fs:
    case Nil{}:
      Nil{}
    case MC.MFile{+p, _} <> t:
      Fvs.shots.k(Bool.and(String.ends_with(p, ".png"), FV.Fv.file.ok(p)), p, Fvs.shots(t))

def Fvs.mech.shots(+text: String, +home: String, +link: String, +url: String, +thread: String, +body: String) -> IO(String):
  do IO<String>:
    s : String <- Fvs.sync(home, link, url, thread, String.join(Fvs.shots(MC.Mech.files(text)), "\n"), "")
    return FV.Fv.fix(home, link, body)

def Fvs.mech.got(m: Maybe<&2, J.Json>, +home: String, +link: String, +url: String, +thread: String, +body: String) -> IO(String):
  match m:
    case Some{+o}:
      Fvs.mech.shots(J.Json.get_str(J.Json.get_obj(o, "result"), "mech", ""), home, link, url, thread, body)
    case None{}:
      IO.pure(String, Fvs.err("invalid reply from that machine"))

def Fvs.mech.r(+r: BN.Resp, +home: String, +link: String, +url: String, +thread: String) -> IO(String):
  Bool.pick(IO(String), Nat.is_eq(Fvs.code(r), 200n), Fvs.mech.got(J.Json.parse(Fvs.body(r)), home, link, url, thread, Fvs.body(r)), IO.pure(String, Fvs.fail(r)))

def Fvs.mech(+home: String, +link: String, +url: String, +thread: String) -> IO(String):
  IO.bind(BN.Resp, String, Fvs.call(home, link, url, "mech.list", [J.KV{"thread", J.Str{thread}}], "", 30n, 4194304n), r => Fvs.mech.r(r, home, link, url, thread))
