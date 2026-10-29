import XCTest
import UIKit
final class ParkedScrollUITests: XCTestCase {
    func testParkedCachedTimelineScroll() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launchEnvironment["BACKPLANE_SELECT"] = "t1790229700-1088"
        app.launch()
        let ready = app.descendants(matching: .any)["qualification.connection.ready"]
        let timeline = app.descendants(matching: .any)["thread.timeline"]
        let status = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "qualification.connection.")).firstMatch
        XCTAssertTrue(ready.waitForExistence(timeout: 60))
        guard let raw = status.value as? String, let data = raw.data(using: .utf8), let fields = (try? JSONSerialization.jsonObject(with: data)) as? [String:String], let thread = fields["thread"] else { XCTFail("Missing diagnostic status"); return }
        XCTAssertTrue(thread.hasSuffix("|t1790229700-1088") && fields["nav"] == thread && fields["contentReady"] == "true")
        XCTAssertTrue(timeline.waitForExistence(timeout: 20), "Real thread timeline must be present")
        let entries = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH %@", "thread.entry.")).allElementsBoundByIndex
        guard let entry = entries.first(where: { $0.exists && $0.isHittable && $0.frame.intersects(timeline.frame) }) else { XCTFail("No visible identified entry to observe"); return }
        let id = entry.identifier
        XCTAssertFalse(id.isEmpty)
        XCTContext.runActivity(named: "QUALIFICATION_PARK_NOW") { _ in }
        let parked = app.descendants(matching: .any)["qualification.connection.parked"]
        XCTAssertTrue(parked.waitForExistence(timeout: 30), "Client must actually park before gesture")
        let offlineQuery = app.descendants(matching: .any).matching(identifier: "thread.loading.offline")
        let offlineExists = offlineQuery.firstMatch.waitForExistence(timeout: 10)
        let offlineElements = offlineExists ? offlineQuery.allElementsBoundByIndex : []
        for element in offlineElements {
            let f = element.frame
            print("safe-offline-element type=\(element.elementType.rawValue) frame=\(f) intersects=\(f.isFiniteRect && !f.intersection(app.frame).isEmpty) hittable=\(element.isHittable) text-equal=\(element.label == "Offline · showing previous conversation")")
        }
        let label = offlineElements.first { $0.label == "Offline · showing previous conversation" && $0.frame.isFiniteRect && !$0.frame.intersection(app.frame).isEmpty }
        let offlineVisible = label != nil
        if let label = label {
            let f = label.frame
            let forbidden = ["pair.address", "hubs.address", "thread.composer"].flatMap { app.descendants(matching: .any).matching(identifier: $0).allElementsBoundByIndex }
            if !forbidden.contains(where: { $0.exists && !$0.frame.intersection(f).isEmpty }) {
                let image = app.screenshot().image
                let rect = CGRect(x:f.minX * image.size.width * image.scale / app.frame.width, y:f.minY * image.size.height * image.scale / app.frame.height, width:f.width * image.size.width * image.scale / app.frame.width, height:f.height * image.size.height * image.scale / app.frame.height).integral
                if let cg = image.cgImage?.cropping(to:rect) { let attachment = XCTAttachment(image:UIImage(cgImage:cg)); attachment.name="offline-label-only"; attachment.lifetime = .keepAlways; add(attachment) }
            }
        }
        print("safe-offline-count \(offlineElements.count)")
        let spinnerVisible = app.descendants(matching: .any)["thread.loading"].exists
        print("safe-offline-label visible=\(offlineVisible) spinner-exists=\(spinnerVisible)")
        let prior = entry.frame
        let viewport = timeline.frame
        let visible = prior.isFiniteRect && viewport.isFiniteRect && !prior.intersection(viewport).isEmpty
        let current = (status.value as? String).flatMap { $0.data(using: .utf8) }.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String:String] }
        let sameOwner = current?["thread"] == thread && current?["nav"] == thread
        print("safe-offline-status socket-offline=\(current?["socket"] == "offline") owner=\(sameOwner)")
        let send = app.descendants(matching: .any)["thread.send"]
        let toolElements = app.descendants(matching: .any).matching(identifier: "thread.tools").allElementsBoundByIndex
        let toolButtons = app.buttons.matching(identifier: "thread.tools").allElementsBoundByIndex
        for element in toolElements { print("safe-tools type=\(element.elementType.rawValue) enabled=\(element.isEnabled) hittable=\(element.isHittable) frame=\(element.frame)") }
        let toolsExist = !toolButtons.isEmpty
        let anyToolsEnabled = toolButtons.contains { $0.isEnabled }
        print("safe-scroll-geometry entry=\(prior) timeline=\(viewport) visible=\(visible) owner=\(sameOwner) tools-count=\(toolElements.count) tools-enabled=\(anyToolsEnabled)")
        print("safe-scroll-before parked=\(parked.exists) timeline-hittable=\(timeline.isHittable) entry-hittable=\(entry.isHittable) frame-y=\(prior.minY) send-exists=\(send.exists) send-enabled=\(send.exists && send.isEnabled)")
        let from = timeline.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.32))
        let to = timeline.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.78))
        from.press(forDuration: 0.1, thenDragTo: to)
        let after = entry.frame
        let moved = entry.exists && abs(after.minY-prior.minY) > 5
        print("safe-scroll-after parked=\(parked.exists) same-entry=\(entry.exists && entry.identifier == id) frame-y=\(after.minY) moved=\(moved) send-enabled=\(send.exists && send.isEnabled)")
        let sendExistsWhileParked = send.exists
        let sendEnabledWhileParked = send.exists && send.isEnabled
        let toolsExistsWhileParked = toolsExist
        let toolsEnabledWhileParked = anyToolsEnabled
        let stayedParked = parked.exists
        XCTContext.runActivity(named: "QUALIFICATION_RECONNECT_NOW") { _ in }
        let recovered = ready.waitForExistence(timeout: 60)
        print("safe-scroll-reconnect ready=\(recovered) parked=\(parked.exists)")
        XCTAssertTrue(recovered, "Explicit reconnect must restore fresh content")
        XCTAssertFalse(parked.exists, "Client must be unparked")
        XCTAssertTrue(offlineVisible && !spinnerVisible, "Cached parked thread must show Offline without a spinner")
        XCTAssertTrue(visible && sameOwner && stayedParked, "Visible same-owner entry must remain parked during gesture")
        XCTAssertTrue(sendExistsWhileParked && toolsExistsWhileParked, "Send and tools must exist")
        XCTAssertFalse(toolsEnabledWhileParked, "Tools must remain disabled while parked")
        XCTAssertTrue(moved, "Parked cached timeline should allow local reading scroll")
        XCTAssertFalse(sendEnabledWhileParked, "Sending must remain disabled while parked")
    }
}

private extension CGRect { var isFiniteRect: Bool { !isNull && !isInfinite && [minX, minY, width, height].allSatisfy { $0.isFinite } && width > 0 && height > 0 } }
