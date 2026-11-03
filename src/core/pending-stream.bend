import Base
import ./thin-live.bend as TL

# Static pending rows expose presence and generation, never streamed text.
# Gate only a live delta's notice with this predicate. Stream storage,
# revision, checkpoints, canonical/global notices and legacy/far delivery
# remain independent of it. No block or message text is inspected here.
def PendingStream.changed(old: TL.Stream, new: TL.Stream) -> Bool:
  match old new:
    case TL.Stream{oldGen, oldSize, _, _, _} TL.Stream{newGen, newSize, _, _, _}:
      Bool.or(Bool.not(Nat.is_eq(oldGen, newGen)),
        Bool.xor(Nat.is_eq(oldSize, 0n), Nat.is_eq(newSize, 0n)))
