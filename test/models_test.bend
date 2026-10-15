import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/mobile/screen.bend as S

def strs(xs: List<&2, String>) -> String:
  String.join(xs, ",")

def info() -> String:
  "{\"claude\":[\"opus\",\"haiku\"],\"about\":{\"claude\":{\"cli\":\"2.1\",\"labels\":{\"opus\":\"Opus 5.5 · claude-opus-5-5\"},\"efforts\":{\"default\":[\"low\",\"high\",\"max\"],\"opus\":[\"low\",\"medium\",\"high\",\"xhigh\",\"max\"],\"haiku\":[]}}}}"

def omp_info() -> String:
  "{\"omp\":[\"m-think\",\"m-plain\"],\"about\":{\"omp\":{\"cli\":\"18.11.1\",\"labels\":{\"m-think\":\"Think\",\"m-plain\":\"Plain\"},\"efforts\":{\"default\":[],\"m-think\":[\"off\",\"auto\",\"minimal\",\"low\"],\"m-plain\":[]}}}}"

def push(+text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("opus chips", strs(U.Effort.chips(info(), "claude", "opus")), ",low,medium,high,xhigh,max")
    C.Check.str("haiku has none", strs(U.Effort.chips(info(), "claude", "haiku")), "")
    C.Check.str("default model", strs(U.Effort.chips(info(), "claude", "")), ",low,high,max")
    C.Check.str("unknown model: provider levels", strs(U.Effort.chips(info(), "claude", "sonnet")), ",low,medium,high,xhigh,max")
    C.Check.str("codex without efforts", strs(U.Effort.chips(info(), "codex", "")), ",low,medium,high,xhigh")
    C.Check.str("omp reasoning model chips", strs(U.Effort.chips(omp_info(), "omp", "m-think")), ",off,auto,minimal,low")
    C.Check.str("omp plain model has none", strs(U.Effort.chips(omp_info(), "omp", "m-plain")), "")
    C.Check.str("omp default model has none", strs(U.Effort.chips(omp_info(), "omp", "")), "")
    C.Check.str("label", U.Models.label_of(info(), "claude", "opus"), "Opus 5.5 · claude-opus-5-5")
    C.Check.str("info push replaces models",
      U.Ui.info(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), push("{\"t\":\"log\",\"items\":[],\"info\":{\"models\":\"old\",\"listen\":\"x\"}}")), push("{\"t\":\"info\",\"info\":{\"models\":\"new\"}}")), "models"), "new")
    C.Check.str("info push keeps the rest",
      U.Ui.info(U.Ui.recv(U.Ui.recv(U.Ui.init(0n), push("{\"t\":\"log\",\"items\":[],\"info\":{\"models\":\"old\",\"listen\":\"x\"}}")), push("{\"t\":\"info\",\"info\":{\"models\":\"new\"}}")), "listen"), "x")
    C.Check.str("phone picker", J.Json.show(S.Mob.picker(info(), "claude", "opus", "high")),
      "{\"label\":\"claude · opus · high\",\"models\":[{\"label\":\"default\",\"value\":\"\",\"on\":false},{\"label\":\"Opus 5.5 · claude-opus-5-5\",\"value\":\"opus\",\"on\":true},{\"label\":\"haiku\",\"value\":\"haiku\",\"on\":false}],\"efforts\":[{\"label\":\"auto\",\"value\":\"\",\"on\":false},{\"label\":\"low\",\"value\":\"low\",\"on\":false},{\"label\":\"medium\",\"value\":\"medium\",\"on\":false},{\"label\":\"high\",\"value\":\"high\",\"on\":true},{\"label\":\"xhigh\",\"value\":\"xhigh\",\"on\":false},{\"label\":\"max\",\"value\":\"max\",\"on\":false}],\"providers\":[{\"label\":\"claude\",\"value\":\"@claude\",\"on\":true},{\"label\":\"codex\",\"value\":\"@codex\",\"on\":false},{\"label\":\"grok\",\"value\":\"@grok\",\"on\":false},{\"label\":\"muse\",\"value\":\"@muse\",\"on\":false},{\"label\":\"antigravity\",\"value\":\"@antigravity\",\"on\":false},{\"label\":\"omp\",\"value\":\"@omp\",\"on\":false}]}")
    C.Check.str("phone picker, default model", J.Json.show(S.Mob.picker(info(), "claude", "", "")),
      "{\"label\":\"claude · default\",\"models\":[{\"label\":\"default\",\"value\":\"\",\"on\":true},{\"label\":\"Opus 5.5 · claude-opus-5-5\",\"value\":\"opus\",\"on\":false},{\"label\":\"haiku\",\"value\":\"haiku\",\"on\":false}],\"efforts\":[{\"label\":\"auto\",\"value\":\"\",\"on\":true},{\"label\":\"low\",\"value\":\"low\",\"on\":false},{\"label\":\"high\",\"value\":\"high\",\"on\":false},{\"label\":\"max\",\"value\":\"max\",\"on\":false}],\"providers\":[{\"label\":\"claude\",\"value\":\"@claude\",\"on\":true},{\"label\":\"codex\",\"value\":\"@codex\",\"on\":false},{\"label\":\"grok\",\"value\":\"@grok\",\"on\":false},{\"label\":\"muse\",\"value\":\"@muse\",\"on\":false},{\"label\":\"antigravity\",\"value\":\"@antigravity\",\"on\":false},{\"label\":\"omp\",\"value\":\"@omp\",\"on\":false}]}")
