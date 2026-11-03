import Base
import ./check.bend as C
import ../src/core/mailtriage.bend as T

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("unrelated malformed setting stays outside the parser", String.join(T.Triage.queue.key.parts("thread.owner.not-json"), ","), "")
    C.Check.str("prefix lookalike is not a queue", String.join(T.Triage.queue.key.parts("triage.queues.[\"b\",\"c\"]"), ","), "")
    C.Check.str("owned queue key preserves order", String.join(T.Triage.queue.key.parts("triage.queue.[\"bot\",\"conversation\"]"), ","), "bot,conversation")
    C.Check.str("queue key keeps Unicode and escapes", String.join(T.Triage.queue.key.parts("triage.queue.[\"工具\",\"a\\nb\"]"), ","), "工具,a\nb")
    C.Check.str("malformed queue key remains empty", String.join(T.Triage.queue.key.parts("triage.queue.[oops"), ","), "")
    C.Check.str("non-string queue elements keep existing filtering", String.join(T.Triage.queue.key.parts("triage.queue.[0,\"bot\",null,\"conversation\"]"), ","), "bot,conversation")
