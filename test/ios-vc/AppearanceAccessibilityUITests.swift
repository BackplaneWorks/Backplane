import XCTest

// Use only on a dedicated, already paired simulator. No typing, send, pairing,
// screenshots, or accessibility-tree logging; the runner controls presets.
final class AppearanceAccessibilityUITests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    private func launchPaired() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        XCTAssertTrue(app.buttons["tab.threads"].waitForExistence(timeout: 60), "Requires a paired compact-width simulator")
        XCTAssertFalse(app.textFields["pair.address"].exists, "Never capture pairing UI in this suite")
        return app
    }

    private func reveal(_ button: XCUIElement, in app: XCUIApplication) {
        // Start on a visible tab, so the drag belongs to the horizontal strip.
        let names = ["home", "threads", "bots", "routines", "settings"]
        func visibleTab() -> XCUIElement? {
            names.map { app.buttons["tab." + $0] }.first { $0.isHittable }
        }
        for _ in 0..<4 {
            if button.isHittable { return }
            guard let visible = visibleTab() else { return }
            visible.swipeLeft()
        }
        for _ in 0..<4 {
            if button.isHittable { return }
            guard let visible = visibleTab() else { return }
            visible.swipeRight()
        }
    }

    func testCompactTabLabelsAndSelection() {
        let app = launchPaired()
        for name in ["home", "threads", "bots", "routines", "settings"] {
            let button = app.buttons["tab." + name]
            XCTAssertTrue(button.exists, "Missing tab \(name)")
            reveal(button, in: app)
            XCTAssertTrue(button.isHittable, "Tab \(name) must be reachable at this text size")
            XCTAssertFalse(button.label.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, "Tab \(name) needs a spoken label")
            button.tap()
            let selected = XCTNSPredicateExpectation(predicate: NSPredicate(format: "selected == YES"), object: button)
            XCTAssertEqual(XCTWaiter.wait(for: [selected], timeout: 5), .completed, "Tab \(name) must expose its selected state")
        }
    }

    func testProjectSearchKeyboardClearanceWithoutEditing() {
        let app = launchPaired()
        let threads = app.buttons["tab.threads"]
        reveal(threads, in: app)
        XCTAssertTrue(threads.isHittable)
        threads.tap()
        let search = app.textFields["projects.search"]
        // Back only while a thread composer proves this is a thread stack.
        for _ in 0..<3 {
            if search.waitForExistence(timeout: 2) { break }
            guard app.descendants(matching: .any)["thread.composer"].exists else { break }
            let back = app.navigationBars.buttons.firstMatch
            guard back.exists && back.isHittable else { break }
            back.tap()
        }
        XCTAssertTrue(search.waitForExistence(timeout: 30))
        XCTAssertTrue(search.isHittable)
        search.tap()
        let keyboard = app.keyboards.firstMatch
        XCTAssertTrue(keyboard.waitForExistence(timeout: 10))
        XCTAssertTrue(search.isHittable, "Search must remain reachable with the keyboard up")
        XCTAssertLessThanOrEqual(search.frame.maxY, keyboard.frame.minY + 1, "Search must clear the keyboard")
        XCTAssertGreaterThanOrEqual(search.frame.minY, app.frame.minY, "Search must remain within the app viewport")
    }

    @available(iOS 17.0, *)
    func testCurrentScreenAccessibilityAudit() throws {
        let app = launchPaired()
        // Aggregate every issue into a failing test without printing XCTest's
        // issue descriptions, which may contain live labels or values.
        var issues = 0
        do {
            try app.performAccessibilityAudit(for: .all) { issue in
                issues += 1
                let element = issue.element
                let identifier = element?.identifier ?? ""
                let category: String
                if identifier.hasPrefix("tab.") { category = "tab" }
                else if identifier == "projects.search" { category = "projects.search" }
                else if identifier == "thread.composer" { category = "thread.composer" }
                else if identifier.hasPrefix("thread.entry.") { category = "thread.entry" }
                else { category = "other" }
                let type = element.map { Int($0.elementType.rawValue) } ?? -1
                let frame = element?.frame ?? .null
                let geometry: String
                if !frame.isNull && !frame.isInfinite && frame.origin.x.isFinite && frame.origin.y.isFinite && frame.width.isFinite && frame.height.isFinite {
                    geometry = "\(frame.origin.x),\(frame.origin.y),\(frame.width),\(frame.height)"
                } else { geometry = "unavailable" }
                print("accessibility-audit type=\(issue.auditType.rawValue) elementType=\(type) category=\(category) frame=\(geometry)")
                // Avoid framework-rendered issue descriptions; the aggregate
                // assertion below still fails for every reported issue.
                return true
            }
        } catch {
            XCTFail("Accessibility audit could not complete")
            return
        }
        XCTAssertEqual(issues, 0, "Accessibility audit reported issues; see sanitized records")
    }
}
