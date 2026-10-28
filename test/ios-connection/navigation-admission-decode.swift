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
        if CommandLine.arguments.count > 1 {
            // Actual Bend Actor projection fixtures, supplied as a pinned
            // artifact. No server, simulator, UI or signing is exercised.
            let text = try! String(contentsOfFile: CommandLine.arguments[1], encoding: .utf8)
            var pictures = 0, redirects = 0
            for line in text.split(separator: "\n") where line.first == "{" {
                let fixture = try! JSONSerialization.jsonObject(with: Data(line.utf8)) as! [String: Any]
                let result = fixture["result"] as! [String: Any]
                guard let shown = result["shown"] as? [String: Any] else {
                    precondition(result["nextTarget"] is [String: Any])
                    redirects += 1
                    continue
                }
                let overlay = result["overlay"] as! [String: Any]
                for picture in [shown, overlay["j"] as! [String: Any]] {
                    let decoded = try! JSONDecoder().decode(Screen.self, from: JSONSerialization.data(withJSONObject: picture))
                    let projects = picture["projects"] as! [[String: Any]]
                    precondition(decoded.projects.count == projects.count, "Actual projection project was lost")
                    precondition((decoded.sidebarPages?.count ?? 0) == (picture["sidebarPages"] as? [Any] ?? []).count)
                    for (project, wire) in zip(decoded.projects, projects) {
                        precondition(project.id == wire["id"] as? String)
                        precondition(project.threads.count == (wire["threads"] as! [Any]).count)
                        precondition(project.snoozed.count == (wire["snoozed"] as! [Any]).count)
                        precondition(project.settled.count == (wire["settled"] as! [Any]).count)
                        precondition((project.archived?.count ?? 0) == (wire["archived"] as? [Any] ?? []).count)
                    }
                    pictures += 1
                }
            }
            precondition(pictures > 0)
            print("PASS actual Actor projections decode without losing projects or rows: \(pictures) pictures; \(redirects) redirects excluded")
        }
    }
}
