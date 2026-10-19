import Base
import ./vt.bend as VT
import ./utf8.bend as U8
import ./model.bend as M
import ./diff.bend as DF

# Terminals in the chat (pure). An agent opens one with the terminal_open
# tool: a program on a pty, started in the thread's folder, shown in the
# thread as an entry the person clicks into and types at. The entry is an
# Act of tone "term" (kind "terminal", id the terminal's id) whose summary
# is "title\nCOLSxROWS\ncommand". Its key, the name its bytes travel under
# (the hub's terminal actor, the clients' `terms` map), is "x:thread:id".
# The hub keeps a screen per terminal too: it answers the agent's
# terminal_read, a client's snapshot (Emb.snap: bytes that draw the screen
# again on a blank terminal) and, at the next message the person sends,
# what the terminals show now (Emb.note).

# ESC
def Emb.e() -> String:
  SCon{Chr{27}, SNil{}}

def Emb.tone() -> String:
  "term"

def Emb.kind() -> String:
  "terminal"

def Emb.key(+thread: String, +id: String) -> String:
  "x:" ++ thread ++ ":" ++ id

# a terminal's key names an embedded one (a thread's own panel is keyed by
# the thread)
def Emb.is_key(+k: String) -> Bool:
  String.starts_with(k, "x:")

# the window's focus on a terminal in the chat
def Emb.focus(+key: String) -> String:
  "emb:" ++ key

def Emb.focused(+focus: String) -> String:
  Bool.pick(String, String.starts_with(focus, "emb:"), String.drop(focus, 4n), "")

# Sizes
# -----

# a digit onto acc (anything else is skipped)
def Emb.digit(+acc: U32, +c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), U32.min(((acc * 10 : U32) + (c - 48 : U32) : U32), 100000), acc)

def Emb.digits(s: String, +acc: U32) -> U32:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Emb.digits(t, Emb.digit(acc, c))

def Emb.num(+s: String) -> U32:
  Emb.digits(s, 0)

def Emb.clamp(+n: U32, +lo: U32, +hi: U32, +d: U32) -> U32:
  Bool.pick(U32, U32.is_eq(n, 0), d, U32.max(lo, U32.min(n, hi)))

# 0 (not given) is the default: 80 columns, 24 rows
def Emb.cols(+n: U32) -> U32:
  Emb.clamp(n, 20, 200, 80)

def Emb.rows(+n: U32) -> U32:
  Emb.clamp(n, 3, 60, 24)

# The entry
# ---------

def Emb.summary(+title: String, +cols: U32, +rows: U32, +cmd: String) -> String:
  title ++ "\n" ++ U32.show(cols) ++ "x" ++ U32.show(rows) ++ "\n" ++ cmd

# (lines cut by DF.Diff.lines and the title by DF.Diff.first, tail calls:
# String.split recurses once per character, and a long summary overflowed
# the phones' stack)
def Emb.part(+summary: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, DF.Diff.lines(summary), i), "")

def Emb.title(+summary: String) -> String:
  DF.Diff.first(summary)

def Emb.size(+summary: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(Emb.part(summary, 1n), 'x'), i), "")

def Emb.wide(+summary: String) -> U32:
  Emb.cols(Emb.num(Emb.size(summary, 0n)))

def Emb.tall(+summary: String) -> U32:
  Emb.rows(Emb.num(Emb.size(summary, 1n)))

# the command (it may hold newlines of its own)
def Emb.cmd(+summary: String) -> String:
  String.join(List.drop(&2, String, DF.Diff.lines(summary), 2n), "\n")

# The screen as text, and as bytes
# --------------------------------

def Emb.line(rs: List<&2, VT.VRun>) -> String:
  match rs:
    case Nil{}:
      ""
    case VT.VRun{t, _, _, _, _} <> rest:
      t ++ Emb.line(rest)

def Emb.lines(ls: List<&2, VT.VLine>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case VT.VLine{rs} <> t:
      Emb.line(rs) <> Emb.lines(t)

def Emb.trail(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case "" <> t:
      Emb.trail(t)
    case l <> t:
      l <> t

# what the screen shows, blank rows at the bottom left out
def Emb.text(+v: VT.Vt) -> String:
  String.join(List.reverse(&2, String, Emb.trail(List.reverse(&2, String, Emb.lines(VT.Vt.lines(v, 0n))))), "\n")

def Emb.rgb(+c: U32) -> String:
  U32.show((U32.shrn(c, 16n) .&. 255 : U32)) ++ ";" ++ U32.show((U32.shrn(c, 8n) .&. 255 : U32)) ++ ";" ++ U32.show((c .&. 255 : U32))

def Emb.sgr(+fg: U32, +bg: U32, bold: Bool, ul: Bool) -> String:
  Emb.e() ++ "[0" ++ Bool.pick(String, U32.is_eq(fg, VT.Vt.fg()), "", ";38;2;" ++ Emb.rgb(fg))
    ++ Bool.pick(String, U32.is_eq(bg, VT.Vt.bg()), "", ";48;2;" ++ Emb.rgb(bg))
    ++ Bool.pick(String, bold, ";1", "") ++ Bool.pick(String, ul, ";4", "") ++ "m"

def Emb.runs(rs: List<&2, VT.VRun>) -> String:
  match rs:
    case Nil{}:
      ""
    case VT.VRun{t, +fg, +bg, bo, ul} <> rest:
      Emb.sgr(fg, bg, bo, ul) ++ t ++ Emb.runs(rest)

def Emb.rows.snap(ls: List<&2, VT.VLine>, +y: U32) -> String:
  match ls:
    case Nil{}:
      ""
    case VT.VLine{rs} <> t:
      Emb.e() ++ "[" ++ U32.show(y) ++ ";1H" ++ Emb.runs(rs) ++ Emb.rows.snap(t, (y + 1 : U32))

def Emb.cur(c: VT.VCur) -> String:
  match c:
    case VT.VCur{+x, +y, on}:
      Bool.pick(String, on, "", Emb.e() ++ "[?25l") ++ Emb.e() ++ "[" ++ U32.show((y + 1 : U32)) ++ ";" ++ U32.show((x + 1 : U32)) ++ "H"

# text that draws v's screen again on a blank terminal of its size: the
# alternate screen, each row's runs, the modes keys depend on, the
# cursor and the title (the scrollback is left behind)
def Emb.snap.text(+v: VT.Vt) -> String:
  Bool.pick(String, VT.Vt.alt(v), Emb.e() ++ "[?1049h", "") ++ Emb.e() ++ "[H" ++ Emb.e() ++ "[2J" ++ Emb.rows.snap(VT.Vt.lines(v, 0n), 1) ++ Emb.e() ++ "[0m"
    ++ Bool.pick(String, VT.Vt.appk(v), Emb.e() ++ "[?1h", "") ++ Bool.pick(String, VT.Vt.bpaste(v), Emb.e() ++ "[?2004h", "")
    ++ Bool.pick(String, String.is_empty(VT.Vt.title(v)), "", Emb.e() ++ "]2;" ++ VT.Vt.title(v) ++ SCon{Chr{7}, SNil{}}) ++ Emb.cur(VT.Vt.cursor(v))

def Emb.snap(+v: VT.Vt) -> List<&2, U32>:
  U8.Utf8.encode(Emb.snap.text(v))

# The command, then a shell
# -------------------------
# A terminal stays a terminal: its command runs in the user's shell, and
# when it ends the shell prints a mark the screen ignores (an OSC the
# emulator skips: ESC ] 7799 ; done <status> BEL) and becomes an
# interactive shell in its place. The hub reads the mark (Emb.done) so the
# agent sees "done N" while the shell stays open.

def Emb.script(+cmd: String, +shell: String) -> String:
  cmd ++ "\nprintf '\\033]7799;done %s\\007' \"$?\"\nexec '" ++ shell ++ "' -l"

def Emb.done.num(bs: List<&2, U32>, +acc: String) -> String:
  match bs:
    case Nil{}:
      acc
    case 7 <> _:
      acc
    case b <> t:
      Emb.done.num(t, acc ++ SCon{Chr{b}, SNil{}})

# the status a mark among bs carries ("" when there is none)
def Emb.done(bs: List<&2, U32>) -> String:
  match bs:
    case Nil{}:
      ""
    case 27 <> 93 <> 55 <> 55 <> 57 <> 57 <> 59 <> 100 <> 111 <> 110 <> 101 <> 32 <> t:
      Emb.done.num(t, "")
    case _ <> t:
      Emb.done(t)

# a terminal's state once bytes bs came: "done N" after the mark
def Emb.after(+st: String, +bs: List<&2, U32>) -> String:
  Bool.pick(String, String.is_empty(Emb.done(bs)), st, "done " ++ Emb.done(bs))

# For the agent
# -------------

# one terminal as the agent reads it
def Emb.report(+id: String, +title: String, +state: String, +text: String) -> String:
  "<terminal id=\"" ++ id ++ "\" title=\"" ++ title ++ "\" state=\"" ++ state ++ "\">\n" ++ text ++ "\n</terminal>\n"

# the terminals that changed since the agent last read them, after the
# person's message ("" when none did)
def Emb.note(rs: List<&2, String>) -> String:
  match rs:
    case Nil{}:
      ""
    case _ <> _:
      "\n\n<terminals>\nWhat the terminals you opened in this chat show now (the person may have used them since):\n" ++ String.concat(rs) ++ "</terminals>"

# Live or inactive
# ----------------
# A terminal is live while it is pinned, or until the person sends a
# message after it was opened (or last restarted) or it has gone 30
# minutes since. Live ones are drawn as terminals; an inactive one is one
# gray row, and a click on it starts its command again (term.restart, which
# sets "emb.at:<key>" to that moment). A pinned one ("emb.pin:<key>" is
# "1") is drawn at the top of the thread, with the subagents.

def Emb.idle() -> Nat:
  1800n

def Emb.pin.key(+key: String) -> String:
  "emb.pin:" ++ key

def Emb.at.key(+key: String) -> String:
  "emb.at:" ++ key

def Emb.pinned(s: M.State, +key: String) -> Bool:
  String.eq(M.Setting.get(s, Emb.pin.key(key), ""), "1")

def Emb.at.num(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Emb.at.num(t, Bool.pick(Nat, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))), acc))

# when it was opened or last restarted
def Emb.touched(s: M.State, +key: String, +at: Nat) -> Nat:
  M.Time.max(at, Emb.at.num(M.Setting.get(s, Emb.at.key(key), ""), 0n))

def Emb.user.at(e: M.Entry) -> Nat:
  match e:
    case M.Msg{_, _, M.User{}, _, at}:
      at
    case _:
      0n

# the newest message the person sent among es: entries are newest first
# (model.bend's Idx), so the walk stops at the first of the person's
# (it went through every entry, twice a frame and once more per terminal:
# 6000 entries each time in a long thread)
def Emb.last_user.go(es: List<&2, M.Entry>, done: Bool, +acc: Nat) -> Nat:
  match es done:
    case Nil{} _:
      acc
    case +e <> +t True{}:
      acc
    case +e <> t False{}:
      Emb.last_user.go(t, Nat.is_gt(Emb.user.at(e), 0n), M.Time.max(acc, Emb.user.at(e)))

def Emb.last_user(es: List<&2, M.Entry>, +acc: Nat) -> Nat:
  Emb.last_user.go(es, False{}, acc)

def Emb.live.k(pinned: Bool, +touched: Nat, +last: Nat, +now: Nat) -> Bool:
  match pinned:
    case True{}:
      True{}
    case False{}:
      Bool.and(Nat.is_le(last, touched), Nat.is_lt(now, Nat.add(touched, Emb.idle())))

def Emb.live(+pinned: Bool, +touched: Nat, +last: Nat, +now: Nat) -> Bool:
  Emb.live.k(pinned, touched, last, now)

def Emb.is(e: M.Entry) -> Bool:
  match e:
    case M.Act{_, _, +tone, _, _, _}:
      String.eq(tone, "term")
    case _:
      False{}

# a terminal entry's key, live state and pin (for the clients)
type EV is Data:
  EV{key: String, id: String, title: String, cols: U32, rows: U32, live: Bool, pinned: Bool}

def Emb.view(+s: M.State, e: M.Entry, +last: Nat, +now: Nat) -> EV:
  match e:
    case M.Act{+id, +th, _, _, +sum, +at}:
      +key = Emb.key(th, id)
      EV{key, id, Emb.title(sum), Emb.wide(sum), Emb.tall(sum), Emb.live(Emb.pinned(s, key), Emb.touched(s, key, at), last, now), Emb.pinned(s, key)}
    case M.Msg{+id, _, _, _, _}:
      EV{"", id, "", 80, 24, False{}, False{}}

def Emb.pins.k(pin: Bool, +s: M.State, +e: M.Entry, +last: Nat, +now: Nat, rest: List<&2, EV>) -> List<&2, EV>:
  match pin:
    case True{}:
      Emb.view(s, e, last, now) <> rest
    case False{}:
      rest

def Emb.pin.of(+s: M.State, +e: M.Entry) -> Bool:
  match e:
    case M.Act{+id, +th, +tone, _, _, _}:
      Bool.and(String.eq(tone, "term"), Emb.pinned(s, Emb.key(th, id)))
    case _:
      False{}

# (each walk over a thread's entries is a tail call onto a reversed
# accumulator: a thread holds thousands, and a walk that built its result
# around the recursive call overflowed the web's and phones' JS stack, so a
# long thread could not be opened)
def Emb.pins.acc(es: List<&2, M.Entry>, +s: M.State, +last: Nat, +now: Nat, acc: List<&2, EV>) -> List<&2, EV>:
  match es:
    case Nil{}:
      acc
    case +e <> t:
      Emb.pins.acc(t, s, last, now, Emb.pins.k(Emb.pin.of(s, e), s, e, last, now, acc))

def Emb.pins.go(es: List<&2, M.Entry>, +s: M.State, +last: Nat, +now: Nat) -> List<&2, EV>:
  List.reverse(&2, EV, Emb.pins.acc(es, s, last, now, Nil{}))

# a thread's pinned terminals
def Emb.pins(+s: M.State, +th: String, +now: Nat) -> List<&2, EV>:
  Emb.pins.go(M.State.terms(s, th), s, Emb.last_user(M.State.mine(s, th), 0n), now)

def Emb.ekey(+e: M.Entry) -> String:
  match e:
    case M.Act{+id, +th, _, _, _, _}:
      Emb.key(th, id)
    case _:
      ""

def Emb.keys.k(hit: Bool, +e: M.Entry, rest: List<&2, String>) -> List<&2, String>:
  match hit:
    case True{}:
      Emb.ekey(e) <> rest
    case False{}:
      rest

# the keys of a thread's terminals that are not pinned (they stop when the
# person sends the next message)
def Emb.loose.acc(es: List<&2, M.Entry>, +s: M.State, acc: List<&2, String>) -> List<&2, String>:
  match es:
    case Nil{}:
      acc
    case +e <> t:
      Emb.loose.acc(t, s, Emb.keys.k(Bool.and(Emb.is(e), Bool.not(Emb.pin.of(s, e))), e, acc))

def Emb.loose(es: List<&2, M.Entry>, +s: M.State) -> List<&2, String>:
  List.reverse(&2, String, Emb.loose.acc(es, s, Nil{}))

def Emb.any.go(es: List<&2, M.Entry>, +found: Bool) -> Bool:
  match es:
    case Nil{}:
      found
    case +e <> t:
      Emb.any.go(t, Bool.or(found, Emb.is(e)))

def Emb.any(es: List<&2, M.Entry>) -> Bool:
  Emb.any.go(es, False{})

def Emb.sum.of(+e: M.Entry) -> String:
  match e:
    case M.Act{_, _, _, _, +sum, _}:
      sum
    case _:
      ""

def Emb.sum.k(hit: Bool, +e: M.Entry, +rest: String) -> String:
  match hit:
    case True{}:
      Emb.sum.of(e)
    case False{}:
      rest

def Emb.hit(+e: M.Entry, +id: String) -> Bool:
  match e:
    case M.Act{i, _, +tone, _, _, _}:
      Bool.and(String.eq(tone, "term"), String.eq(i, id))
    case _:
      False{}

def Emb.sum.pick(got: Bool, +hit: Bool, +e: M.Entry, +acc: String) -> String:
  match got:
    case True{}:
      acc
    case False{}:
      Emb.sum.k(hit, e, acc)

def Emb.sum.go(es: List<&2, M.Entry>, +id: String, +got: Bool, +acc: String) -> String:
  match es:
    case Nil{}:
      acc
    case +e <> t:
      Emb.sum.go(t, id, Bool.or(got, Emb.hit(e, id)), Emb.sum.pick(got, Emb.hit(e, id), e, acc))

# the summary of terminal id among es, the first that has it ("" when
# there is none)
def Emb.sum(es: List<&2, M.Entry>, +id: String) -> String:
  Emb.sum.go(es, id, False{}, "")
