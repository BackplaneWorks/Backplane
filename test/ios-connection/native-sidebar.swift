import Foundation

@main struct NativeSidebarChecks {
    static func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        try? JSONDecoder().decode(type, from: Data(json.utf8))
    }
    static func main() {
        let old = #"{"id":"h|p","title":"P","root":"/p","machine":"","open":false,"threads":[],"snoozedShelf":"S","snoozed":[],"shelf":"Done","settled":[]}"#
        let p = decode(Project.self, old)!
        precondition(p.settledCount == nil && p.archivedCount == nil && p.settled.isEmpty)
        precondition(decode(NativeSidebarProjectValidation.self, old) != nil)
        let closed = #"{"id":"h|p","title":"P","root":"/p","machine":"","open":false,"threads":[],"snoozedShelf":"S","snoozed":[],"shelf":"Done","settled":[],"value":"h|arch","archOpen":false,"archived":[],"settledCount":9,"archivedCount":12,"threadsPage":null,"snoozedPage":null,"settledPage":null,"archivedPage":null}"#
        let q = decode(Project.self, closed)!
        precondition(q.settledCount?.value == 9 && q.archivedCount?.value == 12 && q.settled.isEmpty && q.archived?.isEmpty == true)
        precondition(decode(NativeSidebarProjectValidation.self, closed) != nil)
        let page = #"{"version":1,"total":11,"offset":4,"limit":4,"selectedOutside":true,"previous":{"label":"Previous","action":"native-sidebar-page","value":"h|opaque-a"},"next":{"label":"Next","action":"native-sidebar-page","value":"h|opaque-b"}}"#
        let pager = decode(NativeSidebarPager.self, page)!
        precondition(pager.belongs(to: "h") && pager.previous?.label == "Previous" && pager.next?.label == "Next" && !pager.belongs(to: "other"))
        precondition(decode(NativeSidebarHubPage.self, #"{"version":1,"hub":"h","projects":null,"active":null,"legacy":null}"#) != nil)
        let projectPage = page.replacingOccurrences(of: "\"limit\":4", with: "\"limit\":9")
            .replacingOccurrences(of: "\"next\":{\"label\":\"Next\",\"action\":\"native-sidebar-page\",\"value\":\"h|opaque-b\"}", with: "\"next\":null")
        precondition(decode(NativeSidebarHubPage.self, #"{"version":1,"hub":"h","projects":\#(projectPage),"active":null,"legacy":null}"#) == nil)
        for invalid in [page.replacingOccurrences(of: "\"limit\":4", with: "\"limit\":0"),
                        page.replacingOccurrences(of: "\"total\":11", with: "\"total\":-1"),
                        page.replacingOccurrences(of: "\"total\":11", with: "\"total\":9007199254740992"),
                        page.replacingOccurrences(of: "native-sidebar-page", with: "select"),
                        page.replacingOccurrences(of: "\"version\":1", with: "\"version\":2")] {
            precondition(decode(NativeSidebarPager.self, invalid) == nil)
        }
        precondition(decode(NativeSidebarProjectValidation.self, #"{"id":"h|p","settledCount":null,"archivedCount":0,"threadsPage":null,"snoozedPage":null,"settledPage":null,"archivedPage":null}"#) == nil)
        precondition(decode(NativeSidebarProjectValidation.self, #"{"id":"other|p","settledCount":0,"archivedCount":0,"threadsPage":\#(page),"snoozedPage":null,"settledPage":null,"archivedPage":null}"#) == nil)
        precondition(decode(NativeSidebarHubPage.self, #"{"version":1,"hub":"h","projects":null}"#) == nil)
        precondition(decode(NativeSidebarPager.self, page.replacingOccurrences(of: "\"limit\":4", with: "\"limit\":49")) == nil)
        precondition(decode(NativeSidebarPager.self, page.replacingOccurrences(of: "\"offset\":4", with: "\"offset\":11")) == nil)
        print("PASS old arrays, closed totals, pager directions and malformed metadata")
    }
}
