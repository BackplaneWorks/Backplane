import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/sign.bend as G
import ../src/core/ring.bend as RG
import ../src/core/utf8.bend as T
import ../src/core/voice.bend as V

# Dictation in the client (client.bend's Voice): a session's messages, a
# segment into the draft, the words to clarify taken, fixed, added or
# skipped; the ring's form with Backplane's own words first; and the ring's
# signature over a body's bytes (a recording is not text).

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def sent.go(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> r:
      t ++ "\n" ++ sent.go(r)
    case U.Focus{f} <> r:
      "focus " ++ f ++ "\n" ++ sent.go(r)
    case _ <> r:
      sent.go(r)

def sent(a: U.Act) -> String:
  sent.go(U.Act.cmds(a))

def recv(+u: U.Ui, +text: String) -> U.Ui:
  U.Ui.recv(u, Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))

def act(+u: U.Ui, +a: String, +v: String) -> U.Ui:
  ui(U.Ui.act(u, a, v))

def yn(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def start() -> U.Ui:
  act(U.Ui.init(0n), "select", "t1")

def live() -> U.Ui:
  recv(act(start(), "voice", ""), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"live\",\"recorder\":\"pw-record\",\"device\":\"alsa_input.usb-HyperX\",\"label\":\"HyperX\"}")

def heard() -> U.Ui:
  recv(live(), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"text\",\"text\":\"open key cad now\",\"unsure\":[{\"word\":\"key cad\",\"guess\":\"KiCad\"}]}")

def heard2() -> U.Ui:
  recv(act(heard(), "voice-use", "0"), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"text\",\"text\":\"check stm32f4\",\"unsure\":[{\"word\":\"stm32f4\",\"guess\":\"\"}]}")

def fixing() -> U.Ui:
  U.Ui.scratch_set(act(heard2(), "voice-fix", "0"), "@f.vfix", "STM32F4")

def zork(+u: U.Ui) -> U.Ui:
  recv(u, "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"text\",\"text\":\"zork\",\"unsure\":[{\"word\":\"ZORK\",\"guess\":\"\"}]}")

def status() -> U.Ui:
  recv(start(), "{\"t\":\"reply\",\"id\":1,\"ok\":true,\"voiceKey\":\"sk-…abcd\",\"voiceDict\":\"KiCad\\nSTM32F4\",\"voiceDevices\":\"{\\\"default\\\":\\\"hx\\\",\\\"devices\\\":[{\\\"name\\\":\\\"hx\\\",\\\"label\\\":\\\"HyperX\\\"},{\\\"name\\\":\\\"c922\\\",\\\"label\\\":\\\"C922\\\"}]}\"}")

# a ring form as server/voice.bend's Vo.ring.form leaves it: Backplane's
# words first, the ring's own after
def ring.form() -> String:
  "--backplane\r\nContent-Disposition: form-data; name=\"transcription\"\r\n\r\nopen key cad\r\n--backplane\r\nContent-Disposition: form-data; name=\"unsure\"\r\n\r\nPossibly misheard: \"key cad\" (KiCad?). Ask if unsure.\r\n--backplane\r\nContent-Disposition: form-data; name=\"heard\"\r\n\r\ngpt-live-transcribe\r\n" ++
  "--b\r\nContent-Disposition: form-data; name=\"transcription\"\r\n\r\nopen keycard\r\n--b\r\nContent-Disposition: form-data; name=\"client\"\r\n\r\nring\r\n--b\r\nContent-Disposition: form-data; name=\"audio\"; filename=\"a.m4a\"\r\nContent-Type: audio/mp4\r\n\r\nxx\r\n--b--\r\n"

def raw() -> List<&2, U32>:
  [45, 45, 120, 13, 10, 0, 255, 128, 1, 2, 3, 200, 10]

def raw.bad() -> List<&2, U32>:
  [45, 45, 120, 13, 10, 0, 255, 128, 1, 2, 3, 201, 10]

# a draft typed first, the window's caret after "abc"
def typed() -> U.Ui:
  U.Ui.scratch_set(ui(U.Ui.draft.set(live(), "abc def")), "@voice.at", "3")

def into(+u: U.Ui, +text: String) -> U.Ui:
  recv(u, "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"text\",\"text\":\"" ++ text ++ "\",\"unsure\":[]}")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("start asks the hub", sent(U.Ui.act(start(), "voice", "")), "{\"id\":0,\"m\":\"bots.voice\",\"p\":{\"op\":\"start\",\"thread\":\"t1\",\"device\":\"\"}}\n")
    C.Check.str("starting", U.Ui.scratch(act(start(), "voice", ""), "@voice"), "starting")
    C.Check.str("live", U.Ui.scratch(live(), "@voice"), "live")
    C.Check.str("device shown", U.Ui.scratch(live(), "@voice.dev"), "HyperX")
    C.Check.str("words so far", U.Ui.scratch(recv(recv(live(), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"part\",\"text\":\"hel\"}"), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"part\",\"text\":\"lo\"}"), "@voice.part"), "hello")
    C.Check.str("level", U.Ui.scratch(recv(live(), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"level\",\"v\":42}"), "@voice.lv"), "42")
    C.Check.str("nothing received", U.Ui.scratch(recv(live(), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"hint\",\"hint\":\"no-audio\"}"), "@voice.hint"), "no-audio")
    C.Check.str("heard at last", U.Ui.scratch(recv(recv(live(), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"hint\",\"hint\":\"silent\"}"), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"hint\",\"hint\":\"ok\"}"), "@voice.hint"), "")
    C.Check.str("stop asks the hub", sent(U.Ui.act(live(), "voice", "")), "{\"id\":1,\"m\":\"bots.voice\",\"p\":{\"op\":\"stop\",\"thread\":\"t1\"}}\n")
    C.Check.str("segment into the draft", U.Ui.draft_text(heard()), "open key cad now")
    C.Check.str("to clarify", Nat.show(List.length(&2, V.Unsure, U.Voice.unsure(heard()))), "1")
    C.Check.str("use the guess", U.Ui.draft_text(act(heard(), "voice-use", "0")), "open KiCad now")
    C.Check.str("used, gone", Nat.show(List.length(&2, V.Unsure, U.Voice.unsure(act(heard(), "voice-use", "0")))), "0")
    C.Check.str("next segment after a space", U.Ui.draft_text(heard2()), "open KiCad now check stm32f4")
    C.Check.str("fix opens the field", sent(U.Ui.act(heard2(), "voice-fix", "0")), "focus voice-fix\n")
    C.Check.str("fix starts from the word", U.Ui.scratch(act(heard2(), "voice-fix", "0"), "@f.vfix"), "stm32f4")
    C.Check.str("fixed in the draft", U.Ui.draft_text(act(fixing(), "voice-fixed", "")), "open KiCad now check STM32F4")
    C.Check.str("fixed joins the dictionary", sent(U.Ui.act(fixing(), "voice-fixed", "")), "{\"id\":1,\"m\":\"bots.voice\",\"p\":{\"op\":\"add\",\"term\":\"STM32F4\"}}\n")
    C.Check.str("fixed, gone", Nat.show(List.length(&2, V.Unsure, U.Voice.unsure(act(fixing(), "voice-fixed", "")))), "0")
    C.Check.str("add as heard", sent(U.Ui.act(heard2(), "voice-add", "0")), "{\"id\":1,\"m\":\"bots.voice\",\"p\":{\"op\":\"add\",\"term\":\"stm32f4\"}}\n")
    C.Check.str("skipped is not asked again", Nat.show(List.length(&2, V.Unsure, U.Voice.unsure(zork(act(zork(start()), "voice-skip", "0"))))), "0")
    C.Check.str("not skipped is asked once", Nat.show(List.length(&2, V.Unsure, U.Voice.unsure(zork(zork(start()))))), "1")
    C.Check.str("idle", U.Ui.scratch(recv(live(), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"idle\"}"), "@voice"), "")
    C.Check.str("error kept", U.Ui.scratch(recv(live(), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"error\",\"error\":\"no key\"}"), "@voice.err"), "no key")
    C.Check.str("error closed", U.Ui.scratch(act(recv(live(), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"error\",\"error\":\"no key\"}"), "voice-err", ""), "@voice.err"), "")
    C.Check.str("settings ask", sent(U.Ui.act(start(), "flag", "settings")), "{\"id\":0,\"m\":\"bots.voice\",\"p\":{\"op\":\"status\"}}\n{\"id\":1,\"m\":\"bots.voice\",\"p\":{\"op\":\"usage\",\"do\":\"read\"}}\n")
    C.Check.str("key masked", U.Ui.scratch(status(), "@voice.key"), "sk-…abcd")
    C.Check.str("dictionary", Nat.show(List.length(&2, String, U.Voice.dict(status()))), "2")
    C.Check.str("microphones", Nat.show(List.length(&2, U.VDev, U.Voice.devices(status()))), "2")
    C.Check.str("default microphone", U.Voice.default(status()), "hx")
    C.Check.str("take a term out", sent(U.Ui.act(status(), "voice-unterm", "kicad")), "{\"id\":0,\"m\":\"bots.voice\",\"p\":{\"op\":\"dict\",\"dict\":\"STM32F4\"}}\n")
    C.Check.str("ring: our words first", RG.Ring.text(ring.form()), "open key cad")
    C.Check.str("ring: words to clarify", RG.Ring.unsure(ring.form()), "Possibly misheard: \"key cad\" (KiCad?). Ask if unsure.")
    C.Check.str("ring: heard by", RG.Ring.heard(ring.form()), "gpt-live-transcribe")
    C.Check.str("ring: form for a bot", RG.Ring.form(ring.form(), "2"), "{\"source\":\"pebble-index-01\",\"transcription\":\"open key cad\",\"client\":\"ring\",\"heardBy\":\"gpt-live-transcribe\",\"unsure\":\"Possibly misheard: \\\"key cad\\\" (KiCad?). Ask if unsure.\",\"audio\":\"omitted, 2 bytes\"}")
    C.Check.str("ring signature over bytes (python hmac)", yn(G.Sign.index.raw("ring-secret", 1790000100n, 1790000000n, "rec-42", "single-click-hold", False{}, raw(), "d50898ea93f35e6f11f1941754a0c1b60512285f902bc5122610d4319d842944")), "yes")
    C.Check.str("ring signature, a byte changed", yn(G.Sign.index.raw("ring-secret", 1790000100n, 1790000000n, "rec-42", "single-click-hold", False{}, raw.bad(), "d50898ea93f35e6f11f1941754a0c1b60512285f902bc5122610d4319d842944")), "no")
    C.Check.str("ring signature over bytes, stale", yn(G.Sign.index.raw("ring-secret", 1790000301n, 1790000000n, "rec-42", "single-click-hold", False{}, raw(), "d50898ea93f35e6f11f1941754a0c1b60512285f902bc5122610d4319d842944")), "no")
    C.Check.str("text signature", yn(G.Sign.index("k", 1790000100n, 1790000000n, "d", "t", False{}, "body é", "c2e6a5e92be68124b8bf179e5aa0a67474d1650f893f846ce08d78a010e15941")), "yes")
    C.Check.str("the same over its bytes", yn(G.Sign.index.raw("k", 1790000100n, 1790000000n, "d", "t", False{}, T.Utf8.encode("body é"), "c2e6a5e92be68124b8bf179e5aa0a67474d1650f893f846ce08d78a010e15941")), "yes")
    C.Check.str("a 4-byte lead past U+10FFFF decodes as U+FFFD", T.Utf8.decode([104, 247, 191, 191, 191, 105]), "h\u{FFFD}i")
    C.Check.str("a surrogate decodes as U+FFFD", T.Utf8.decode([104, 237, 160, 128, 105]), "h\u{FFFD}i")
    C.Check.str("the last character still decodes", T.Utf8.decode([244, 143, 191, 191]), "\u{10FFFF}")
    C.Check.str("at the window's caret", U.Ui.draft_text(into(typed(), "Hello")), "abc hello def")
    C.Check.str("caret after it", U.Ui.scratch(into(typed(), "Hello"), "@voice.to"), "9")
    C.Check.str("the caret is used once", U.Ui.draft_text(into(into(typed(), "Hello"), "Again")), "abc hello def again")
    C.Check.str("after typed text, at the end", U.Ui.draft_text(into(ui(U.Ui.draft.set(live(), "fix the regulator")), "Then check it.")), "fix the regulator then check it.")
    C.Check.str("a dictionary term keeps its capital", U.Ui.draft_text(recv(ui(U.Ui.draft.set(live(), "open")), "{\"t\":\"voice\",\"thread\":\"t1\",\"ev\":\"text\",\"text\":\"Backplane\",\"firm\":true,\"unsure\":[]}")), "open Backplane")
