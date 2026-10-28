import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/wnote.bend as WN
import ../src/core/hub.bend as H
import ../src/core/secret.bend as S

# Small synthetic diagnostics exercise the bounded classifier without a
# long-history/resource-exhaustion probe.
def padding() -> String:
  String.join(List.replicate(String, 32n, "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"), "")

def outtext(os: List<&2, H.Out>) -> String:
  match os:
    case H.TurnEnded{_, _, _, e, _, x} <> _: e ++ " " ++ x
    case _ <> t: outtext(t)
    case Nil{}: ""

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("success needs no diagnostic classification", WN.Wn.reason("completed", "usage limit reached"), "")
    C.Check.str("stop needs no diagnostic classification", WN.Wn.reason("stopped", "usage limit reached"), "")
    C.Check.str("limit near diagnostic start", WN.Wn.reason("failed", "USAGE LIMIT REACHED " ++ padding()), "limit")
    C.Check.str("unbounded reply tail is not a diagnostic", WN.Wn.reason("failed", padding() ++ " usage limit reached"), "error")
    C.Check.str("stat permission error stays closed", Nat.show(WN.Wn.mode(WN.Wn.wants("", WN.Wn.seen.fail(13)), "")), "0")
    C.Check.str("stat IO error stays closed", Nat.show(WN.Wn.mode(WN.Wn.wants("", WN.Wn.seen.fail(5)), "")), "0")
    C.Check.str("POSIX missing default is unsigned", Nat.show(WN.Wn.mode(WN.Wn.wants("", WN.Wn.seen.fail(2)), "")), "1")
    C.Check.str("Windows missing default is unsigned", Nat.show(WN.Wn.mode(WN.Wn.wants("", WN.Wn.seen.fail(4058)), "")), "1")
    C.Check.str("Windows access denied stays closed", Nat.show(WN.Wn.mode(WN.Wn.wants("", WN.Wn.seen.fail(4092)), "")), "0")
    +entries : List<&2, M.Entry> = [M.Act{"current-error", "t1", "error", "api", "current model error", 4n}, M.Msg{"queued", "t1", M.User{}, "next task", 3n}, M.Msg{"start", "t1", M.User{}, "first task", 2n}, M.Act{"previous-error", "t1", "error", "api", "old error", 1n}]
    C.Check.str("queued user is not the start boundary", WN.Wn.err(entries, "@start", "t1"), "current model error")
    C.Check.str("wake boundary excludes old error", WN.Wn.err(M.Act{"previous-error", "t1", "error", "api", "old error", 1n} <> Nil{}, "@previous-error", "t1"), "")
    +plain : String = "SYNTHETIC-VAULT-PHRASE-459"
    +outs : List<&2, H.Out> = H.Secret.outs(S.Sec.vault([plain]), [H.TurnEnded{"t1", "failed", "fail", "stderr echoed " ++ plain, "error", "reply echoed " ++ plain}])
    C.Check.str("unlabelled vault value omitted from captured error and reply", Bool.show(String.contains(outtext(outs), plain)), "False")
    C.Check.str("masked diagnostic keeps its context", Bool.show(String.contains(outtext(outs), "stderr echoed")), "True")
