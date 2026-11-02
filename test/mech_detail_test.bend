import Base
import ./check.bend as C
import ../src/core/mech.bend as MC

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a kind without a level is that kind", MC.Mech.detail.kind("2"), "2")
    C.Check.str("a kind without a level is normal", MC.Mech.detail.at("2"), "normal")
    C.Check.str("a kind's level", MC.Mech.detail.at("2@high"), "high")
    C.Check.str("a kind's kind before its level", MC.Mech.detail.kind("3@max"), "3")
    C.Check.str("an unknown level is normal", MC.Mech.detail.at("2@ultra"), "normal")
    C.Check.str("low meshes as before", String.join(MC.Mech.detail.args("low"), " "), "--linear 0.1 --angular 0.5")
    C.Check.str("normal is finer", String.join(MC.Mech.detail.args("normal"), " "), "--linear 0.03 --angular 0.2")
    C.Check.str("an unknown level meshes as normal", String.join(MC.Mech.detail.args("x"), " "), "--linear 0.03 --angular 0.2")
    C.Check.str("max is the finest", String.join(MC.Mech.detail.args("max"), " "), "--linear 0.01 --angular 0.05")
