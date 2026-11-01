import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/core/client.bend as U
import ../src/mobile/session.bend as S
import ../src/mobile/screen.bend as SC

# Dictation on the phones: live words go into the draft the phone owns
# (session.bend's Session.voice, bridge.js's voiceHeard), by the same
# client.bend Voice.recv the hub and the web run; the screen's "voice"
# object (screen.bend's Mob.voice) is the web's strip, fixes and question.

def words(+commit: String, +tail: String, first: Bool, fin: Bool) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"voice"}}, J.KV{"thread", J.Str{"t1"}}, J.KV{"ev", J.Str{"words"}}, J.KV{"first", J.Flag{first}},
    J.KV{"commit", J.Str{commit}}, J.KV{"tail", J.Str{tail}}, J.KV{"firm", J.Flag{False{}}}, J.KV{"final", J.Flag{fin}},
    J.KV{"fixes", J.Arr{J.End{}}}, J.KV{"unsure", J.Arr{J.End{}}}])

def ev(+e: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"voice"}}, J.KV{"thread", J.Str{"t1"}}, J.KV{"ev", J.Str{e}}])

# one event after another, each on the draft and place the one before left
def step(+r: J.Json, +o: J.Json) -> J.Json:
  S.Session.voice(J.Json.get_obj(r, "keep"), J.Json.get_str(r, "draft", ""), o)

def start(+draft: String) -> J.Json:
  J.Json.obj([J.KV{"draft", J.Str{draft}}, J.KV{"keep", J.Json.obj(Nil{})}])

def draft(+r: J.Json) -> String:
  J.Json.get_str(r, "draft", "")

def ui.go(kvs: List<&2, J.KV>, u: U.Ui) -> U.Ui:
  match kvs:
    case Nil{}: u
    case J.KV{+k, +v} <> t: ui.go(t, U.Ui.scratch_set(u, k, J.Json.as_str(Some{v}, "")))

def ui(+kvs: List<&2, J.KV>) -> U.Ui:
  ui.go(kvs, U.Ui.init(0n))

def voice(+u: U.Ui) -> J.Json:
  SC.Mob.voice(I.En{}, u, "t1")

def main() -> IO(Unit):
  do IO<Unit>:
    +live : J.Json = step(start("Note:"), ev("live"))
    +a : J.Json = step(live, words("", "hello", True{}, False{}))
    C.Check.str("a tail shows at the end of the draft", draft(a), "Note: hello")
    +b : J.Json = step(a, words("hello world", "and", True{}, False{}))
    C.Check.str("committed words replace the tail; the new tail follows", draft(b), "Note: hello world and")
    +c : J.Json = step(b, words("and more", "", False{}, True{}))
    C.Check.str("later words read on from the committed ones", draft(c), "Note: hello world and more")
    C.Check.str("a level changes nothing in the draft", draft(step(c, J.Json.obj([J.KV{"t", J.Str{"voice"}}, J.KV{"thread", J.Str{"t1"}}, J.KV{"ev", J.Str{"level"}}, J.KV{"v", J.Json.nat(40n)}]))), draft(c))
    +d : J.Json = step(step(c, ev("idle")), ev("live"))
    C.Check.str("a new session starts where the draft ends", draft(step(d, words("", "next", True{}, False{}))), "Note: hello world and more next")
    C.Check.str("no session, no strip", J.Json.show(J.Json.get_obj(voice(ui(Nil{})), "on")), "false")
    C.Check.str("listening: one word, the level",
      J.Json.get_str(voice(ui([J.KV{"@voice", J.Str{"live"}}, J.KV{"@voice.th", J.Str{"t1"}}, J.KV{"@voice.lv", J.Str{"35"}}, J.KV{"@voice.eng", J.Str{"cloud"}}])), "status", "") ++ " " ++
      J.Json.show(J.Json.get_obj(voice(ui([J.KV{"@voice", J.Str{"live"}}, J.KV{"@voice.th", J.Str{"t1"}}, J.KV{"@voice.lv", J.Str{"35"}}])), "level")), "Listening 35")
    C.Check.str("words settling: Writing, never the words",
      J.Json.get_str(voice(ui([J.KV{"@voice", J.Str{"live"}}, J.KV{"@voice.th", J.Str{"t1"}}, J.KV{"@voice.eng", J.Str{"local"}}, J.KV{"@voice.load", J.Str{"900"}}, J.KV{"@voice.part", J.Str{"secret words"}}])), "status", ""), "Writing")
    C.Check.str("another thread's session shows no strip here",
      J.Json.show(J.Json.get_obj(voice(ui([J.KV{"@voice", J.Str{"live"}}, J.KV{"@voice.th", J.Str{"t2"}}])), "on")), "false")
    C.Check.str("a fix is a chip that undoes it",
      J.Json.show(J.Json.get_obj(voice(ui([J.KV{"@voice.uth", J.Str{"t1"}}, J.KV{"@voice.fx", J.Str{"[{\"heard\":\"key cad\",\"out\":\"KiCad\"}]"}}])), "fixes")),
      "[{\"label\":\"KiCad ← key cad ↶\",\"tip\":\"Put back what was heard (key cad), and stop fixing it this way\",\"action\":\"voice-unterm\",\"value\":\"#0\"}]")
    C.Check.str("an error is a bad note",
      J.Json.get_str(voice(ui([J.KV{"@voice.err", J.Str{"no model"}}])), "note", ""), "Dictation: no model")
