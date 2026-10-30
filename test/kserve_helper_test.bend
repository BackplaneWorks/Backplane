import Base
import ./check.bend as C
import ../src/server/kserve.bend as S

def missing(m: Maybe<&1, S.KMsg>) -> String:
  match m:
    case Some{S.KBad{_, why}}: why
    case _: "wrong message"

def main() -> IO(Unit):
  do IO<Unit>:
    +q : Chan(S.KMsg) <- Chan.new(S.KMsg, 4)
    +ev : Chan(String) <- Chan.new(String, 4)
    +hp : S.Hp = S.Hp{None{}, Nil{}, 0n, "/definitely-missing-backplane-kipc"}
    k : S.Ks = S.Ks{hp, Nil{}, Nil{}, "", 1, "/tmp", "", S.KEnv{"/unused-kicad", "", Nil{}, ""}, q, ev}
    out : S.Ks <- S.KServe.launch(k, "project")
    Chan.close(S.KMsg, q)
    Chan.close(String, ev)
    m : Maybe<&1, S.KMsg> <- Chan.recv(S.KMsg, q)
    C.Check.str("missing helper fails before KiCad launch or timeout", missing(m), "the KiCad IPC helper was not found or is not executable: /definitely-missing-backplane-kipc")
    C.Check.str("failed helper has no input channel", Bool.show(S.Hp.started(S.Ks.hp(out))), "False")
