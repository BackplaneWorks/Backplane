import Base
import ./check.bend as C
import ../src/server/push.bend as P
import ../src/server/push-gate.bend as G

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("TopicDisallowed permanently retires exact registration", Bool.show(P.Push.dead("400", "TopicDisallowed")), "True")
    C.Check.str("device-topic mismatch permanently retires registration", Bool.show(P.Push.dead("400", "DeviceTokenNotForTopic")), "True")
    C.Check.str("unregistered still retires", Bool.show(P.Push.dead("410", "Unregistered")), "True")
    C.Check.str("bad token still retires", Bool.show(P.Push.dead("400", "BadDeviceToken")), "True")
    C.Check.str("temporary failure retains source device", Bool.show(P.Push.dead("503", "ServiceUnavailable")), "False")
    C.Check.str("payload failure does not delete good device", Bool.show(P.Push.dead("400", "PayloadTooLarge")), "False")
    C.Check.str("auth failure does not delete good device", Bool.show(P.Push.dead("403", "ExpiredProviderToken")), "False")
    C.Check.str("successful delivery keeps device", Bool.show(P.Push.dead("200", "")), "False")
    C.Check.str("topic circuit uses actual topic", P.Push.topic(["authorization: synthetic", "apns-topic: test.app.push-type.liveactivity"]), "test.app.push-type.liveactivity")
    C.Check.str("missing topic fails to empty key", P.Push.topic(["test: header"]), "")
    C.Check.str("curl final status survives bounded retry output", P.Curl.code("500503200"), "200")
    +lease : String <- G.PushGate.claim("https://synthetic.invalid/device/opaque", "synthetic.topic", "activity", "1", 100n)
    C.Check.str("effect grants exact topic registration", Bool.show(Bool.not(String.is_empty(lease))), "True")
    G.PushGate.finish(lease, "400", "TopicDisallowed", 100n)
    held : String <- G.PushGate.claim("https://synthetic.invalid/device/opaque", "synthetic.topic", "activity", "1", 200n)
    C.Check.str("real effect suppresses permanent registration", held, "")
    +newer : String <- G.PushGate.claim("https://synthetic.invalid/device/opaque", "synthetic.topic", "activity", "2", 200n)
    C.Check.str("new registration receives independent grant", Bool.show(Bool.not(String.is_empty(newer))), "True")
    G.PushGate.finish(newer, "200", "", 200n)
