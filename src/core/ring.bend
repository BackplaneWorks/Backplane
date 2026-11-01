import Base
import ./json.bend as J

# The Pebble Index ring's webhook body (docs/bots.md, "Webhooks"): a
# multipart/form-data body with a `transcription` part (the voice note as
# text), `recordedAt`, `client` and, for the app's test button, `test`.
# A part's value is the text between the blank line after its headers and
# the next boundary. Walks are tail calls (bodies run to 256 KB).

type Cut is Data:
  Cut{hit: Bool, rest: String}

def Ring.after.k(here: Bool, +s: String, +pat: String) -> Cut:
  match here:
    case True{}:
      Cut{True{}, String.drop(s, String.length(pat))}
    case False{}:
      Cut{False{}, s}

def Ring.after.go(s: String, +pat: String, c: Cut) -> Cut:
  match s c:
    case _ Cut{True{}, r}:
      Cut{True{}, r}
    case SNil{} Cut{False{}, _}:
      Cut{False{}, SNil{}}
    case SCon{_, +t} Cut{False{}, _}:
      Ring.after.go(t, pat, Ring.after.k(String.starts_with(t, pat), t, pat))

# what follows the first `pat` in s
def Ring.after(+s: String, +pat: String) -> Cut:
  Ring.after.go(s, pat, Ring.after.k(String.starts_with(s, pat), s, pat))

# the text read so far, and whether `pat` has come (the walk goes on to
# the end, a tail call each step, keeping nothing more)
type Us is Data:
  Us{done: Bool, acc: String}

def Ring.until.step(stop: Bool, +c: Char, u: Us) -> Us:
  match stop u:
    case _ Us{True{}, a}:
      Us{True{}, a}
    case True{} Us{False{}, a}:
      Us{True{}, a}
    case False{} Us{False{}, a}:
      Us{False{}, SCon{c, a}}

# whether pat starts at c (then t): its first character first, then the
# rest of it against t. Asking String.starts_with of SCon{c, t} built a
# string as long as the rest of the body at every character (quadratic,
# like ref-origin.bend's Origin.scan was)
def Ring.at.k(same: Bool, +t: String, +rest: String) -> Bool:
  match same:
    case True{}: String.starts_with(t, rest)
    case False{}: False{}

def Ring.at(+c: Char, +t: String, pat: String) -> Bool:
  match pat:
    case SNil{}: True{}
    case SCon{p, rest}: Ring.at.k(Char.is_eq(c, p), t, rest)

def Ring.until.go(s: String, +pat: String, u: Us) -> String:
  match s u:
    case SNil{} Us{_, a}:
      String.reverse(a)
    case SCon{+c, +t} _:
      Ring.until.go(t, pat, Ring.until.step(Ring.at(c, t, pat), c, u))

# the text of s up to `pat` (all of it when pat never comes)
def Ring.until(s: String, +pat: String) -> String:
  Ring.until.go(s, pat, Us{False{}, SNil{}})

def Ring.value(c: Cut) -> String:
  match c:
    case Cut{True{}, r}:
      Ring.until(r, "\r\n--")
    case Cut{False{}, _}:
      ""

def Ring.body(c: Cut) -> String:
  match c:
    case Cut{True{}, r}:
      Ring.value(Ring.after(r, "\r\n\r\n"))
    case Cut{False{}, _}:
      ""

# a part's text ("" when the body has no such part)
def Ring.part(+body: String, +name: String) -> String:
  Ring.body(Ring.after(body, "name=\"" ++ name ++ "\""))

# the voice note's words
def Ring.text(+body: String) -> String:
  String.trim(Ring.part(body, "transcription"))

# the words to clarify, when Backplane transcribed the recording itself
# (server/voice.bend puts an `unsure` part first)
def Ring.unsure(+body: String) -> String:
  String.trim(Ring.part(body, "unsure"))

# the model that heard the recording ("" when the ring's own words came)
def Ring.heard(+body: String) -> String:
  String.trim(Ring.part(body, "heard"))

# the app's "Send test event"
def Ring.test(+body: String) -> Bool:
  String.eq(String.trim(Ring.part(body, "test")), "true")

# The same body under a bearer token (docs/bots.md, "A Pebble Index
# ring"): its text fields as a JSON object for the bot, which reads it as
# untrusted data like any webhook's. The recording stays out: a bot
# cannot hear M4A bytes, so it gets a note with their size
# (X-Audio-Size) in its place.

def Ring.is_form(+ctype: String) -> Bool:
  String.starts_with(String.trim(ctype), "multipart/form-data")

def Ring.kv(+k: String, +v: String, +rest: List<&2, J.KV>) -> List<&2, J.KV>:
  Bool.pick(List<&2, J.KV>, String.is_empty(v), rest, J.KV{k, J.Str{v}} <> rest)

def Ring.num.k(m: Maybe<&2, Nat>, +k: String, +v: String, +rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match m:
    case Some{n}:
      J.KV{k, J.Num{Nat.show(n)}} <> rest
    case None{}:
      Ring.kv(k, v, rest)

# recordedAt (ms since the epoch) as a number when it is one
def Ring.num(+k: String, +v: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  Ring.num.k(Nat.read(v), k, v, rest)

def Ring.audio(has: Bool, +size: String) -> String:
  match has:
    case True{}:
      Bool.pick(String, String.is_empty(size), "omitted", "omitted, " ++ size ++ " bytes")
    case False{}:
      ""

def Ring.source(+client: String) -> String:
  Bool.pick(String, String.eq(client, "ring"), "pebble-index-01", "form-data")

# the form's fields as JSON text; `size` is X-Audio-Size ("" when absent)
def Ring.form(+body: String, +size: String) -> String:
  J.Json.show(J.Json.obj(J.KV{"source", J.Str{Ring.source(String.trim(Ring.part(body, "client")))}} <>
    Ring.kv("transcription", Ring.text(body),
      Ring.num("recordedAt", String.trim(Ring.part(body, "recordedAt")),
        Ring.kv("client", String.trim(Ring.part(body, "client")),
          Ring.kv("test", Bool.pick(String, Ring.test(body), "true", ""),
            Ring.kv("heardBy", Ring.heard(body), Ring.kv("unsure", Ring.unsure(body),
              Ring.kv("audio", Ring.audio(String.contains(body, "name=\"audio\""), String.trim(size)), Nil{})))))))))
