import Base
import ./signal.bend as S
import ./model.bend as M
import ./i18n.bend as I
import ./msgs.bend as MSG

# Presentation only: a pending reply is a static message, never partial text.
# Work waiting on the person has its own question/approval UI.
def Pending.visible(+signal: Nat) -> Bool:
  Bool.and(Bool.not(S.Signal.blocked(signal)), Bool.or(S.Signal.active(signal), S.Signal.waits.is(signal)))

def Pending.label(lc: I.Loc) -> String:
  MSG.T.phone.working(lc)

def Pending.synthetic(+id: String) -> Bool:
  String.starts_with(id, "@live:")

def Pending.has(es: List<&2, M.Entry>) -> Bool:
  match es:
    case e <> _: Pending.synthetic(M.Idx.id(e))
    case Nil{}: False{}

def Pending.text(+lc: I.Loc, +id: String, text: String) -> String:
  Bool.pick(String, Pending.synthetic(id), Pending.label(lc), text)

# The legacy window uses the same policy via its typed phase.
def Pending.phase(ph: M.Phase) -> Bool:
  match ph:
    case M.PhWorking{}: Pending.visible(1n)
    case M.PhMonitoring{_}: Pending.visible(6n)
    case _: False{}
