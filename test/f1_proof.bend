import Base
import ../src/core/model.bend as M
import ../src/core/far.bend as FR
import ../src/core/hot.bend as HOTF
import ../src/core/farsnap.bend as FSNAP

# F1's laws on their own, to time them apart from PROOF.bend (bend f1_proof.bend)
# ---- F1: far mirrors keep only what is live (hot.bend, farsnap.bend) ----
# A snapshot folded shows the same live view as the state it was made from
law far_snap_live:
  {HOTF.Hot.view(FSNAP.Snap.fold(FSNAP.Snap.of.at(FSNAP.Snap.demo(), 2n)), 2n) == HOTF.Hot.view(FSNAP.Snap.demo(), 2n) : List<&2, HOTF.HRow>}

# and a snapshot of that fold is the snapshot again
law far_snap_fix:
  {FSNAP.Snap.of.at(FSNAP.Snap.fold(FSNAP.Snap.of.at(FSNAP.Snap.demo(), 2n)), 2n) == FSNAP.Snap.of.at(FSNAP.Snap.demo(), 2n) : List<&2, M.Change>}

# a mirror compacted from a machine's whole log shows what that log does live
law far_compact_live:
  {HOTF.Hot.view(FR.Fr.items.fold(FR.In.items(FR.In.compact(FR.Fr.demo.in())), M.State.empty()), FSNAP.Snap.k()) == HOTF.Hot.view(FSNAP.Snap.demo(), FSNAP.Snap.k()) : List<&2, HOTF.HRow>}

# compacting keeps the head and the epoch, and leaves no tail
law far_compact_head:
  {FR.In.head(FR.In.compact(FR.Fr.demo.in())) == FR.In.head(FR.Fr.demo.in()) : Nat}

law far_compact_tail:
  {FR.In.tail(FR.In.compact(FR.Fr.demo.in())) == 0n : Nat}

# a shared change gives its store row, a private one none
law far_fact_shared:
  {FR.Fr.fact(FSNAP.Snap.demo(), M.ThreadRenamed{"t1", "x", 50n}, 7n) == [["#7", "p1"]] : List<&2, List<&2, String>>}

law far_fact_private:
  {FR.Fr.fact(FSNAP.Snap.demo(), M.SettingSet{"k", "v", 50n}, 7n) == Nil{} : List<&2, List<&2, String>>}

# live: settled, archived and deleted threads are not; a task, a bot and a
# running turn keep one
law hot_demo_threads:
  {[HOTF.Hot.thread.of(FSNAP.Snap.demo(), "t6"), HOTF.Hot.thread.of(FSNAP.Snap.demo(), "t7"), HOTF.Hot.thread.of(FSNAP.Snap.demo(), "t8"),
    HOTF.Hot.thread.of(FSNAP.Snap.demo(), "t10"), HOTF.Hot.thread.of(FSNAP.Snap.demo(), "t2"), HOTF.Hot.thread.of(FSNAP.Snap.demo(), "t4")]
    == [False{}, False{}, False{}, True{}, True{}, True{}] : List<&2, Bool>}

def far_snap_live(): {==}

def far_snap_fix(): {==}

def far_compact_live(): {==}

def far_compact_head(): {==}

def far_compact_tail(): {==}

def far_fact_shared(): {==}

def far_fact_private(): {==}

def hot_demo_threads(): {==}
