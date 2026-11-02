import Base
import ../src/core/fold.bend as F
import ../src/core/model.bend as M
import ../src/core/i18n.bend as I
import ./check.bend as C

# Preserve both pre-existing contracts alongside the new visible-call labels.
law singleton_legacy_label:
  for +id: String
  for +th: String
  for +kind: String
  for +summary: String
  for +at: Nat
  {F.Fold.roles([M.Act{id, th, "tool", kind, summary, at}], Map.new(&2, String), True{}) == [F.RHead{id, "1 tool call · running: " ++ kind, False{}, True{}}] : List<&2, F.Role>}

def singleton_legacy_label(id, th, kind, summary, at): {==}

law outside_page_label:
  for +id: String
  for +next: String
  for +th: String
  for +summary: String
  for +at: Nat
  {F.Fold.roles.ctx(I.En{}, F.Around{Nil{}, [M.Act{next, th, "tool", "Bash", summary, at}]}, [M.Act{id, th, "tool", "Bash", summary, at}], Map.new(&2, String), True{}) == [F.RHead{id, "2 tool calls · running: Bash", False{}, True{}}] : List<&2, F.Role>}

def outside_page_label(id, next, th, summary, at): {==}

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("legacy singleton tool-name contract proved", "yes", "yes")
    C.Check.str("outside-page tool-name contract proved", "yes", "yes")
    C.Check.str("visible singleton keeps summary", F.Fold.label.live(I.En{}, [M.Act{"a", "t", "tool", "Bash", "make check", 0n}]), "1 tool call · running: make check")
    C.Check.str("visible multi-call keeps latest summary", F.Fold.label.live(I.En{}, [M.Act{"a", "t", "tool", "Read", "board.kicad_pcb", 0n}, M.Act{"b", "t", "tool", "Bash", "make check", 0n}]), "2 tool calls · running: make check")
