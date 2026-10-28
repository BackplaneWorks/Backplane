import Base
import ../core/json.bend as J
import ../core/order.bend as OR
import ./google.bend as GS
import ./proc.bend as P
import ./host.bend as H
import ./kicad.bend as K

# Ordering, the IO (core/order.bend; docs/order.md)
# ==================================================
# The order server is reached with curl, its URL, headers and form lines
# in a config file (google.bend's Gc.*), never on the command line. This
# install's id (a random word made once, <home>/secrets/install-id) goes
# with every request: the server's limits count it, nothing else does.

def Od.tmp(+home: String) -> String:
  home ++ "/secrets"

# made before curl writes there, 0700 like the API keys' (botnet.bend's
# Bf.dir): a fresh home has none until something needs it
def Od.tmp.made(+home: String) -> IO(Unit):
  do IO<Unit>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Od.tmp(home))
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(Od.tmp(home), 448)
    return Unit{}

def Od.install.k(+old: String, +fresh: String, +path: String) -> IO(String):
  Bool.pick(IO(String), Nat.is_ge(String.length(String.trim(old)), 16n), IO.pure(String, String.trim(old)),
    IO.bind(Unit, String, GS.Gf.write(path, fresh), _ => IO.pure(String, fresh)))

def Od.install(+home: String) -> IO(String):
  do IO<String>:
    Od.tmp.made(home)
    old : String <- GS.Gf.text(Od.tmp(home) ++ "/install-id")
    +fresh : String <- GS.Gf.nonce(6n)
    Od.install.k(old, fresh, Od.tmp(home) ++ "/install-id")

# the order server's address: Backplane's own (order.bend's Order.server)
# unless the setting order.server replaces it. BACKPLANE_ORDER_SERVER
# replaces Backplane's own: every test hub sets it to a dead local address,
# so no test ever reaches the real server (with Ordering on by default); a
# test that orders sets order.server to its own stand-in
def Od.at.k(+server: String, +env: String) -> String:
  Bool.pick(String, Bool.and(String.eq(server, OR.Order.server()), Bool.not(String.is_empty(env))), env, server)

def Od.at(+server: String) -> IO(String):
  IO.bind(String, String, GS.Gf.env("BACKPLANE_ORDER_SERVER", ""), e => IO.pure(String, Od.at.k(server, e)))

def Od.heads(+install: String) -> List<&2, String>:
  ["X-Backplane-Install: " ++ install, "Accept: application/json"]

# The project card
# ----------------
# fab.look: the intent, each *.fab.json found (not under extra/ or mfg/,
# no hidden folders), and, with estimates on, the server's "from" price of
# the bare parts.

def Od.find() -> String:
  "find . -maxdepth 5 -name '*.fab.json' -not -path './extra/*' -not -path './mfg/*' -not -path '*/.*' -not -path '*/node_modules/*' 2>/dev/null | LC_ALL=C sort | head -n 40 | sed 's|^\\./||'"

def Od.ran(r: P.Ran) -> String:
  match r:
    case P.Ran{0, out}:
      out
    case P.Ran{_, _}:
      ""

def Od.line.k(blank: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match blank:
    case True{}:
      rest
    case False{}:
      String.trim(l) <> rest

def Od.lines(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Od.line.k(String.is_empty(String.trim(l)), l, Od.lines(t))

def Od.read(ps: List<&2, String>, +cwd: String) -> IO(List<&2, OR.FabFile>):
  match ps:
    case Nil{}:
      IO.pure(List<&2, OR.FabFile>, Nil{})
    case +p <> t:
      do IO<List<&2, OR.FabFile>>:
        text : String <- GS.Gf.text(cwd ++ "/" ++ p)
        rest : List<&2, OR.FabFile> <- Od.read(t, cwd)
        return OR.FabFile{p, text} <> rest

# a part as the card lists it: name, kind and where its fab.json is
def Od.part.row(+p: J.Json) -> J.Json:
  J.Json.obj([J.KV{"name", J.Str{J.Json.get_str(p, "name", "")}}, J.KV{"kind", J.Str{J.Json.get_str(p, "kind", "")}}, J.KV{"path", J.Str{J.Json.get_str(p, "path", "")}}])

def Od.part.rows(ps: List<&2, J.Json>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Od.part.row(p) <> Od.part.rows(t)

# what the server said, as the card's fields: the price's label, or why there is none
def Od.est.got(r: GS.GResp) -> J.Json:
  match r:
    case GS.GResp{+code, +body, +err}:
      Bool.pick(J.Json, GS.Gc.ok(code), Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}),
        J.Json.obj([J.KV{"error", J.Str{Bool.pick(String, String.eq(code, "000"), "the order server is not reachable" ++ Bool.pick(String, String.is_empty(err), "", ": " ++ err),
          J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "error", "the order server said " ++ code))}}]))

def Od.est.k(ask: Bool, +home: String, +server: String, +install: String, +body: String) -> IO(J.Json):
  match ask:
    case True{}:
      do IO<J.Json>:
        +url : String <- Od.at(server)
        r : GS.GResp <- GS.Gc.req(Od.tmp(home), url ++ "/v1/estimate", ["Content-Type: application/json", "X-Backplane-Install: " ++ install], body)
        return Od.est.got(r)
    case False{}:
      IO.pure(J.Json, J.Null{})

# the card's answer: {thread, intent, goal, parts, boards, label, error, estimates}
def Od.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      J.Str{x} <> Od.strs(t)

# board_files: every KiCad board in the project (paths from its folder),
# for the viewer's board picker
def Od.card(+thread: String, +intent: String, ps: List<&2, J.Json>, +boards: String, +est: J.Json, +on: Bool, bs: List<&2, String>, +bd: String, +ord: String) -> J.Json:
  J.Json.obj([J.KV{"thread", J.Str{thread}}, J.KV{"intent", J.Str{intent}}, J.KV{"goal", J.Str{OR.Intent.goal(intent)}},
    J.KV{"parts", J.Json.arr(Od.part.rows(ps))}, J.KV{"boards", J.Num{boards}}, J.KV{"label", J.Str{J.Json.get_str(est, "label", "")}},
    J.KV{"error", J.Str{J.Json.get_str(est, "error", "")}}, J.KV{"estimates", J.Flag{on}}, J.KV{"board_files", J.Json.arr(Od.strs(bs))}, J.KV{"board_default", J.Str{bd}}, J.KV{"order", J.Str{ord}}])

# the board the viewer opens by default (kicad.bend's Files.read), from the folder
def Od.pcb(+cwd: String, f: K.Files) -> String:
  match f:
    case K.Files{_, +pcb, _, _}:
      Bool.pick(String, String.starts_with(pcb, cwd ++ "/"), String.drop(pcb, 1n+String.length(cwd)), "")

# KiCad boards and project files in the project (paths from its folder, in
# name order; hidden folders, extra/, mfg/, node_modules and backups left
# out): order.bend's Boards.pick keeps the boards the picker offers, and
# their count is what a card with no fab files yet goes by
def Od.boards() -> String:
  "find . -maxdepth 5 -type d \\( -name '.?*' -o -name node_modules -o -path ./extra -o -path ./mfg \\) -prune -o -type f \\( -name '*.kicad_pcb' -o -name '*.kicad_pro' \\) -not -name '_autosave-*' -not -name '*backup*' -print 2>/dev/null | LC_ALL=C sort | head -n 2000 | sed 's|^\\./||'"

# with no fab files but a board, the server is only asked whether it is there
# (curl's files go in the secrets folder: without it, in a fresh home, the
# card had no Buy it)
def Od.health.k(ask: Bool, +home: String, +server: String) -> IO(J.Json):
  match ask:
    case True{}:
      do IO<J.Json>:
        Od.tmp.made(home)
        +url : String <- Od.at(server)
        r : GS.GResp <- GS.Gc.req(Od.tmp(home), url ++ "/v1/health", ["Accept: application/json"], "")
        return Od.est.got(r)
    case False{}:
      IO.pure(J.Json, J.Null{})

def Od.look.ps(ps: List<&2, J.Json>, +home: String, +cwd: String, +thread: String, +intent: String, +on: Bool, +server: String, +bs: List<&2, String>, +bd: String, +ord: String) -> IO(J.Json):
  match ps:
    case Nil{}:
      do IO<J.Json>:
        +n : String = Nat.show(List.length(&2, String, bs))
        est : J.Json <- Od.health.k(Bool.and(on, Bool.not(String.eq(n, "0"))), home, server)
        return Od.card(thread, intent, Nil{}, n, est, on, bs, bd, ord)
    case +p <> +t:
      do IO<J.Json>:
        +install : String <- Od.install(home)
        est : J.Json <- Od.est.k(on, home, server, install, OR.Order.est.body(p <> t))
        return Od.card(thread, intent, p <> t, "0", est, on, bs, bd, ord)

# where the first newline is (the length when there is none)
def Od.nl(s: String) -> Nat:
  match s:
    case SNil{}:
      0n
    case SCon{Chr{+c}, t}:
      Bool.pick(Nat, U32.is_eq(c, 10), 0n, 1n+Od.nl(t))

# the n-th line of a few ("" past the end)
def Od.nth(ls: List<&2, String>, +n: Nat) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> t:
      Bool.pick(String, Nat.is_eq(n, 0n), String.trim(l), Od.nth(t, Nat.sub(n, 1n)))

# (only thread ids' characters: letters, digits and dashes)
def Od.id.ok.go(+x: String) -> Bool:
  match x:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Bool.or(Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))), Bool.or(U32.is_eq(c, 45), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)))), Od.id.ok.go(t))

def Od.id.ok(+x: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(x)), Od.id.ok.go(x))

def Od.ids.k(ok: Bool, +x: String, rest: List<&2, String>) -> List<&2, String>:
  match ok:
    case True{}:
      x <> rest
    case False{}:
      rest

def Od.ids(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Od.ids.k(Od.id.ok(x), x, Od.ids(t))

# the newest paid order of the project's buy threads (ids from hub.bend's
# Buy.of, comma-separated): its status, "" for none. A newer quote not paid
# for yet does not hide it
def Od.order.sh(+ids: String) -> String:
  "for f in $(ls -t q_*.json 2>/dev/null); do if grep -qE '\"thread\":\"(" ++ String.join(Od.ids(String.split(ids, ',')), "|") ++ ")\"' \"$f\"; then if grep -qE '\"status\":\"(paid|placed|shipped|delivered)\"' \"$f\"; then cat \"$f\"; break; fi; fi; done"

# Manufacturing reviews (order_review)
# ------------------------------------
# The design's fingerprint: the names and contents of every board,
# schematic, model and fab file outside mfg/, extra/ and .backplane/,
# hashed in name order (sha256sum, or shasum on a Mac)
def Od.print.sh() -> String:
  "find . \\( -path ./mfg -o -path ./extra -o -path ./.git -o -path ./.backplane -o -name node_modules -o -name '*backup*' \\) -prune -o -type f -not -name '_autosave-*' \\( -name '*.kicad_pcb' -o -name '*.kicad_sch' -o -name '*.step' -o -name '*.stp' -o -name '*.dxf' -o -name '*.fab.json' \\) -print 2>/dev/null | LC_ALL=C sort | while IFS= read -r f; do printf '%s\\n' \"$f\"; cat \"$f\"; done | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-32"

def Od.print(+cwd: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", Od.print.sh()], cwd), r => IO.pure(String, String.trim(Od.ran(r))))

def Od.review.k(record: Bool, +cwd: String, +fp: String, +args: J.Json, +now: Nat) -> IO(J.Json):
  match record:
    case True{}:
      do IO<J.Json>:
        mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(cwd ++ "/.backplane")
        GS.Gf.write(cwd ++ "/.backplane/review.json", J.Json.show(OR.Order.review.rec.with(fp, J.Json.get_str(args, "verdict", ""), J.Json.get_str(args, "summary", ""), J.Json.elems(J.Json.get_obj(args, "issues")), now)) ++ "\n")
        m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(cwd ++ "/.backplane/review.json", 420)
        return J.Json.obj([J.KV{"recorded", J.Flag{True{}}}, J.KV{"verdict", J.Str{OR.Order.review.verdict(J.Json.get_str(args, "verdict", ""))}}])
    case False{}:
      IO.bind(String, J.Json, GS.Gf.text(cwd ++ "/.backplane/review.json"),
        t => IO.pure(J.Json, OR.Order.review.said(fp, Maybe.default(&2, J.Json, J.Json.parse(t), J.Null{}))))

# order_review {action: check | record, verdict, summary, issues} in a thread's folder
def Od.review(+cwd: String, +args: J.Json, +now: Nat) -> IO(J.Json):
  IO.bind(String, J.Json, Od.print(cwd), fp => Od.review.k(String.eq(J.Json.get_str(args, "action", "check"), "record"), cwd, fp, args, now))

# Quotes
# ------
# order_quote: the manifest's files (paths inside the order's folder, plain
# characters only), sent as one multipart request; then the quote is
# polled until it is worked out. The token stays with the hub: in
# <home>/orders/<quote>.json (0600) and in the Buy now link it posts.

def Od.safe.c(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 32), Bool.not(Bool.or(Bool.or(U32.is_eq(c, 34), U32.is_eq(c, 92)), Bool.or(U32.is_eq(c, 59), U32.is_eq(c, 127)))))

def Od.safe.chars(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Od.safe.c(c), Od.safe.chars(t))

# a path from the order's folder that stays inside it
def Od.safe(+p: String) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(p)), Bool.not(String.starts_with(p, "/"))),
    Bool.and(Bool.not(String.contains(p, "..")), Od.safe.chars(p)))

def Od.keys(fs: J.Json) -> List<&2, String>:
  match fs:
    case J.Field{+k, _, t}:
      k <> Od.keys(t)
    case _:
      Nil{}

def Od.files.k(o: J.Json) -> List<&2, String>:
  match o:
    case J.Obj{fs}:
      Od.keys(fs)
    case _:
      Nil{}

# the files a manifest lists ("files": {path: sha256})
def Od.files(+m: J.Json) -> List<&2, String>:
  Od.files.k(J.Json.get_obj(m, "files"))

def Od.all.safe(ps: List<&2, String>) -> Bool:
  match ps:
    case Nil{}:
      True{}
    case +p <> t:
      Bool.and(Od.safe(p), Od.all.safe(t))

def Od.forms(ps: List<&2, String>, +dir: String) -> String:
  match ps:
    case Nil{}:
      ""
    case +p <> t:
      "form = " ++ GS.Gc.q("file=@\"" ++ p ++ "\";filename=\"" ++ p ++ "\"") ++ "\n" ++ Od.forms(t, dir)

def Od.upload.cfg(+url: String, +install: String, +dir: String, ps: List<&2, String>, +out: String) -> String:
  "url = " ++ GS.Gc.q(url) ++ "\n" ++ GS.Gc.headers(Od.heads(install)) ++ "form = " ++ GS.Gc.q("manifest=<manifest.json") ++ "\n"
    ++ Od.forms(ps, dir) ++ "output = " ++ GS.Gc.q(out) ++ "\n" ++ "write-out = \"%{http_code}\"\n"

# the server's answer to an upload: {id, token} or {error} (curl runs in the
# order's folder and names every file from there, by the checked relative paths)
def Od.upload(+home: String, +server: String, +install: String, +dir: String, ps: List<&2, String>) -> IO(J.Json):
  do IO<J.Json>:
    +name : String <- GS.Gf.nonce(3n)
    +cfg : String = Od.tmp(home) ++ "/o-" ++ name ++ ".curl"
    +out : String = Od.tmp(home) ++ "/o-" ++ name ++ ".out"
    +url : String <- Od.at(server)
    GS.Gf.write(cfg, Od.upload.cfg(url ++ "/v1/quotes", install, dir, ps, out))
    r : P.Ran <- P.Proc.run("curl", ["-sS", "--max-time", "300", "-K", cfg], dir)
    +said : String = GS.Gc.ran(r)
    text : String <- GS.Gf.text(out)
    GS.Gf.rm(cfg)
    GS.Gf.rm(out)
    return Od.est.got(GS.GResp{GS.Gc.code(said), text, GS.Gc.said(said)})

def Od.get(+home: String, +server: String, +install: String, +qid: String, +token: String) -> IO(J.Json):
  do IO<J.Json>:
    +url : String <- Od.at(server)
    r : GS.GResp <- GS.Gc.req(Od.tmp(home), url ++ "/v1/quotes/" ++ qid, ["Authorization: Bearer " ++ token, "X-Backplane-Install: " ++ install], "")
    return Od.est.got(r)

def Od.pending(+q: J.Json) -> Bool:
  Bool.or(String.eq(J.Json.get_str(q, "status", ""), "received"), String.eq(J.Json.get_str(q, "status", ""), "analysing"))

def Od.poll.k(+q: J.Json, again: Unit -> IO(J.Json)) -> IO(J.Json):
  Bool.pick(IO(J.Json), Od.pending(q), again(Unit{}), IO.pure(J.Json, q))

# at most n polls, 3 s apart
def Od.poll(n: Nat, +home: String, +server: String, +install: String, +qid: String, +token: String) -> IO(J.Json):
  match n:
    case 0n:
      IO.pure(J.Json, J.Json.obj([J.KV{"error", J.Str{"the quote is taking long; ask order_status in a few minutes"}}]))
    case 1n+m:
      IO.bind(Unit, J.Json, IO.sleep(3000), _ =>
        IO.bind(J.Json, J.Json, Od.get(home, server, install, qid, token), q => Od.poll.k(q, u => Od.poll(m, home, server, install, qid, token))))

# the record a quote leaves: what the hub needs to follow it
def Od.record(+qid: String, +token: String, +thread: String, +server: String, +status: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"id", J.Str{qid}}, J.KV{"token", J.Str{token}}, J.KV{"thread", J.Str{thread}}, J.KV{"server", J.Str{server}}, J.KV{"status", J.Str{status}}]))

def Od.orders(+home: String) -> String:
  home ++ "/orders"

def Od.order.k(none: Bool, +home: String, +ids: String) -> IO(String):
  match none:
    case True{}:
      IO.pure(String, "")
    case False{}:
      IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", Od.order.sh(ids)], Od.orders(home)),
        r => IO.pure(String, J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(Od.ran(r)), J.Null{}), "status", "")))

def Od.order(+home: String, +ids: String) -> IO(String):
  Od.order.k(List.is_empty(&2, String, Od.ids(String.split(ids, ','))), home, ids)

# opts: "on|off\n<server>\n<the project's buy threads>" (hub.bend's Buy.look)
def Od.look(+home: String, +cwd: String, +thread: String, +opts: String) -> IO(J.Json):
  do IO<J.Json>:
    intent : String <- GS.Gf.text(cwd ++ "/.backplane/intent.md")
    r : P.Ran <- P.Proc.run("sh", ["-c", Od.find()], cwd)
    fs : List<&2, OR.FabFile> <- Od.read(Od.lines(String.lines(Od.ran(r))), cwd)
    b : P.Ran <- P.Proc.run("sh", ["-c", Od.boards()], cwd)
    kf : K.Files <- K.Files.read(cwd)
    +bd : String = Od.pcb(cwd, kf)
    st : String <- Od.order(home, Od.nth(String.lines(opts), 2n))
    Od.look.ps(OR.Order.parts(fs), home, cwd, thread, String.trim(intent), Bool.not(String.starts_with(opts, "off")),
      Od.nth(String.lines(opts), 1n), OR.Boards.pick(Od.lines(String.lines(Od.ran(b))), bd), bd, st)

# fab.intent: the person's own words for .backplane/intent.md, then the card
def Od.intent(+home: String, +cwd: String, +thread: String, +opts: String, text: String) -> IO(J.Json):
  do IO<J.Json>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(cwd ++ "/.backplane")
    GS.Gf.write(cwd ++ "/.backplane/intent.md", String.trim(text) ++ "\n")
    m : Result<&1, &1, U32 & String, Unit> <- H.FS.chmod(cwd ++ "/.backplane/intent.md", 420)
    Od.look(home, cwd, thread, opts)

def Od.keep(+home: String, +qid: String, +token: String, +thread: String, +server: String, +status: String) -> IO(Unit):
  do IO<Unit>:
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(Od.orders(home))
    GS.Gf.write(Od.orders(home) ++ "/" ++ qid ++ ".json", Od.record(qid, token, thread, server, status))

# the Buy now link: the server sends it straight to Stripe Checkout (with the token)
def Od.pay(+server: String, +qid: String, +token: String) -> String:
  server ++ "/q/" ++ qid ++ "?t=" ++ token
