import Base
import ./model.bend as M
import ./project-home.bend as PH
import ./folder.bend as F

# A missing folder is a refusal, never a process working-directory default.
def ThreadPath.folder(+root: String) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>, F.Folder.absolute(root), Some{root}, None{})

def ThreadPath.project(p: M.Project) -> Maybe<&2, String>:
  match p:
    case M.Project{_, _, root, _, False{}}: ThreadPath.folder(root)
    case _: None{}

def ThreadPath.project.got(r: Map<&2, M.Project> & M.Project) -> Maybe<&2, String>:
  (_, p) = r
  ThreadPath.project(p)

def ThreadPath.project.of(s: M.State, +id: String) -> Maybe<&2, String>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      ThreadPath.project.got(Map.get(M.Project, M.Project.blank(id), ps, id))

def ThreadPath.worktree(p: Maybe<&2, String>, +wt: String) -> Maybe<&2, String>:
  match p:
    case None{}: None{}
    case Some{root}:
      Bool.pick(Maybe<&2, String>, String.is_empty(wt), Some{root}, ThreadPath.folder(wt))

# Private coordination threads have no project and carry an explicit folder.
# Archiving preserves a thread's folder; deletion and project removal revoke it.
def ThreadPath.cwd(+s: M.State, t: M.Thread) -> Maybe<&2, String>:
  match t:
    case M.Thread{_, +p, _, _, _, _, _, _, _, _, _, _, _, _, +wt, _, _, _, _, _, False{}}:
      Bool.pick(Maybe<&2, String>, String.is_empty(p), ThreadPath.folder(wt), ThreadPath.worktree(ThreadPath.project.of(s, p), wt))
    case _: None{}

def ThreadPath.thread.k(empty: Bool, +s: M.State, +id: String) -> Maybe<&2, String>:
  match empty:
    case True{}: None{}
    case False{}: ThreadPath.cwd(s, M.State.thread.get(s, id))

def ThreadPath.thread(+s: M.State, +id: String) -> Maybe<&2, String>:
  ThreadPath.thread.k(String.is_empty(id), s, id)

# Project-home owners and local threads share exactly one folder boundary.
def ThreadPath.of(+s: M.State, +owner: String) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>, PH.Home.is(owner), ThreadPath.project.of(s, PH.Home.project(owner)), ThreadPath.thread(s, owner))
