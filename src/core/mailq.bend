import Base
import ./model.bend as M
import ./mail.bend as ML
import ./mailroom_history.bend as RH

# The mail box's headers from the store (pure)
# --------------------------------------------
# ML.Box keeps every mail header in memory (Box.mails: per conversation,
# Headers.add's order; Box.ids: every id seen). store.db's `mail` table
# (core/storedb.bend: one row per header the box took, by its line's seq)
# answers the same reads by query: the newest header of a conversation,
# a page of its headers after a cursor id, one header by id, whether an
# id was seen. server/thinstore.bend's Ms.* run these.
#
# Headers.add's order, from arrival order alone: a header whose `at` is at
# least every earlier one's (in its conversation) becomes the head; any
# other goes second. So the head is the newest by `at` (the later of
# equals), and the rest stand in the order they left the head's place or
# came in under it, newest first: a header that came in under the head at
# its own seq, a head when the next head came. `k` is that seq (the head:
# the largest integer), and the box's list is the rows by k, descending.

def Mq.cols() -> String:
  "seq, id, conv, src, dst, kind, ref, want, head, size, at"

# a conversation's headers with their k (?1: the conversation)
def Mq.sql.keyed() -> String:
  "WITH x AS (SELECT " ++ Mq.cols() ++ ", max(at) OVER (ORDER BY seq ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING) AS pm FROM mail WHERE conv = ?1)," ++
  " y AS (SELECT *, (pm IS NULL OR at >= pm) AS hd FROM x)," ++
  " z AS (SELECT *, CASE WHEN hd THEN coalesce(lead(seq) OVER (PARTITION BY hd ORDER BY seq), 9223372036854775807) ELSE seq END AS k FROM y) "

# the headers after the one with id ?2 (all of them for ""), at most ?3,
# in the box's order
def Mq.sql.page() -> String:
  Mq.sql.keyed() ++ "SELECT " ++ Mq.cols() ++ " FROM z WHERE ?2 = '' OR k < (SELECT k FROM z WHERE id = ?2) ORDER BY k DESC LIMIT ?3"

# the newest header (Box.newest): the largest at, the later of equals
def Mq.sql.newest() -> String:
  "SELECT " ++ Mq.cols() ++ " FROM mail WHERE conv = ?1 ORDER BY at DESC, seq DESC LIMIT 1"

# one header of a conversation by id
def Mq.sql.one() -> String:
  "SELECT " ++ Mq.cols() ++ " FROM mail WHERE conv = ?1 AND id = ?2 LIMIT 1"

# whether the box took a header with this id (Box.has)
def Mq.sql.has() -> String:
  "SELECT count(*) FROM (SELECT 1 FROM mail WHERE id = ?1 LIMIT 1)"

# whether the conversation is known (Box.conv's id is not empty)
def Mq.sql.conv() -> String:
  "SELECT count(*) FROM conv WHERE id = ?1"

# the room posts' texts for these ids (a JSON array) in room ?1, the
# newest of each id first (World.posts is newest first)
def Mq.sql.sources() -> String:
  "SELECT id, text FROM post WHERE room = ?1 AND id IN (SELECT value FROM json_each(?2)) ORDER BY seq DESC"

def Mq.num(s: String) -> Nat:
  M.Switch.num(s, 0n)

def Mq.mail(cs: List<&2, String>) -> ML.Mail:
  match cs:
    case _ <> id <> c <> f <> t <> k <> r <> w <> h <> sz <> at <> _:
      ML.Mail{id, c, f, t, k, r, w, h, Mq.num(sz), Mq.num(at)}
    case _:
      ML.Mail{"", "", "", "", "", "", "", "", 0n, 0n}

def Mq.mails(xs: List<&2, List<&2, String>>) -> List<&2, ML.Mail>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: Mq.mail(x) <> Mq.mails(t)

def Mq.first(xs: List<&2, List<&2, String>>) -> ML.Mail:
  match xs:
    case x <> _: Mq.mail(x)
    case Nil{}: ML.Mail{"", "", "", "", "", "", "", "", 0n, 0n}

def Mq.count(xs: List<&2, List<&2, String>>) -> Nat:
  match xs:
    case (n <> _) <> _: Mq.num(n)
    case _: 0n

# A room's page (RH.RoomHistory.page) from the rows after the cursor (at
# most n + 1): None when the conversation or the cursor is unknown
def Mq.cap(+n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_gt(n, 200n), 200n, n)

def Mq.page.k(known: Bool, rows: List<&2, List<&2, String>>, +n: Nat) -> Maybe<&2, RH.RoomPage>:
  match known:
    case False{}: None{}
    case True{}: Some{RH.RoomHistory.take(Mq.mails(rows), Mq.cap(n), Nil{})}

def Mq.page(+conv_known: Bool, +cursor_known: Bool, rows: List<&2, List<&2, String>>, +n: Nat) -> Maybe<&2, RH.RoomPage>:
  Mq.page.k(Bool.and(conv_known, cursor_known), rows, n)

# texts by id, the first row of each id kept (Map.set on a fresh id only)
def Mq.sources.put(have: Bool, +id: String, text: String, m: Map<&2, String>) -> Map<&2, String>:
  match have:
    case True{}: m
    case False{}: Map.set(&2, String, m, id, text)

def Mq.has.k(p: Map<&2, String> & Bool) -> Bool:
  (_, b) = p
  b

def Mq.sources(xs: List<&2, List<&2, String>>, +m: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}: m
    case (+id <> text <> _) <> t: Mq.sources(t, Mq.sources.put(Mq.has.k(Map.has(&2, String, m, id)), id, text, m))
    case _ <> t: Mq.sources(t, m)
