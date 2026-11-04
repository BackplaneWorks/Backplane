import Base
import ./check.bend as C
import ../src/core/kicad-project.bend as K
import ../src/core/json.bend as J

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("linked checkout resolves to main", K.Project.root(True{}, "/thread", "/main"), "/main")
    C.Check.str("ordinary project retains cwd", K.Project.root(False{}, "/project", "/main"), "/project")
    C.Check.str("main is canonical for viewer paths", K.Project.path(False{}, "/thread", "/main", "/thread/board.kicad_pcb"), "/main/board.kicad_pcb")
    C.Check.str("relative project path uses main", K.Project.path(False{}, "/thread", "/main", "board.kicad_pro"), "/main/board.kicad_pro")
    C.Check.str("explicit copy preserves thread path", K.Project.path(True{}, "/thread", "/main", "/thread/board.kicad_pcb"), "/thread/board.kicad_pcb")
    C.Check.str("external explicit document stays explicit", K.Project.path(False{}, "/thread", "/main", "/outside/board.kicad_sch"), "/outside/board.kicad_sch")
    C.Check.str("path sibling is not remapped", K.Project.path(False{}, "/thread", "/main", "/thread-other/board.kicad_pcb"), "/thread-other/board.kicad_pcb")
    C.Check.str("other file types retain their paths", K.Project.path(False{}, "/thread", "/main", "/thread/mech/a.step"), "/thread/mech/a.step")
    C.Check.str("unchanged worktree opens without prompt", yes(K.Project.choose("", False{})), "no")
    C.Check.str("changed worktree offers choice", yes(K.Project.choose("", True{})), "yes")
    C.Check.str("explicit main opens without another prompt", yes(K.Project.choose("main", True{})), "no")
    C.Check.str("explicit copy opens without another prompt", yes(K.Project.choose("thread", True{})), "no")
    C.Check.str("unknown choice cannot open a copy", yes(K.Project.choose("other", True{})), "yes")
    C.Check.str("divergent mutation needs a target", yes(K.Project.refuse_write(True{}, True{}, False{}, False{})), "yes")
    C.Check.str("explicit mutation target is allowed", yes(K.Project.refuse_write(True{}, True{}, False{}, True{})), "no")
    C.Check.str("reads keep their main default", yes(K.Project.refuse_write(True{}, True{}, True{}, False{})), "no")
    C.Check.str("identical copies keep existing writes", yes(K.Project.refuse_write(True{}, False{}, False{}, False{})), "no")
    C.Check.str("ordinary checkout keeps existing writes", yes(K.Project.refuse_write(False{}, True{}, False{}, False{})), "no")
    C.Check.str("GetItems is a known read", yes(K.Project.readonly("kiapi.common.commands.GetItems")), "yes")
    C.Check.str("BeginCommit is a mutation", yes(K.Project.readonly("kiapi.common.commands.BeginCommit")), "no")
    C.Check.str("unknown Get prefix is not trusted", yes(K.Project.readonly("kiapi.common.commands.GetAndDeleteItems")), "no")
    C.Check.str("unknown namespace is not trusted", yes(K.Project.readonly("kiapi.future.GetItems")), "no")
    C.Check.str("SaveDocument is a mutation", yes(K.Project.readonly("kiapi.common.commands.SaveDocument")), "no")
    +cfg : J.Json = J.Json.obj([J.KV{"project", J.Str{"boards/main.kicad_pro"}}])
    C.Check.str("relative thread config cannot replace main", J.Json.show(K.Project.config(cfg, J.Json.obj([J.KV{"project", J.Str{"thread.kicad_pro"}}]))), "{\"project\":\"boards/main.kicad_pro\",\"pcb\":\"\",\"schematic\":\"\"}")
    C.Check.str("absolute override retained", J.Json.get_str(K.Project.config(cfg, J.Json.obj([J.KV{"project", J.Str{"/explicit/main.kicad_pro"}}])), "project", ""), "/explicit/main.kicad_pro")
