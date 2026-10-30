import Base
import ../src/server/push.bend as P

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
