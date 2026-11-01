import Base
import ../src/server/push.bend as P
import ./check.bend as C

law push_disallowed_permanent:
  {P.Push.dead("400", "TopicDisallowed") == True{} : Bool}

law push_topic_mismatch_permanent:
  {P.Push.dead("400", "DeviceTokenNotForTopic") == True{} : Bool}

law push_transient_keeps_device:
  {P.Push.dead("503", "ServiceUnavailable") == False{} : Bool}

law push_payload_keeps_device:
  {P.Push.dead("400", "PayloadTooLarge") == False{} : Bool}

def push_disallowed_permanent():
  {==}

def push_topic_mismatch_permanent():
  {==}

def push_transient_keeps_device():
  {==}

def push_payload_keeps_device():
  {==}

# The laws above are checked when this file is; main prints a verdict line
# per case so scripts/test.sh counts it (it needs "ok " lines and exit 0).
def Push.word(b: Bool) -> String:
  match b:
    case True{}: "True"
    case False{}: "False"

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("TopicDisallowed is permanent", Push.word(P.Push.dead("400", "TopicDisallowed")), "True")
    C.Check.str("DeviceTokenNotForTopic is permanent", Push.word(P.Push.dead("400", "DeviceTokenNotForTopic")), "True")
    C.Check.str("ServiceUnavailable keeps the device", Push.word(P.Push.dead("503", "ServiceUnavailable")), "False")
    C.Check.str("PayloadTooLarge keeps the device", Push.word(P.Push.dead("400", "PayloadTooLarge")), "False")
