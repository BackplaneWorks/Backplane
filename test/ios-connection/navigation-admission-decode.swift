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
        let oldProject = #"{"id":"h|p","title":"P","root":"/p","machine":"","open":false,"threads":[],"snoozedShelf":"S","snoozed":[],"shelf":"Done","settled":[]}"#
        let closedProject = String(oldProject.dropLast()) + #", "value":"h|arch:p","archOpen":false,"archived":[],"settledCount":9,"archivedCount":12,"threadsPage":null,"snoozedPage":null,"settledPage":null,"archivedPage":null}"#
        let descriptor = #"{"version":1,"hub":"h","projects":null,"active":null,"legacy":null}"#
        func decodeScreen(_ project: String, pages: String? = nil) -> Screen? {
            let text = #"{"projects":[\#(project)]\#(pages.map { ",\"sidebarPages\":" + $0 } ?? "")}"#
            return try? JSONDecoder().decode(Screen.self, from: Data(text.utf8))
        }
        precondition(decodeScreen(oldProject)?.projects.count == 1)
        precondition(decodeScreen(closedProject, pages: "[" + descriptor + "]")?.projects.first?.settledCount?.value == 9)
        precondition(decodeScreen(oldProject.replacingOccurrences(of: "h|p", with: "other|p") + "," + closedProject, pages: "[" + descriptor + "]")?.projects.count == 2)
        print("PASS actual Screen preserves old arrays and negotiated empty-shelf counts")
        precondition(decodeScreen(closedProject) == nil)
        precondition(decodeScreen(oldProject, pages: "[" + descriptor + "]") == nil)
        precondition(decodeScreen(closedProject, pages: "[" + descriptor + "," + descriptor + "]") == nil)
        print("PASS actual Screen refuses missing, duplicate or unsolicited metadata owners")
        precondition(decodeScreen(closedProject.replacingOccurrences(of: "\"settledCount\":9", with: "\"settledCount\":null"), pages: "[" + descriptor + "]") == nil)
        precondition(decodeScreen(closedProject.replacingOccurrences(of: "h|p", with: ""), pages: "[" + descriptor + "]") == nil)
        print("PASS malformed new metadata cannot silently drop a project through legacy lossy decoding")
    }
}
