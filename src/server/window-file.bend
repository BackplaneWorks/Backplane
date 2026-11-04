import Base
import ../core/model.bend as M

# One private view index. This setter preserves every other model field.
def State.window.file(st: M.State, +key: String, es: List<&2, M.Entry>) -> M.State:
  match st:
    case M.State{ps, ts, entries, n, tk, settings, queued, asks, devices, nav, +bw}:
      M.State{ps, ts, entries, n, tk, settings, queued, asks, devices, nav,
        [M.Ext{M.Ext.world(M.Ext.of(bw)), Map.set(&2, List<&2, M.Entry>, M.Ext.by(M.Ext.of(bw)), key, es), M.Ext.mail(M.Ext.of(bw)), M.Ext.usage(M.Ext.of(bw))}]}
