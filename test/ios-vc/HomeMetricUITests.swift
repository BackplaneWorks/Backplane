import XCTest
import UIKit

final class HomeMetricUITests: XCTestCase {
    func testHomeMetricLayoutAndAudit() throws {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
        app.launch()
        XCTAssertFalse(app.textFields["pair.address"].exists)
        let tab = app.buttons["tab.home"]
        XCTAssertTrue(tab.waitForExistence(timeout: 30)); tab.tap()
        if app.navigationBars["Bub"].exists {
            let back = app.navigationBars.buttons.firstMatch
            XCTAssertTrue(back.waitForExistence(timeout: 10)); back.tap()
        }
        XCTAssertTrue(tab.isSelected, "Home must be selected")
        let ids = ["need", "running", "working", "rooms"]
        var frames: [CGRect] = []
        for id in ids { for part in ["label", "value"] {
            let label = app.descendants(matching: .any)["home.metric." + part + "." + id]
            XCTAssertTrue(label.waitForExistence(timeout: 30), "Home metric label must exist")
            XCTAssertFalse(label.label.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            let f = label.frame
            print("metric-frame part=\(part) id=\(id) x=\(f.minX) y=\(f.minY) w=\(f.width) h=\(f.height)")
            XCTAssertGreaterThan(f.width, 0); XCTAssertGreaterThan(f.height, 0)
            XCTAssertTrue(app.frame.contains(f), "Metric frame must fit viewport")
            frames.append(f)
        }}
        for i in frames.indices { for j in frames.indices where j > i {
            XCTAssertFalse(frames[i].intersects(frames[j]), "Metric labels must not overlap")
        }}
        // Crop in memory to the four generic metric labels only; never attach thread contents.
        let union = frames.dropFirst().reduce(frames[0]) { $0.union($1) }.insetBy(dx: -4, dy: -4)
        let screenshot = app.screenshot().image
        let factorX = screenshot.size.width * screenshot.scale / app.frame.width
        let factorY = screenshot.size.height * screenshot.scale / app.frame.height
        let crop = CGRect(x: union.minX * factorX, y: union.minY * factorY, width: union.width * factorX, height: union.height * factorY).integral
        if let image = screenshot.cgImage?.cropping(to: crop) {
            let attachment = XCTAttachment(image: UIImage(cgImage: image)); attachment.name = "home-metric-labels"; attachment.lifetime = .keepAlways; add(attachment)
        } else { XCTFail("Could not crop Home metric evidence") }
        var metricIssues = 0, otherIssues = 0
        do {
            try app.performAccessibilityAudit(for: .all) { issue in
                let element = issue.element; let identifier = element?.identifier ?? ""
                let matched = ids.first { identifier == "home.metric.label." + $0 || identifier == "home.metric.value." + $0 }
                let issueFrame = element?.frame ?? .null
                let overlap = !issueFrame.isNull && !issueFrame.isInfinite && frames.contains { !$0.intersection(issueFrame).isEmpty }
                print("metric-audit-overlap \(overlap)")
                if matched != nil { metricIssues += 1 } else { otherIssues += 1 }
                let f = element?.frame ?? .null
                let geometry = f.isNull || f.isInfinite ? "unavailable" : "\(f.minX),\(f.minY),\(f.width),\(f.height)"
                print("metric-audit type=\(issue.auditType.rawValue) elementType=\(element.map { Int($0.elementType.rawValue) } ?? -1) category=\(matched ?? "other") frame=\(geometry)")
                return true
            }
        } catch { XCTFail("Accessibility audit could not complete"); return }
        print("metric-audit-summary metric=\(metricIssues) other=\(otherIssues)")
        XCTAssertEqual(metricIssues, 0, "Home metric labels have audit findings")
    }
}
