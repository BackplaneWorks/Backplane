import Base
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J

# An observation of the real client, not a second draft model. Scratch and
# all conversation drafts share this map, so equality preserves every byte.
def drafts(u: U.Ui) -> Map<&2, String>:
  match u:
    case U.Ui{_, _, dr, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: dr

def selection(u: U.Ui, +id: String) -> {U.Ui.sel(U.Ui.select.nav(u, id)) == id : String}:
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: {==}

def error_drafts(+u: U.Ui, error: String) -> {drafts(U.Ui.error_put(u, error)) == drafts(u) : Map<&2, String>}:
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: {==}

def adoption_drafts(+u: U.Ui, state: M.State) -> {drafts(U.Ui.adopt.raw(u, state)) == drafts(u) : Map<&2, String>}:
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: {==}

# Lex.unit matches the pair of flags. Split the remaining flag explicitly;
# the first flag being True is sufficient in both reachable cases.
def high_unit(code: U32, acc: String, lo: Bool) -> {J.Lex.unit(True{}, lo, code, acc) == J.LHi{acc, code} : J.Lex}:
  match lo:
    case False{}: {==}
    case True{}: {==}
