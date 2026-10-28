import Foundation

@main struct NavigationAdmissionDecodeChecks {
    static func main() {
        let accepted = Out.decode(#"{"cmds":[],"screen":null,"navigationAccepted":true}"#, screen: true)
        precondition(accepted?.navigationAccepted == true)
        print("PASS actual Out preserves admitted navigation")
        let rejected = Out.decode(#"{"cmds":[],"screen":null,"navigationAccepted":false}"#, screen: true)
        precondition(rejected?.navigationAccepted == false)
        print("PASS actual Out preserves explicit local rejection")
        let absent = Out.decode(#"{"cmds":[],"screen":null}"#, screen: true)
        precondition(absent != nil && absent?.navigationAccepted == nil)
        precondition(!(absent?.navigationAccepted == false))
        print("PASS old or missing admission is not rejection proof")
        let malformed = Out.decode(#"{"cmds":[],"screen":null,"navigationAccepted":"false"}"#, screen: true)
        precondition(malformed == nil)
        print("PASS malformed admission cannot release a claim")
        let metadata = Out.decode(#"{"cmds":[],"screen":null,"navigationAccepted":false}"#, screen: false)
        precondition(metadata?.navigationAccepted == false)
        print("PASS withheld screen retains explicit admission metadata")
    }
}
