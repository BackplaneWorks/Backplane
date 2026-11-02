import Base
import ./model.bend as M
import ./bot.bend as BT
import ./mail.bend as ML
import ./client.bend as U

# Complete canonical input to the server-rendered sidebar. Keep every
# metadata map, including projects, queues, devices, navigation, bot/world
# ownership and mail. Timeline entries and the global log sequence belong
# to the main view, which always receives the original current Ui.
# This transient projection is never retained in the private region memo.
type Input is Data:
  Input{projects: Map<&2, M.Project>, threads: Map<&2, M.Thread>, tasks: Map<&2, M.Task>, settings: Map<&2, String>, queued: Map<&2, List<&2, M.QItem>>, asks: Map<&2, M.Ask>, devices: Map<&2, M.Device>, nav: Map<&2, M.Nav>, world: BT.World, mail: ML.Box}

def Input.of(+st: M.State) -> Input:
  match st:
    case M.State{ps, ts, _, _, tk, sg, qu, ak, dv, nv, _}:
      Input{ps, ts, tk, sg, qu, ak, dv, nv, M.State.bots(st), M.State.mail(st)}

def Input.state(input: Input) -> M.State:
  match input:
    case Input{ps, ts, tk, sg, qu, ak, dv, nv, world, mail}:
      M.State{ps, ts, Nil{}, 0n, tk, sg, qu, ak, dv, nv, [M.Ext{world, Map.new(&2, List<&2, M.Entry>), [mail], Map.new(&2, String)}]}

def Input.ui(u: U.Ui) -> U.Ui:
  match u:
    case U.Ui{st, sel, dr, live, op, on, now, ad, pa, version, rid, want, error, note, nf, outbox, stores}:
      U.Ui{Input.state(Input.of(st)), sel, dr, live, op, on, now, ad, pa, version, rid, want, error, note, nf, outbox, stores}
