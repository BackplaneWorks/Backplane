import Base
import ./check.bend as C
import ../src/core/client.bend as U
# opencode's models: kept apart from the models info every client parses
# (Info.put), searched by id and name (Models.shown), and cut to those shown
# for their labels (Models.view)

def info() -> String:
  "{\"claude\":[\"opus\",\"sonnet\"],\"opencode\":[\"opencode/beam-free\",\"openrouter/x-ai/grok-4.5\",\"openrouter/anthropic/claude-sonnet-5\"],\"about\":{\"claude\":{\"cli\":\"2.1\",\"labels\":{\"opus\":\"Opus\"}},\"opencode\":{\"cli\":\"1.18\",\"labels\":{\"default\":\"its default\",\"opencode/beam-free\":\"Beam Free\",\"openrouter/x-ai/grok-4.5\":\"Grok 4.5\",\"openrouter/anthropic/claude-sonnet-5\":\"Claude Sonnet 5\"}}}}"

def nf() -> Map<&2, String>:
  U.Info.put(Map.new(&2, String), "models", info())

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("small keeps claude", String.join(U.Models.list(U.Str.get(nf(), "models"), "claude"), ","), "opus,sonnet")
    C.Check.str("small has opencode's cli", U.Models.cli(U.Str.get(nf(), "models"), "opencode"), "1.18")
    C.Check.str("oc has all", Nat.show(List.length(&2, String, U.Models.list(U.Models.of(nf(), "opencode"), "opencode"))), "3")
    C.Check.str("search by name", String.join(U.Models.shown(U.Models.of(nf(), "opencode"), "opencode", "grok"), ","), "openrouter/x-ai/grok-4.5")
    C.Check.str("two words", String.join(U.Models.shown(U.Models.of(nf(), "opencode"), "opencode", "Sonnet anthropic"), ","), "openrouter/anthropic/claude-sonnet-5")
    C.Check.str("empty shows all", Nat.show(List.length(&2, String, U.Models.shown(U.Models.of(nf(), "opencode"), "opencode", ""))), "4")
    C.Check.str("view label", U.Models.label_of(U.Models.view(U.Models.of(nf(), "opencode"), "opencode", ["openrouter/x-ai/grok-4.5"]), "opencode", "openrouter/x-ai/grok-4.5"), "Grok 4.5")
