import Base
import ../src/core/model.bend as M
import ../src/core/hub.bend as B

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
  {B.Fq.step.sets(B.Fq.queue(B.Fq.demo.s(), 5n, "p3", "t1", "thread_send", "{}")) == [B.Fq.demo.l1() ++ "\n" ++ B.Fq.demo.l2() ++ "\n" ++ B.Fq.line(B.Hub.id("q", 5n, B.Hub.seq(B.Fq.demo.s())), "p3", "t1", "thread_send", 5n, "{}")] : List<&2, String>}
# ---- end far queue ----

def fq_tools(): {==}

def fq_fields(): {==}

def fq_at(): {==}

def fq_order(): {==}

def fq_done_drops(): {==}

def fq_done_once(): {==}

def fq_old(): {==}

def fq_queue(): {==}
