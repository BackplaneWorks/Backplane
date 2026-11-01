import Base
import ./sidebar_input_proof.bend as SIDEINPUTPROOF
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M

# The complete plain-sidebar metadata boundary includes bot/mail ownership.
law side_ix_metadata_input:
  for +st: M.State
  {U.Side.ix.inputs(U.SidebarInput.of(st)) == U.Side.ix(st) : U.SIx}

law side_input_activity:
  for +st: M.State
  for th: String
  for act: String
  for tone: String
  for kind: String
  for summary: String
  for at: Nat
  {U.SidebarInput.of(M.State.apply(st, M.ActivityLogged{th, act, tone, kind, summary, at})) == U.SidebarInput.of(st) : U.SidebarInput}

law side_input_assistant:
  for +st: M.State
  for th: String
  for id: String
  for text: String
  for at: Nat
  {U.SidebarInput.of(M.State.apply(st, M.MessagePosted{th, id, M.Assistant{}, text, at})) == U.SidebarInput.of(st) : U.SidebarInput}

law side_input_system:
  for +st: M.State
  for th: String
  for id: String
  for text: String
  for at: Nat
  {U.SidebarInput.of(M.State.apply(st, M.MessagePosted{th, id, M.System{}, text, at})) == U.SidebarInput.of(st) : U.SidebarInput}

def side_ix_metadata_input(st):
  SIDEINPUTPROOF.exact(st)

def side_input_activity(st, th, act, tone, kind, summary, at):
  match st:
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, Nil{}}: {==}
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, M.Ext{world, by, mail, usage} <> rest}: {==}

def side_input_assistant(st, th, id, text, at):
  match st:
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, Nil{}}: {==}
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, M.Ext{world, by, mail, usage} <> rest}: {==}

def side_input_system(st, th, id, text, at):
  match st:
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, Nil{}}: {==}
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, M.Ext{world, by, mail, usage} <> rest}: {==}

def main() -> IO(Unit):
  C.Check.str("four exact sidebar dependency laws", "proved", "proved")
