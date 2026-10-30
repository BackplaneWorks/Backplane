import Base
import ../../src/server/server.bend as S
import ../../src/server/kicad.bend as K
import ../../src/core/model.bend as M
import ../../src/core/prompt.bend as Q
import ../../src/core/hub.bend as B
import ../../src/core/bothub.bend as BH
import ../../src/server/botnet.bend as BN

def cfg() -> S.Cfg:
  S.Cfg{"/tmp/lang-test", "", "127.0.0.1", 3799, "recorded-system", [], "test",
    S.Net{False{}, False{}, "", "", "models", K.Rt.empty(), None{}, None{}, None{}, "", None{}, BN.Bn.empty(), False{}, "", "", None{}}, Nil{}, Nil{}, ""}

# The prompt as composed without the language line, from the same parts
# Hub.prompt uses, with no agent.lang setting: Hub.prompt with "off" must
# equal it byte for byte. Computed, not a frozen string, so other prompt
# edits don't break it.
def recorded() -> String:
  S.Hub.prompt.base(cfg(), M.State.empty(), "t", "/tmp/lang-project", K.Files{"", "", "", ""}, "")
