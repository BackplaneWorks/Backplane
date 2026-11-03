import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/hub.bend as B

# One read of Claude's stdout holds many lines; a run of text deltas to one
# block among them goes to the clients as one delta (B.Delta.merge), and
# everything else stays as it was, in its place.

def delta(+i: String, +text: String) -> String:
  "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":" ++ i ++ ",\"delta\":{\"type\":\"text_delta\",\"text\":" ++ J.Json.show(J.Str{text}) ++ "}},\"session_id\":\"s\",\"parent_tool_use_id\":null}"

def sub(+text: String) -> String:
  "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":" ++ J.Json.show(J.Str{text}) ++ "}},\"parent_tool_use_id\":\"toolu_1\"}"

def start() -> String:
  "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}}"

def other() -> String:
  "{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"content\":[]}}"

# what a line is, shortly: a delta's text in brackets, anything else by its type
def show1(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{+o}:
      Bool.pick(String, String.eq(J.Json.get_str(J.Json.get_obj(J.Json.get_obj(o, "event"), "delta"), "type", ""), "text_delta"),
        "[" ++ J.Json.get_str(J.Json.get_obj(J.Json.get_obj(o, "event"), "delta"), "text", "") ++ Bool.pick(String, String.is_empty(J.Json.get_str(o, "parent_tool_use_id", "")), "", " sub") ++ "]",
        J.Json.get_str(o, "type", "?"))
    case None{}:
      "bad"

def show(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}:
      ""
    case l <> t:
      show1(J.Json.parse(l)) ++ " " ++ show(t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("stream pause stays within200ms latency budget", Bool.show(U32.is_le(B.Delta.pace(), 200)), "True")
    C.Check.str("stream pause coalesces at least100ms", Bool.show(U32.is_ge(B.Delta.pace(), 100)), "True")
    C.Check.str("a run is one delta", show(B.Delta.merge([delta("0", "Hel"), delta("0", "lo, "), delta("0", "wörld \"q\"\n")])), "[Hello, wörld \"q\"\n] ")
    C.Check.str("one delta stays one", show(B.Delta.merge([delta("0", "a")])), "[a] ")
    C.Check.str("other lines keep their place", show(B.Delta.merge([other(), delta("0", "a"), delta("0", "b"), start(), delta("1", "c"), delta("1", "d"), other()])),
      "assistant [ab] stream_event [cd] assistant ")
    C.Check.str("another block starts a run", show(B.Delta.merge([delta("0", "a"), delta("1", "b"), delta("1", "c")])), "[a] [bc] ")
    C.Check.str("a subagent's deltas pass as they are", show(B.Delta.merge([delta("0", "a"), sub("x"), sub("y"), delta("0", "b")])), "[a] [x sub] [y sub] [b] ")
    C.Check.str("nothing else changes", String.join(B.Delta.merge([other(), "not json", start(), sub("x")]), "\n"), String.join([other(), "not json", start(), sub("x")], "\n"))
    C.Check.str("none is none", show(B.Delta.merge([])), "")
    # a read that ends mid-stream waits a moment before the next
    C.Check.str("ends in text: waits", Bool.show(B.Delta.streaming([other(), delta("0", "a")])), "True")
    C.Check.str("ends in anything else: reads on", Bool.show(B.Delta.streaming([delta("0", "a"), other()])), "False")
    C.Check.str("nothing read: reads on", Bool.show(B.Delta.streaming([])), "False")
