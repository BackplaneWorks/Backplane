import Foundation

func route(_ s: String) -> InternalURLRoute { InternalURLRoute.route(URL(string: s)!) }
let canonical = "backplane://thread/t1792085069-132986"
precondition(route(canonical) == .reference(canonical, origin: nil))
let legacy = "backplane://open?thread=t1792085069-132986"
precondition(route(legacy) == .reference(legacy, origin: nil))
let encoded = "backplane://thread/link~space%2Fid#m%2F1"
precondition(route(encoded) == .reference(encoded, origin: nil), "split/decode remains owned by Bend")
let web = "https://desk:3787/#/thread/t2"
precondition(route(web) == .reference(web, origin: "desk:3787"))
precondition(route("https://desk/#/thread/t2") == .reference("https://desk/#/thread/t2", origin: "desk"))
precondition(route("https://desk/path/#/thread/t2") == .external)
precondition(route("https://example.com/page") == .external)
precondition(route("foreign://pair?url=http%3A%2F%2Fdesk") == .external)
precondition(route("backplane-file://f?p=a") == .external)
let unknown = "backplane://unknown?url=http%3A%2F%2Fdesk"
precondition(route(unknown) == .reference(unknown, origin: nil), "never falls through to pairing")
let pair = "backplane://pair?url=http%3A%2F%2Fdesk%3A3787%2F%23token%3Dabc"
precondition(route(pair) == .pair(pair))
precondition(route("backplane://pair") == .ignored)
precondition(route("backplane://google?code=secret") == .ignored)
precondition(route("backplane://open?pick=1") == .ignored)
func request(_ origin: String?, _ hub: String?) throws -> [String: String] {
    let text = InternalURLRoute.request(encoded, origin: origin, sourceHub: hub)
    return try JSONSerialization.jsonObject(with: Data(text.utf8)) as! [String: String]
}
let source = try request(nil, "desk:3787")
precondition(source == ["uri": encoded, "hub": "desk:3787"])
let other = try request("other:3787", "desk:3787")
precondition(other["hub"] == "other:3787")
let unknownOrigin = try request("unknown:3787", nil)
precondition(unknownOrigin["hub"] == "unknown:3787")
let external = try request(nil, nil)
precondition(external["hub"] == "")
print("PASS native canonical/legacy message and URL-entry routing, exact remote/encoded IDs, origin isolation, explicit pairing and unknown scheme rejection")
