import Base
import ./telemetry.bend as TI
import ../core/telemetry.bend as T
import ../core/model.bend as M

# These are normal human UI actions, not queries, timers or agent methods.
# Authentication and successful reply admission are supplied by the owning hub.
# This is an interaction signal, not hardware attestation of a human person.
def Human.allowed(+action: String) -> Bool:
  Bool.or(String.eq(action, "send"), Bool.or(String.eq(action, "send-alt"),
    Bool.or(String.eq(action, "fold"), Bool.or(String.eq(action, "subs-open"),
      Bool.or(String.eq(action, "subs-shut"), String.eq(action, "bot-tab"))))))

def Human.mark_if(go: Bool, cfg: List<&2, TI.TmCfg>, now: Nat) -> List<&2, TI.TmCfg>:
  match go:
    case True{}: TI.Tio.human(cfg, now)
    case False{}: cfg

def Human.mark(authenticated: Bool, successful: Bool, +st: M.State, method: String, action: String, cfg: List<&2, TI.TmCfg>, now: Nat) -> List<&2, TI.TmCfg>:
  Human.mark_if(Bool.and(authenticated, Bool.and(successful, Bool.and(String.eq(method, "view.action"),
    Bool.and(Human.allowed(action), T.Tm.allowed(st))))), cfg, now)

# Inspect every committed setting, rather than just the endpoints of a batch:
# off then on in one append must still discard earlier action evidence.
def Privacy.key(+key: String) -> Bool:
  Bool.or(String.eq(key, "telemetry.share"), Bool.or(String.eq(key, "telemetry.asked"), Bool.or(String.eq(key, "telemetry.usage.asked"), String.eq(key, "telemetry.reset"))))

def Privacy.clear_if(clear: Bool, cfg: List<&2, TI.TmCfg>) -> List<&2, TI.TmCfg>:
  match clear:
    case True{}: TI.Tio.human.clear(cfg)
    case False{}: cfg

def Privacy.change(c: M.Change, cfg: List<&2, TI.TmCfg>) -> List<&2, TI.TmCfg>:
  match c:
    case M.SettingSet{key, _, _}: Privacy.clear_if(Privacy.key(key), cfg)
    case _: cfg

def Privacy.committed(cs: List<&2, M.Change>, cfg: List<&2, TI.TmCfg>) -> List<&2, TI.TmCfg>:
  match cs:
    case Nil{}: cfg
    case c <> tail: Privacy.committed(tail, Privacy.change(c, cfg))
