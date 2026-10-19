import XCTest

// Focused actual app test. Hosts contain duplicate target IDs and distinct
// body markers, so a successful tap on the wrong machine fails acceptance.
final class LinkTap: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    private func check(_ label: String, title: String, marker: String) {
        let app = XCUIApplication(bundleIdentifier: "dev.backplane.reading")
        app.launchEnvironment["BACKPLANE_LINK"] = "http://127.0.0.1:3923 http://127.0.0.1:3924"
        app.launchEnvironment["JSC_useJIT"] = "0"
        app.launch()
        if app.buttons["Skip"].exists { app.buttons["Skip"].tap() }
        let threads = app.tabBars.buttons["Threads"]
        XCTAssertTrue(threads.waitForExistence(timeout: 15), app.debugDescription)
        threads.tap()
        for _ in 0..<3 {
            let back = app.navigationBars.buttons["BackButton"]
            if !back.exists { break }
            back.tap()
        }
        let source = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "Link source A")).firstMatch
        XCTAssertTrue(source.waitForExistence(timeout: 10), app.debugDescription)
        source.tap()
        XCTAssertTrue(app.navigationBars.matching(NSPredicate(format: "identifier CONTAINS %@", "Link source A")).firstMatch.waitForExistence(timeout: 5), app.debugDescription)
        let loading = app.descendants(matching: .any).matching(identifier: "thread.loading").firstMatch
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: loading)], timeout: 10), .completed)
        // SwiftUI exposes an attributed Text paragraph as StaticText, not
        // XCUIElementTypeLink. This paragraph consists entirely of the link;
        // tap the rendered text itself, never call the URL handler in tests.
        let link = app.staticTexts[label]
        XCTAssertTrue(link.waitForExistence(timeout: 5), app.debugDescription)
        XCTAssertTrue(link.isHittable, app.debugDescription)
        let before = XCTAttachment(screenshot: app.screenshot()); before.name = label + "-rendered"; before.lifetime = .keepAlways; add(before)
        link.tap()
        XCTAssertTrue(app.navigationBars.matching(NSPredicate(format: "identifier CONTAINS %@", title)).firstMatch.waitForExistence(timeout: 5), app.debugDescription)
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", marker)).firstMatch.waitForExistence(timeout: 5), app.debugDescription)
        XCTAssertFalse(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "WRONG_MACHINE_B")).firstMatch.exists)
        let after = XCTAttachment(screenshot: app.screenshot()); after.name = label + "-destination"; after.lifetime = .keepAlways; add(after)
        app.terminate()
    }

    func testCanonicalRenderedTap() { check("Canonical Raul", title: "Raul on A", marker: "MATCH_A_RAUL") }
    func testLegacyRenderedTap() { check("Legacy Raul", title: "Raul on A", marker: "MATCH_A_RAUL") }
    func testRemoteRenderedTap() { check("Remote SeaGull", title: "SeaGull remote A", marker: "MATCH_A_REMOTE") }
}
