import Base
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/fmcp.bend as F

# The far queue's laws on their own (bend test/far_queue_proof.bend)
# ---- far queue: calls for the owner's machines that are away (hub.bend's Fq.*) ----
# thread_send and thread_launch wait for a machine that is away; nothing else does
law fq_tools:
  {[B.Fq.tool("thread_send"), B.Fq.tool("thread_launch"), B.Fq.tool("thread_interrupt"), B.Fq.tool("thread_read")] == [True{}, True{}, False{}, False{}] : List<&2, Bool>}

# a line keeps each field
law fq_fields:
  {[B.Fq.f.key(B.Fq.demo.l1()), B.Fq.f.link(B.Fq.demo.l1()), B.Fq.f.thread(B.Fq.demo.l1()), B.Fq.f.tool(B.Fq.demo.l1()), B.Fq.f.body(B.Fq.demo.l1())]
    == ["q1", "p1", "t1", "thread_send", "{}"] : List<&2, String>}

law fq_at:
  {B.Fq.f.at(B.Fq.demo.l2()) == 200n : Nat}

# queued calls are sent in the order they were queued
law fq_order:
  {B.Fq.lines(B.Fq.demo.s()) == [B.Fq.demo.l1(), B.Fq.demo.l2()] : List<&2, String>}

# a call answered leaves the queue, the others stay in order
law fq_done_drops:
  {B.Fq.step.sets(B.Fq.done(B.Fq.demo.s(), 300n, "q1", "t1", "ok")) == [B.Fq.demo.l2()] : List<&2, String>}

# and its thread is told once: a key no longer queued does nothing
law fq_done_once:
  {B.Fq.step.n(B.Fq.done(B.Fq.demo.s(), 300n, "q9", "t1", "ok")) == 0n : Nat}

# a call older than a day is not sent
law fq_old:
  {[B.Fq.old(B.Fq.demo.l1(), 86500n), B.Fq.old(B.Fq.demo.l1(), 86501n)] == [False{}, True{}] : List<&2, Bool>}

# a call queued keeps what was there and asks for the queue to be sent
law fq_queue:
  {B.Fq.step.sets(B.Fq.queue(B.Fq.demo.s(), 5n, "q3", "p3", "t1", "thread_send", "{}")) == [B.Fq.demo.l1() ++ "\n" ++ B.Fq.demo.l2() ++ "\n" ++ B.Fq.line("q3", "p3", "t1", "thread_send", 5n, "{}")] : List<&2, String>}

# the queue is bounded in calls and in bytes per call
law fq_room:
  {[B.Fq.room.n(99n, 10n), B.Fq.room.n(100n, 10n), B.Fq.room.n(0n, 65536n), B.Fq.room.n(0n, 65537n)] == [True{}, False{}, True{}, False{}] : List<&2, Bool>}

# its machine keeps more receipts per link than the queue holds calls, so a
# call sent again always finds its answer
law fq_window:
  {Nat.is_lt(B.Fq.max(), F.Fm.seen.max()) == True{} : Bool}

# a keyed call that comes again is answered the same and runs nothing: one
# message reached t0, the second time made no change
law fq_replay_once:
  {F.Fm.replay.demo() == ["1", "0", "same"] : List<&2, String>}

# the bound counts UTF-8 bytes, and of the request as keyed
law fq_bytes_utf8:
  {[B.Fq.utf8("a\u{e9}\u{20ac}\u{1f600}", 0n), B.Fq.utf8("", 0n)] == [10n, 0n] : List<&2, Nat>}

law fq_room_keyed:
  {Nat.is_gt(F.Fm.later.size("{\"id\":0,\"m\":\"mcp.call\",\"p\":{\"tool\":\"thread_send\"}}", "q1"), B.Fq.utf8("{\"id\":0,\"m\":\"mcp.call\",\"p\":{\"tool\":\"thread_send\"}}", 0n)) == True{} : Bool}
# ---- end far queue ----

def fq_tools(): {==}

def fq_fields(): {==}

def fq_at(): {==}

def fq_order(): {==}

def fq_done_drops(): {==}

def fq_done_once(): {==}

def fq_old(): {==}

def fq_queue(): {==}

def fq_room(): {==}

def fq_window(): {==}

def fq_replay_once(): {==}

def fq_bytes_utf8(): {==}

def fq_room_keyed(): {==}
