import Base
import ./json.bend as J

# Mail that rides a running turn's tool results (pure)
# ====================================================
# A Codex, Grok, Muse or Antigravity turn cannot take input while it
# runs. A message for one (from a bot, another thread, a wake, a task)
# is posted at once and waits as the thread's ride, the setting
# "ride.<thread>" (persisted like any setting, so a restart neither loses
# nor repeats it). The turn's next Backplane tool result carries every
# waiting message as an "incoming messages" block and the ride is
# emptied in the same step, so each message is delivered once. A turn
# that ends with a ride waiting starts the next turn with it at once.
# (The person's explicit steer still stops the turn and starts it again:
# hub.bend's Steer.go.)

type RItem is Data:
  RItem{id: String, text: String}

def Ride.key(+thread: String) -> String:
  "ride." ++ thread

def RItem.id(+x: RItem) -> String:
  match x:
    case RItem{+v, _}:
      v

def RItem.text(+x: RItem) -> String:
  match x:
    case RItem{_, +v}:
      v

def Ride.one(+j: J.Json) -> RItem:
  RItem{J.Json.get_str(j, "id", ""), J.Json.get_str(j, "text", "")}

def Ride.items(j: J.Json) -> List<&2, RItem>:
  match j:
    case J.Item{+h, t}:
      Ride.one(h) <> Ride.items(t)
    case _:
      Nil{}

def Ride.arr(j: J.Json) -> List<&2, RItem>:
  match j:
    case J.Arr{items}:
      Ride.items(items)
    case _:
      Nil{}

def Ride.parsed(m: Maybe<&2, J.Json>) -> List<&2, RItem>:
  match m:
    case Some{j}:
      Ride.arr(j)
    case None{}:
      Nil{}

def Ride.read.k(empty: Bool, +v: String) -> List<&2, RItem>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      Ride.parsed(J.Json.parse(v))

# the messages a ride's setting holds, oldest first
def Ride.read(+v: String) -> List<&2, RItem>:
  Ride.read.k(String.is_empty(v), v)

def Ride.json(+x: RItem) -> J.Json:
  J.Json.obj([J.KV{"id", J.Str{RItem.id(x)}}, J.KV{"text", J.Str{RItem.text(x)}}])

def Ride.jsons(xs: List<&2, RItem>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Ride.json(x) <> Ride.jsons(t)

def Ride.show.k(empty: Bool, +xs: List<&2, RItem>) -> String:
  match empty:
    case True{}:
      ""
    case False{}:
      J.Json.show(J.Json.arr(Ride.jsons(xs)))

# a ride's setting (empty when nothing waits)
def Ride.show(+xs: List<&2, RItem>) -> String:
  Ride.show.k(List.is_empty(&2, RItem, xs), xs)

def Ride.has.go(xs: List<&2, RItem>, +id: String, found: Bool) -> Bool:
  match xs found:
    case _ True{}:
      True{}
    case Nil{} False{}:
      False{}
    case +x <> t False{}:
      Ride.has.go(t, id, String.eq(RItem.id(x), id))

def Ride.has(xs: List<&2, RItem>, +id: String) -> Bool:
  Ride.has.go(xs, id, False{})

def Ride.add.k(dup: Bool, +xs: List<&2, RItem>, +id: String, +text: String) -> List<&2, RItem>:
  match dup:
    case True{}:
      xs
    case False{}:
      List.append(&2, RItem, xs, [RItem{id, text}])

# a message added to a ride's setting (once: a message already there is
# not added again)
def Ride.add(+v: String, +id: String, +text: String) -> String:
  Ride.show(Ride.add.k(Ride.has(Ride.read(v), id), Ride.read(v), id, text))

def Ride.texts(xs: List<&2, RItem>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      RItem.text(x) <> Ride.texts(t)

def Ride.ids(xs: List<&2, RItem>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      RItem.id(x) <> Ride.ids(t)

# the messages as one turn's input (joined as queued messages always were)
def Ride.input(+v: String) -> String:
  String.join(Ride.texts(Ride.read(v)), "\n\n")

# the block a tool result carries
def Ride.block(+v: String) -> String:
  "[Backplane] Incoming messages, sent to you while this turn ran. Each is shown to you once; take them into account now:\n\n" ++
    String.join(Ride.texts(Ride.read(v)), "\n\n---\n\n")

def Ride.content.end(+block: String, items: J.Json) -> J.Json:
  match items:
    case J.Item{h, t}:
      J.Item{h, Ride.content.end(block, t)}
    case _:
      J.Item{J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{block}}]), J.End{}}

# a tools/call result's content with the block added after it
def Ride.content(+block: String, c: J.Json) -> J.Json:
  match c:
    case J.Arr{items}:
      J.Arr{Ride.content.end(block, items)}
    case _:
      J.Json.arr([J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{block}}])])

# whether an object's first field is named key
def Ride.fhead(+fs: J.Json, +key: String) -> Bool:
  match fs:
    case J.Field{+k, _, _}:
      String.eq(k, key)
    case _:
      False{}

# fields with the one named "content" given the block (hit: the first is it)
def Ride.fields(fs: J.Json, +block: String, hit: Bool) -> J.Json:
  match fs hit:
    case J.Field{k, v, t} True{}:
      J.Field{k, Ride.content(block, v), t}
    case J.Field{k, v, +t} False{}:
      J.Field{k, v, Ride.fields(t, block, Ride.fhead(t, "content"))}
    case +o _:
      o

# a result object with the block added to its content
def Ride.result(+block: String, r: J.Json) -> J.Json:
  match r:
    case J.Obj{+fs}:
      J.Obj{Ride.fields(fs, block, Ride.fhead(fs, "content"))}
    case _:
      r

# fields with the "result" one's content given the block
def Ride.reply.fs(fs: J.Json, +block: String, hit: Bool) -> J.Json:
  match fs hit:
    case J.Field{k, v, t} True{}:
      J.Field{k, Ride.result(block, v), t}
    case J.Field{k, v, +t} False{}:
      J.Field{k, v, Ride.reply.fs(t, block, Ride.fhead(t, "result"))}
    case +o _:
      o

def Ride.reply.j(+block: String, j: J.Json) -> J.Json:
  match j:
    case J.Obj{+fs}:
      J.Obj{Ride.reply.fs(fs, block, Ride.fhead(fs, "result"))}
    case _:
      j

def Ride.takes.c(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{J.Arr{_}}:
      True{}
    case _:
      False{}

def Ride.takes.m(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{+j}:
      Ride.takes.c(J.Json.get(J.Json.get_obj(j, "result"), "content"))
    case None{}:
      False{}

# whether a JSON-RPC reply is a tools/call result with a content list (an
# error, or a reply without content, carries nothing)
def Ride.takes(+reply: String) -> Bool:
  Ride.takes.m(J.Json.parse(reply))

def Ride.carry.m(+block: String, m: Maybe<&2, J.Json>, +reply: String) -> String:
  match m:
    case Some{j}:
      J.Json.show(Ride.reply.j(block, j))
    case None{}:
      reply

# the reply with the ride's block added to its result's content
def Ride.carry(+v: String, +reply: String) -> String:
  Ride.carry.m(Ride.block(v), J.Json.parse(reply), reply)
