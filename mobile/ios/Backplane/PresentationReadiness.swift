// A request for the next destination cannot make the currently displayed
// picture fresh. Connection, pairing and epoch checks remain at the caller.
enum PresentationReadiness {
    static func matches(displayed: String, mark: String, awaited: String?) -> Bool {
        mark == displayed && (awaited == nil || awaited == displayed)
    }
}
