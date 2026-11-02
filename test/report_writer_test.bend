import Base
import ./check.bend as C
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/json.bend as J

def thread() -> String: "improvement-0123456789abcdef0123456789abcdef"
def base() -> M.State:
  M.State.replay([M.ProjectCreated{"reports", "Reports", "/tmp/test-reports", 1n},
    M.ThreadCreated{thread(), "reports", "Report", M.Local{}, "", "/tmp/test-report", "codex", "", 1n},
    M.MessagePosted{thread(), "user", M.User{}, "PRIVATE input", 2n},
    M.MessagePosted{thread(), "answer", M.Assistant{}, "  Exact public template 日本語\n", 3n}], M.State.empty())
def done() -> M.State: M.State.apply(base(), M.TurnChanged{thread(), M.Completed{}, 4n})
def legacy() -> M.State:
  M.State.replay([M.ProjectCreated{"scratch", "Scratch", "/tmp/legacy-report", 1n},
    M.ThreadCreated{thread(), "scratch", "Report", M.Local{}, "", "/tmp/legacy-report", "codex", "", 1n},
    M.MessagePosted{thread(), "answer", M.Assistant{}, "Legacy completed report", 2n},
    M.TurnChanged{thread(), M.Completed{}, 3n}], M.State.empty())
def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("partial report is not a completed draft", J.Json.get_str(H.Improvements.report(base(), thread()), "body", ""), "")
    C.Check.str("completed body exact", J.Json.get_str(H.Improvements.report(done(), thread()), "body", ""), "  Exact public template 日本語\n")
    C.Check.str("no request echo", Bool.show(String.contains(J.Json.show(H.Improvements.report(done(), thread())), "PRIVATE")), "False")
    C.Check.str("only own report thread", J.Json.get_str(H.Improvements.report(done(), "other"), "error", ""), "Choose the report-writing thread first.")
    C.Check.str("legacy report still readable", J.Json.get_str(H.Improvements.report(legacy(), thread()), "body", ""), "Legacy completed report")
    C.Check.str("byte budget ASCII", Bool.show(H.Improvements.report.fits("abcd", 4n, True{})), "True")
    C.Check.str("byte budget Unicode", Bool.show(H.Improvements.report.fits("🐛", 3n, True{})), "False")
    C.Check.str("byte budget Unicode exact", Bool.show(H.Improvements.report.fits("🐛", 4n, True{})), "True")
    return Unit{}
