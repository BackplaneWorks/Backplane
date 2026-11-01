import Base
import ../src/core/sidebar-read.bend as SIDEREAD
import ../src/core/model.bend as M
import ./check.bend as C

# Exact full sidebar metadata under history-only commits.
law sidebar_read_activity:
  for +st: M.State
  for th: String
  for act: String
  for tone: String
  for kind: String
  for summary: String
  for at: Nat
  {SIDEREAD.Input.of(M.State.apply(st, M.ActivityLogged{th, act, tone, kind, summary, at})) == SIDEREAD.Input.of(st) : SIDEREAD.Input}

law sidebar_read_assistant:
  for +st: M.State
  for th: String
  for id: String
  for text: String
  for at: Nat
  {SIDEREAD.Input.of(M.State.apply(st, M.MessagePosted{th, id, M.Assistant{}, text, at})) == SIDEREAD.Input.of(st) : SIDEREAD.Input}

law sidebar_read_system:
  for +st: M.State
  for th: String
  for id: String
  for text: String
  for at: Nat
  {SIDEREAD.Input.of(M.State.apply(st, M.MessagePosted{th, id, M.System{}, text, at})) == SIDEREAD.Input.of(st) : SIDEREAD.Input}

def sidebar_read_activity(st, th, act, tone, kind, summary, at):
  match st:
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, Nil{}}: {==}
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, M.Ext{world, by, mail, usage} <> rest}: {==}

def sidebar_read_assistant(st, th, id, text, at):
  match st:
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, Nil{}}: {==}
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, M.Ext{world, by, mail, usage} <> rest}: {==}

def sidebar_read_system(st, th, id, text, at):
  match st:
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, Nil{}}: {==}
    case M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, M.Ext{world, by, mail, usage} <> rest}: {==}

def main() -> IO(Unit):
  C.Check.str("three exact full-sidebar history input laws", "proved", "proved")
