import Base
import ./json.bend as J
import ./model.bend as M
import ./bot.bend as BT
import ./notice.bend as NO

# What the person has actually been told. Each record is the setting
# "told.<id>" holding {"key","text","channel","bot","at"}: the topic key
# (normalized), what was said, where (alert, digest, room, chat, email or
# other), the bot that said it ("" for a thread that is no bot's or the hub
# itself) and when. Every alert record the hub writes is marked (Told.after,
# from hub.bend's Alerts.changes); agents mark what they say
# and check before they say it again (core/ledger.bend's told_mark and
# told_check). The minute tick removes records older than Told.keep()
# seconds, and all but the newest Told.max() (Told.prune). They are
# settings, so they stay on this machine.

type Told is Data:
  Told{id: String, key: String, text: String, channel: String, bot: String, at: Nat}

def Told.id(+t: Told) -> String:
  match t:
    case Told{i, _, _, _, _, _}:
      i

def Told.key(+t: Told) -> String:
  match t:
    case Told{_, k, _, _, _, _}:
      k

def Told.text(+t: Told) -> String:
  match t:
    case Told{_, _, x, _, _, _}:
      x

def Told.channel(+t: Told) -> String:
  match t:
    case Told{_, _, _, c, _, _}:
      c

def Told.bot(+t: Told) -> String:
  match t:
    case Told{_, _, _, _, b, _}:
      b

def Told.at(+t: Told) -> Nat:
  match t:
    case Told{_, _, _, _, _, a}:
      a

# 30 days, in seconds
def Told.keep() -> Nat:
  2592000n

# the most records kept
def Told.max() -> Nat:
  2000n

# Keys and words
# --------------

# a character a key may hold: a-z, 0-9 and . _ : -
def Told.key.ok(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))),
    Bool.or(Bool.or(U32.is_eq(c, 46), U32.is_eq(c, 95)), Bool.or(U32.is_eq(c, 58), U32.is_eq(c, 45))))

def Told.lower(+c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), (c + 32 : U32), c)

# how much of a key is made so far, whether its last character was a
# replacement dash, and its characters reversed
type NS is Data:
  NS{n: Nat, dash: Bool, acc: String}

# a good character is kept; a run of others becomes one "-"; nothing is
# added once 120 characters are in
def Told.norm.take(+full: Bool, +ok: Bool, +dash: Bool, +c: U32, +n: Nat, +acc: String) -> NS:
  match full ok dash:
    case True{} _ _:
      NS{n, dash, acc}
    case False{} True{} _:
      NS{Nat.add(n, 1n), False{}, SCon{Chr{c}, acc}}
    case False{} False{} True{}:
      NS{n, True{}, acc}
    case False{} False{} False{}:
      NS{Nat.add(n, 1n), True{}, SCon{Chr{45}, acc}}

def Told.norm.step(+c: U32, +st: NS) -> NS:
  match st:
    case NS{+n, dash, acc}:
      Told.norm.take(Nat.is_ge(n, 120n), Told.key.ok(Told.lower(c)), dash, Told.lower(c), n, acc)

def Told.norm.go(+s: String, +st: NS) -> NS:
  match s:
    case SNil{}:
      st
    case SCon{Chr{+c}, t}:
      Told.norm.go(t, Told.norm.step(c, st))

def Told.norm.fin(+st: NS) -> String:
  match st:
    case NS{_, _, acc}:
      String.reverse(acc)

# the topic key: trimmed, lower case, each run of other characters one "-",
# at most 120 characters
def Told.key.norm(+s: String) -> String:
  Told.norm.fin(Told.norm.go(String.trim(s), NS{0n, False{}, SNil{}}))

def Told.alnum(+c: U32) -> Bool:
  Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)))

def Told.has(+xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, x), Told.has(t, x))

# the word being read (its length, its characters reversed; a word stops
# growing at 64 characters), how many words are kept, and them reversed
type TW is Data:
  TW{n: Nat, cur: String, got: Nat, acc: List<&2, String>}

def Told.word.keep(+ok: Bool, +w: String, +got: Nat, +acc: List<&2, String>) -> TW:
  match ok:
    case True{}:
      TW{0n, SNil{}, Nat.add(got, 1n), w <> acc}
    case False{}:
      TW{0n, SNil{}, got, acc}

def Told.word.end.k(+ok: Bool, +distinct: Bool, +w: String, +got: Nat, +acc: List<&2, String>) -> TW:
  Told.word.keep(Bool.and(ok, Bool.not(Bool.and(distinct, Told.has(acc, w)))), w, got, acc)

# a word ends: kept when it has 3 or more characters, is new (when words
# must be distinct) and the cap is not reached
def Told.word.end(+distinct: Bool, +max: Nat, +n: Nat, +cur: String, +got: Nat, +acc: List<&2, String>) -> TW:
  Told.word.end.k(Bool.and(Nat.is_ge(n, 3n), Nat.is_lt(got, max)), distinct, String.reverse(cur), got, acc)

def Told.word.add(+more: Bool, +c: U32, +cur: String) -> String:
  match more:
    case True{}:
      SCon{Chr{c}, cur}
    case False{}:
      cur

def Told.words.step(+alnum: Bool, +distinct: Bool, +max: Nat, +c: U32, +st: TW) -> TW:
  match alnum st:
    case True{} TW{+n, cur, got, acc}:
      TW{Nat.add(n, 1n), Told.word.add(Nat.is_lt(n, 64n), c, cur), got, acc}
    case False{} TW{n, cur, got, acc}:
      Told.word.end(distinct, max, n, cur, got, acc)

def Told.words.go(+s: String, +distinct: Bool, +max: Nat, +st: TW) -> TW:
  match s:
    case SNil{}:
      st
    case SCon{Chr{+c}, t}:
      Told.words.go(t, distinct, max, Told.words.step(Told.alnum(Told.lower(c)), distinct, max, Told.lower(c), st))

def Told.words.out(+st: TW) -> List<&2, String>:
  match st:
    case TW{_, _, _, acc}:
      List.reverse(&2, String, acc)

def Told.words.fin(+distinct: Bool, +max: Nat, +st: TW) -> List<&2, String>:
  match st:
    case TW{n, cur, got, acc}:
      Told.words.out(Told.word.end(distinct, max, n, cur, got, acc))

# lower-cased runs of letters and digits, 3 or more characters each, `max`
# of them at most (distinct ones when asked), in one tail-recursive walk;
# only the characters kept are lowered
def Told.words.run(+s: String, +distinct: Bool, +max: Nat) -> List<&2, String>:
  Told.words.fin(distinct, max, Told.words.go(s, distinct, max, TW{0n, SNil{}, 0n, Nil{}}))

def Told.words(+s: String) -> List<&2, String>:
  Told.words.run(s, True{}, 40n)

# the key made from a text: its first 8 words of 3 or more characters
def Told.key.of(+s: String) -> String:
  Told.key.norm(String.join(Told.words.run(s, False{}, 8n), "-"))

# how many of q's words r has too
def Told.overlap(+q: List<&2, String>, +r: List<&2, String>, +n: Nat) -> Nat:
  match q:
    case Nil{}:
      n
    case +h <> t:
      Told.overlap(t, r, Bool.pick(Nat, Told.has(r, h), Nat.add(n, 1n), n))

# m words in the smaller set, o shared: the one word of a set of one, else
# at least 2 and 60% of the smaller set
def Told.same.n(+m: Nat, +o: Nat) -> Bool:
  match m:
    case 0n:
      False{}
    case 1n:
      Nat.is_ge(o, 1n)
    case _:
      Bool.and(Nat.is_ge(o, 2n), Nat.is_ge(Nat.mul(o, 5n), Nat.mul(m, 3n)))

def Told.same.k(+a: Nat, +b: Nat, +o: Nat) -> Bool:
  Told.same.n(Bool.pick(Nat, Nat.is_lt(a, b), a, b), o)

# the two word sets overlap enough to be one thing told twice
def Told.same(+q: List<&2, String>, +r: List<&2, String>) -> Bool:
  Told.same.k(List.length(&2, String, q), List.length(&2, String, r), Told.overlap(q, r, 0n))

# Records
# -------

def Told.channel.k(+empty: Bool, +known: Bool, +c: String) -> String:
  match empty known:
    case True{} _:
      "chat"
    case False{} True{}:
      c
    case False{} False{}:
      "other"

def Told.channel.of(+c: String) -> String:
  Told.channel.k(String.is_empty(c), Told.has(["alert", "digest", "room", "chat", "email", "other"], c), c)

def Told.channel.ok(+c: String) -> String:
  Told.channel.of(String.to_lower(String.take(String.trim(c), 20n)))

def Told.json(+t: Told) -> J.Json:
  match t:
    case Told{_, k, x, c, b, a}:
      J.Json.obj([J.KV{"key", J.Str{k}}, J.KV{"text", J.Str{x}}, J.KV{"channel", J.Str{c}}, J.KV{"bot", J.Str{b}}, J.KV{"at", J.Json.nat(a)}])

def Told.value(+t: Told) -> String:
  J.Json.show(Told.json(t))

def Told.of.json(+id: String, +o: J.Json) -> Told:
  Told{id, J.Json.get_str(o, "key", ""), J.Json.get_str(o, "text", ""), J.Json.get_str(o, "channel", "other"), J.Json.get_str(o, "bot", ""), J.Json.get_nat(o, "at", 0n)}

def Told.key.id(+k: String) -> String:
  String.drop(k, 5n)

def Told.is(+k: String) -> Bool:
  String.starts_with(k, "told.")

def Told.read.one(+m: Maybe<&2, J.Json>, +k: String, +acc: List<&2, Told>) -> List<&2, Told>:
  match m:
    case Some{o}:
      Told.of.json(Told.key.id(k), o) <> acc
    case None{}:
      acc

def Told.read.put(+hit: Bool, +k: String, +s: M.State, +acc: List<&2, Told>) -> List<&2, Told>:
  match hit:
    case True{}:
      Told.read.one(J.Json.parse(M.Setting.get(s, k, "")), k, acc)
    case False{}:
      acc

def Told.read(+ks: List<&2, String>, +s: M.State, +acc: List<&2, Told>) -> List<&2, Told>:
  match ks:
    case Nil{}:
      acc
    case +k <> t:
      Told.read(t, s, Told.read.put(Told.is(k), k, s, acc))

# newest first
def Told.newer(a: Told, b: Told) -> Bool:
  Nat.is_ge(Told.at(a), Told.at(b))

# every record, newest first, in one pass over the settings' keys
def Told.all(+s: M.State) -> List<&2, Told>:
  List.sort(~Told, ~Told.newer, Told.read(M.Setting.keys(s), s, Nil{}))

# Marking
# -------

def Told.seq(+s: M.State) -> Nat:
  match s:
    case M.State{_, _, _, n, _, _, _, _, _, _, _}:
      n

# the same shape as the hub's ids (Hub.id): "k<time>-<sequence>"; prune
# reads the time back from it
def Told.new.id(+now: Nat, +seq: Nat) -> String:
  "k" ++ Nat.show(now) ++ "-" ++ Nat.show(seq)

def Told.text.cut(+text: String) -> String:
  String.trim(String.take(String.trim(text), 280n))

# the key given, normalized; else made from the text; else the text itself
def Told.key.pick.k(+made: String, +text: String) -> String:
  Bool.pick(String, String.is_empty(made), Told.key.norm(text), made)

def Told.key.pick(+key: String, +text: String) -> String:
  Bool.pick(String, String.is_empty(String.trim(key)), Told.key.pick.k(Told.key.of(text), text), Told.key.norm(key))

def Told.mark.set(+id: String, +now: Nat, +key: String, +text: String, +channel: String, +bot: String) -> List<&2, M.Change>:
  [M.SettingSet{"told." ++ id, Told.value(Told{id, key, text, Told.channel.ok(channel), bot, now}), now}]

def Told.mark.k(+none: Bool, +s: M.State, +now: Nat, +key: String, +text: String, +channel: String, +bot: String) -> List<&2, M.Change>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      Told.mark.set(Told.new.id(now, Told.seq(s)), now, Told.key.pick(key, text), Told.text.cut(text), channel, bot)

# Record that the person was told something. The key is the given one,
# normalized, else made from the text (Told.key.of). With neither a key
# nor a text there is nothing to mark.
def Told.mark(+s: M.State, +now: Nat, +key: String, +text: String, +channel: String, +bot: String) -> List<&2, M.Change>:
  Told.mark.k(Bool.and(String.is_empty(String.trim(key)), String.is_empty(String.trim(text))), s, now, key, text, channel, bot)

# Checking
# --------

# how many days back (default 7, at most 30)
def Told.days(+d: Nat) -> Nat:
  match d:
    case 0n:
      7n
    case _:
      Bool.pick(Nat, Nat.is_gt(d, 30n), 30n, d)

def Told.hit(+k: String, +qw: List<&2, String>, +r: Told) -> Bool:
  Bool.or(Bool.and(Bool.not(String.is_empty(k)), String.eq(Told.key(r), k)), Told.same(qw, Told.words(Told.text(r))))

# newest first: how many are found, whether the search is over, and them
# reversed
type CS is Data:
  CS{n: Nat, stop: Bool, acc: List<&2, Told>}

def Told.check.take(+stop: Bool, +hit: Bool, +r: Told, +n: Nat, +acc: List<&2, Told>) -> CS:
  match stop hit:
    case True{} _:
      CS{n, True{}, acc}
    case False{} True{}:
      CS{Nat.add(n, 1n), False{}, r <> acc}
    case False{} False{}:
      CS{n, False{}, acc}

# the search ends at a record older than `since` or at the tenth found
def Told.check.step(+r: Told, +since: Nat, +k: String, +qw: List<&2, String>, +st: CS) -> CS:
  match st:
    case CS{+n, True{}, acc}:
      CS{n, True{}, acc}
    case CS{+n, False{}, acc}:
      Told.check.take(Bool.or(Nat.is_lt(Told.at(r), since), Nat.is_ge(n, 10n)), Told.hit(k, qw, r), r, n, acc)

def Told.check.go(+xs: List<&2, Told>, +since: Nat, +k: String, +qw: List<&2, String>, +st: CS) -> CS:
  match xs:
    case Nil{}:
      st
    case +r <> t:
      Told.check.go(t, since, k, qw, Told.check.step(r, since, k, qw, st))

def Told.check.out(+st: CS) -> List<&2, Told>:
  match st:
    case CS{_, _, acc}:
      List.reverse(&2, Told, acc)

def Told.check.d(+day: Nat, +s: M.State, +now: Nat, key: String, text: String, days: Nat) -> List<&2, Told>:
  Told.check.out(Told.check.go(Told.all(s), Nat.sub(now, Nat.mul(Told.days(days), day)), Told.key.norm(key), Told.words(text), CS{0n, False{}, Nil{}}))

# The records of the last `days` days (default 7, at most 30) that have
# this key (normalized), or whose words overlap the text's (Told.same);
# newest first, at most ten.
def Told.check(+s: M.State, +now: Nat, key: String, text: String, days: Nat) -> List<&2, Told>:
  Told.check.d(86400n, s, now, key, text, days)

# Pruning
# -------

# a key and the time its record was made
type TK is Data:
  TK{key: String, at: Nat}

# the time in a record's id "k<time>-<sequence>": the digits read so far,
# whether there were any, and 0 reading, 1 done, 2 not an id of ours
type TS is Data:
  TS{acc: Nat, any: Bool, st: Nat}

def Told.stamp.step(+digit: Bool, +dash: Bool, +c: U32, +ts: TS) -> TS:
  match digit dash ts:
    case True{} _ TS{+a, _, 0n}:
      TS{Nat.add(Nat.mul(a, 10n), U32.to_nat((c - 48 : U32))), True{}, 0n}
    case False{} True{} TS{a, True{}, 0n}:
      TS{a, True{}, 1n}
    case _ _ TS{a, any, 0n}:
      TS{a, any, 2n}
    case _ _ TS{a, any, s}:
      TS{a, any, s}

def Told.stamp.go(+s: String, +ts: TS) -> TS:
  match s:
    case SNil{}:
      ts
    case SCon{Chr{+c}, t}:
      Told.stamp.go(t, Told.stamp.step(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), U32.is_eq(c, 45), c, ts))

# the time of a record's id, None when it is not one of ours
def Told.stamp.of(+ts: TS) -> Maybe<&2, Nat>:
  match ts:
    case TS{a, _, 1n}:
      Some{a}
    case _:
      None{}

def Told.stamp.k(+k: Bool, +t: String) -> Maybe<&2, Nat>:
  match k:
    case True{}:
      Told.stamp.of(Told.stamp.go(t, TS{0n, False{}, 0n}))
    case False{}:
      None{}

def Told.stamp.id(+s: String) -> Maybe<&2, Nat>:
  match s:
    case SCon{Chr{+c}, t}:
      Told.stamp.k(U32.is_eq(c, 107), t)
    case SNil{}:
      None{}

# a key's time: from its id, else (a record written by hand) from its value
def Told.tk.at(+m: Maybe<&2, Nat>, +k: String, +s: M.State) -> Nat:
  match m:
    case Some{a}:
      a
    case None{}:
      J.Json.get_nat(Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, k, "")), J.Null{}), "at", 0n)

def Told.tk.put(+hit: Bool, +k: String, +s: M.State, +acc: List<&2, TK>) -> List<&2, TK>:
  match hit:
    case True{}:
      TK{k, Told.tk.at(Told.stamp.id(Told.key.id(k)), k, s)} <> acc
    case False{}:
      acc

def Told.tk.read(+ks: List<&2, String>, +s: M.State, +acc: List<&2, TK>) -> List<&2, TK>:
  match ks:
    case Nil{}:
      acc
    case +k <> t:
      Told.tk.read(t, s, Told.tk.put(Told.is(k), k, s, acc))

def Told.tk.newer(a: TK, b: TK) -> Bool:
  match a b:
    case TK{_, x} TK{_, y}:
      Nat.is_ge(x, y)

def Told.prune.drop(+gone: Bool, +k: String, +now: Nat, +rest: List<&2, M.Change>) -> List<&2, M.Change>:
  match gone:
    case True{}:
      M.SettingSet{k, "", now} <> rest
    case False{}:
      rest

def Told.prune.go(xs: List<&2, TK>, +keep: Nat, +max: Nat, +i: Nat, +now: Nat) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      Nil{}
    case TK{+k, +at} <> t:
      Told.prune.drop(Bool.or(Nat.is_ge(i, max), Nat.is_lt(Nat.add(at, keep), now)), k, now, Told.prune.go(t, keep, max, Nat.add(i, 1n), now))

# Removing: a record older than 30 days, and the oldest ones beyond the
# newest 2000. Reads only the keys (a record's time is in its id), so the
# minute tick costs one pass over the settings' keys.
def Told.prune.at(+keep: Nat, +max: Nat, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  Told.prune.go(List.sort(~TK, ~Told.tk.newer, Told.tk.read(M.Setting.keys(s), s, Nil{})), keep, max, 0n, now)

def Told.prune(+s: M.State, +now: Nat) -> List<&2, M.Change>:
  Told.prune.at(Told.keep(), Told.max(), s, now)

# Turns
# -----

def Told.last(+es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}:
      ""
    case M.Msg{_, _, M.Assistant{}, text, _} <> _:
      text
    case _ <> t:
      Told.last(t)

# the first line of a reply, at most 200 characters
def Told.first(+text: String) -> String:
  String.take(String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(String.take(String.trim(text), 400n))), "")), 200n)

def Told.title(+t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, title, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      title

def Told.botid(+m: Maybe<&2, BT.Bot>) -> String:
  match m:
    case Some{b}:
      BT.Bot.id(b)
    case None{}:
      ""

def Told.line(+title: String, +line: String) -> String:
  Bool.pick(String, String.is_empty(line), title, title ++ ": " ++ line)

# Alerts
# ------
# Every alert comes from one place since the notification policy
# (notice.bend's Policy, hub.bend's Alerts.*): a step writes the thread's
# record "alert.<thread>" with a kind, and the server alerts for it. So the
# person was told exactly what those records say: each one with a kind is
# marked here, under the record's key ("turn-<thread>", "room-<room>",
# "note-..."), channel alert, the thread's bot, and the record's text (a
# note's, an ask's, a merge's url) or else the thread's title with the
# first line of its last reply. A quiet record (no kind) marks nothing: a
# bot's chatter, a report from a launched thread, a stopped turn.

def Told.alerting(+k: String, +v: String) -> Bool:
  Bool.and(String.starts_with(k, "alert."), Bool.and(Bool.not(String.is_empty(v)), Bool.not(String.starts_with(v, "\t"))))

def Told.rec.said(+title: String, +said: String, +s: M.State, +thread: String) -> String:
  Bool.pick(String, String.is_empty(said), Told.line(title, Told.first(Told.last(M.State.mine(s, thread)))), Told.line(title, said))

def Told.rec.key(+key: String, +thread: String) -> String:
  Bool.pick(String, String.is_empty(key), "turn-" ++ thread, key)

def Told.rec.mark(+s: M.State, +i: Nat, +thread: String, +r: NO.AlertRec, +at: Nat) -> List<&2, M.Change>:
  Told.mark.set(Told.new.id(at, Told.seq(s)) ++ "-" ++ Nat.show(i), at, Told.key.norm(Told.rec.key(NO.Alert.rkey(r), thread)),
    Told.text.cut(Told.rec.said(Told.title(M.State.thread.get(s, thread)), NO.Alert.rtext(r), s, thread)), "alert", Told.botid(BT.World.of_thread(M.State.bots(s), thread)))

def Told.alerts.one(+hit: Bool, +s: M.State, +i: Nat, +k: String, +v: String, +at: Nat) -> List<&2, M.Change>:
  match hit:
    case True{}:
      Told.rec.mark(s, i, String.drop(k, 6n), NO.Alert.parse(v), at)
    case False{}:
      Nil{}

# the marks for a step's changes: one per record that alerts (`s` is the
# state before them; i keeps ids apart within the step)
def Told.alerts(cs: List<&2, M.Change>, +s: M.State, +i: Nat) -> List<&2, M.Change>:
  match cs:
    case Nil{}:
      Nil{}
    case M.SettingSet{+k, +v, +at} <> t:
      List.append(&2, M.Change, Told.alerts.one(Told.alerting(k, v), s, i, k, v, at), Told.alerts(t, s, 1n+i))
    case _ <> t:
      Told.alerts(t, s, 1n+i)

# a step's changes with what they told the person (hub.bend's
# Alerts.changes, which every step passes through before it is stored)
def Told.after(+st: M.State, +cs: List<&2, M.Change>) -> List<&2, M.Change>:
  List.append(&2, M.Change, cs, Told.alerts(cs, st, 0n))

# what a turn's end marks: the records its end writes (the same
# NO.Policy.end the hub's Ended.out uses), marked as above
def Told.alert(+s: M.State, +now: Nat, +thread: String, +state: String) -> List<&2, M.Change>:
  Told.alerts(NO.Policy.end(s, now, thread, state), s, 0n)

# Examples the laws read
# ----------------------

# one record, marked at 1000: key "  Lunch: Dana!!  ", the text, channel digest
def Told.demo.marked() -> M.State:
  M.State.replay(Told.mark(M.State.empty(), 1000n, "  Lunch: Dana!!  ", "Dana lunch in NYC Wed 10/28", "digest", ""), M.State.empty())

def Told.demo.first(+s: M.State) -> Told:
  Maybe.default(&2, Told, List.head(&2, Told, Told.all(s)), Told{"", "", "", "", "", 0n})

# a bot woken by a room message that posted nothing: its turn's end is quiet
def Told.demo.chatter() -> M.State:
  M.State.replay([M.ThreadCreated{"tb", "", "Bub", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Bub", "tb", 0n, "", 100n},
    M.MessagePosted{"tb", "u1", M.User{}, "[message from Pebble@backplane in #ring-requests]\nshort version", 1000n}, M.TurnChanged{"tb", M.Running{}, 1000n},
    M.MessagePosted{"tb", "m1", M.Assistant{}, "Noted.", 1004n}, M.TurnChanged{"tb", M.Completed{}, 1005n}], M.State.empty())

# the same bot woken by a report from a thread it launched (hub.bend's
# Lr.head): bot-to-bot, the launched thread alerted for itself
def Told.demo.report() -> M.State:
  M.State.replay([M.ThreadCreated{"tb", "", "Bub", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Bub", "tb", 0n, "", 100n},
    M.MessagePosted{"tb", "t:tw:m9", M.User{}, "[Backplane] Launched thread \"work\" (tw) completed.", 1000n}, M.TurnChanged{"tb", M.Running{}, 1000n},
    M.MessagePosted{"tb", "m1", M.Assistant{}, "Noted.", 1004n}, M.TurnChanged{"tb", M.Completed{}, 1005n}], M.State.empty())

