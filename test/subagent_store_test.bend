import Base
import ../src/core/model.bend as M
import ../src/core/subs.bend as SB
import ../src/core/storedb.bend as S
import ../src/server/subagent-history.bend as H
import ./subagent_history_test.bend as F

# The driver runs these exact production reads on SQLite, then folds the
# selected records through the same private inspector projection as the hub.
def sql() -> String:
  H.SubHistory.sql()

def schema() -> String:
  S.Sdb.schema()

def limit() -> U32:
  H.SubHistory.limit()

def state() -> M.State:
  M.State.replay([M.ThreadCreated{"t1", "p", "Selected", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"other", "p", "Other", M.Local{}, "", "", "claude", "", 1n},
    M.MessagePosted{"t1", "m", M.User{}, "Keep chat", 2n}], M.State.empty())

def from(rows: List<&2, List<&2, String>>) -> M.State:
  H.SubHistory.attach(state(), "t1", rows)

def panel(rows: List<&2, List<&2, String>>) -> String:
  SB.Subs.panel.word(F.rows(from(rows), Map.new(&2, String)))

def detail(rows: List<&2, List<&2, String>>) -> String:
  F.fingerprint(F.find(F.rows(from(rows), F.expanded()), "c1"))

def chat(rows: List<&2, List<&2, String>>) -> String:
  Nat.show(List.length(&2, M.Entry, M.State.mine(from(rows), "t1")))

def foreign(rows: List<&2, List<&2, String>>) -> String:
  Nat.show(List.length(&2, M.Entry, M.State.mine(from(rows), M.Bg.log("other"))))

def repeat(+rows: List<&2, List<&2, String>>) -> String:
  SB.Subs.panel.word(F.rows(H.SubHistory.attach(from(rows), "t1", rows), Map.new(&2, String)))

def fallback() -> String:
  SB.Subs.panel.word(F.rows(H.SubHistory.result(True{}, F.finished(), "t1", Nil{}), Map.new(&2, String)))
