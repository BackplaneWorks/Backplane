import Foundation

@main struct DecodeDiagnosticTests {
    private struct Envelope: Decodable { let count: Int }
    private struct DynamicKey: CodingKey {
        let stringValue: String
        var intValue: Int? { nil }
        init(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { return nil }
    }
    static func main() throws {
        var checks = 0
        func check(_ value: Bool, _ name: String) {
            guard value else { fputs("FAIL \(name)\n", stderr); exit(1) }
            checks += 1
        }
        for (json, expected) in [("{\"count\":\"private-payload-canary\"}", "type:count"),
                                 ("{}", "missing:count"), ("{\"count\":null}", "null:count")] {
            do { _ = try JSONDecoder().decode(Envelope.self, from: Data(json.utf8)); check(false, "invalid fixture rejected") }
            catch { check(WireDecodeDiagnostic.summary(error) == expected, "schema-only decode diagnosis") }
        }
        let context = DecodingError.Context(codingPath: [DynamicKey(stringValue: "private-key-canary")],
                                           debugDescription: "private-description-canary",
                                           underlyingError: NSError(domain: "private-domain-canary", code: 1))
        let redacted = WireDecodeDiagnostic.summary(DecodingError.dataCorrupted(context))
        check(redacted == "invalid:<key>", "dynamic keys and context values excluded")
        check(WireDecodeDiagnostic.summary(NSError(domain: "private-domain-canary", code: 1)) == "unknown", "foreign errors excluded")
        print("PASS decode diagnostic \(checks) checks")
    }
}
