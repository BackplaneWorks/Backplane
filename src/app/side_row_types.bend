import Base
import ./nui.bend as N
import ../core/client.bend as U
import ../core/model.bend as M
import ../gfx/draw.bend as D
import ../gfx/raster.bend as R

# Row cache payloads do not depend on layout: the topology can collect
# them while the renderer continues to own resolution and drawing.
type SideRowK is Data:
  SideRowK{active: Bool, on: Bool, id: String, title: String, note: String, badge: String, faded: Bool, ac: U32, dot: U32, tint: U32, clip: D.Clip, y: U32, locale: String, epoch: Nat}

type SideRowM is Data:
  SideRowM{key: SideRowK, ops: List<&2, R.Op>, hits: List<&2, N.Hit>, due: Nat}

type SideRowReq is Data:
  SideProjectRow{thread: M.Thread, snoozed: Bool, clip: D.Clip, y: U32}
  SideActiveRow{row: U.ARow, clip: D.Clip, y: U32}

type SideRowStep is Data:
  SideRowStep{ops: List<&2, R.Op>, hits: List<&2, N.Hit>, memo: List<&2, SideRowM>, hit: Nat}
