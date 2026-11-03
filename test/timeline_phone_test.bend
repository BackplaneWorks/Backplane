import Base
import ./check.bend as C
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/mobile/screen.bend as S

# The phones draw the timeline's failure and restart rows as the web does.
# A provider's API error reply: its blocks
# the one sentence (core/fold.bend's ApiErr.say), its text (Copy) the raw
# reply; any other reply is unchanged.

def raw() -> String:
  "API Error: 529 {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}"

def shown(+text: String) -> String:
  J.Json.show(S.Mob.own.s.loc(I.En{}, M.Msg{"m", "t", M.Assistant{}, text, 1n}, Map.new(&2, String), "Failed 00:02"))

def has(+s: String, +x: String) -> String:
  Bool.pick(String, String.contains(s, x), "yes", "no")

def own(e: M.Entry) -> String:
  J.Json.show(S.Mob.own.s.loc(I.En{}, e, Map.new(&2, String), "06:04"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("its blocks say the sentence", has(shown(raw()), "Claude's API was overloaded (529) and ended the turn."), "yes")
    C.Check.str("its text is the raw reply", has(shown(raw()), "\"text\":" ++ J.Json.show(J.Json.str(raw()))), "yes")
    C.Check.str("another reply's blocks are its own", has(shown("All done"), "Claude's API"), "no")
    C.Check.str("a restart's continue message is the Continuing row", own(M.Msg{"c1", "t", M.User{}, M.Carry.text(), 1n}),
      "{\"id\":\"c1\",\"kind\":\"act\",\"tone\":\"info restart\",\"label\":\"restart\",\"text\":\"Continuing after the restart\"}")
    C.Check.str("a restart row is not red", has(own(M.Act{"r0", "t", "error", "restart", "Backplane restarted at 06:04 during this turn.", 1n}), "\"tone\":\"info restart\""), "yes")
    C.Check.str("a person's message stays theirs", has(own(M.Msg{"u1", "t", M.User{}, "go on", 1n}), "\"kind\":\"user\""), "yes")
