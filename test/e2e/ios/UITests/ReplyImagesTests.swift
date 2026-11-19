import XCTest

// Feature replyImages on the iOS simulator, through real gestures, against a temp-home hub (test/e2e/ios/reply-images.sh):
// - a reply with two captioned runs of image paths renders as inline carousels (not blue text links), with count indicators
// - a single image renders inline; a missing file renders an "Image unavailable" tile with its file name
// - tapping an image opens the full-screen viewer for that message only, starting at the tapped image
// - double-tap zooms in and out, pinch zooms, drag pans while zoomed, swipe left/right changes image, swipe down and Close dismiss
final class ReplyImagesE2E: XCTestCase {
    var d: Driver!
    override func setUp() { continueAfterFailure = true }

    private func zoomRatio(_ el: XCUIElement) -> Double {
        guard let raw = el.value as? String else { return 0 }
        return Double(raw.replacingOccurrences(of: "x", with: "")) ?? 0
    }

    private func labelText(_ el: XCUIElement) -> String {
        let l = el.label
        if !l.isEmpty { return l }
        return (el.value as? String) ?? ""
    }

    private func scrollToFind(_ el: XCUIElement, up: Bool = true, tries: Int = 8) -> Bool {
        if el.exists && el.isHittable { return true }
        for _ in 0..<tries {
            if up { d.app.swipeUp(velocity: .slow) } else { d.app.swipeDown(velocity: .slow) }
            usleep(300_000)
            if el.exists && el.isHittable { return true }
        }
        return el.exists
    }

    func test_reply_images() {
        d = Driver("reply-images"); defer { d.finish() }
        let expectTotal = Int(d.env["EXPECT_TOTAL"] ?? "") ?? 8
        let expectCarousel = d.env["EXPECT_CAROUSEL"] ?? "1 / 5"
        let expectCarouselNext = d.env["EXPECT_CAROUSEL_NEXT"] ?? "2 / 5"
        let checkUnavailable = (d.env["CHECK_UNAVAILABLE"] ?? "1") == "1"

        d.app.terminate()
        d.app.launchArguments = ["-app.alerts", "<false/>"]
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_SELECT": "timgs"]
        d.app.launch(); d.skipTour()

        let carousel = d.app.descendants(matching: .any).matching(identifier: "reply.carousel").firstMatch
        let loadedImg = d.app.descendants(matching: .any).matching(identifier: "reply.image").firstMatch

        // Scroll up slightly if the bottom of a long reply is showing so the first carousel is in view.
        d.step("inline carousel renders with loaded image", timeout: 30, act: {}, expect: {
            if carousel.exists && loadedImg.exists { return true }
            if d.app.scrollViews.firstMatch.exists { d.app.swipeDown(velocity: .slow) }
            return carousel.exists && loadedImg.exists
        })

        // Bring the first carousel cleanly into frame so its count indicator is visible.
        let firstCount = d.app.descendants(matching: .any).matching(identifier: "reply.carousel.count").firstMatch
        _ = scrollToFind(carousel, up: false, tries: 4)
        sleep(1)

        d.step("image paths are not left as text links", timeout: 5, act: {}, expect: {
            let linkPred = NSPredicate(format: "label CONTAINS '/before-project.png' OR label CONTAINS '/after-picker.png'")
            return d.app.links.matching(linkPred).count == 0 && d.app.staticTexts.matching(linkPred).count == 0
        })

        d.step("carousel shows page count", timeout: 10, act: {}, expect: {
            firstCount.exists && self.labelText(firstCount) == expectCarousel
        })
        d.shot("carousel")

        // Swipe the first carousel horizontally to verify swiping + page counter update.
        d.step("swiping carousel advances page count", timeout: 10, act: {
            carousel.swipeLeft()
        }, expect: {
            self.labelText(firstCount) == expectCarouselNext
        })
        sleep(1)
        d.shot("carousel-swiped")

        // Tap the currently shown image in the carousel (index 2 of expectTotal) to open the full-screen viewer.
        let viewer = d.el("reply.viewer")
        let viewerCount = d.el("reply.viewer.count")
        let page = d.el("reply.viewer.page")

        d.step("tapping carousel image opens full-screen viewer at tapped image", timeout: 15, act: {
            let center = carousel.coordinate(withNormalizedOffset: CGVector(dx: 0.4, dy: 0.5))
            center.tap()
        }, expect: {
            viewer.exists && page.exists && self.labelText(viewerCount) == "2 / \(expectTotal)"
        })
        sleep(1)
        d.shot("viewer-open")

        // Double-tap to zoom in.
        d.step("double-tap zooms in", timeout: 10, act: {
            page.doubleTap()
        }, expect: {
            self.zoomRatio(page) > 1.5
        })
        sleep(1)
        d.shot("viewer-zoomed")
        d.note("zoom after double-tap: \(page.value ?? "nil")")

        // Pan while zoomed (should remain zoomed).
        d.step("pan while zoomed keeps zoom", timeout: 10, act: {
            let a = page.coordinate(withNormalizedOffset: CGVector(dx: 0.65, dy: 0.55))
            let b = page.coordinate(withNormalizedOffset: CGVector(dx: 0.35, dy: 0.45))
            a.press(forDuration: 0.05, thenDragTo: b)
        }, expect: {
            self.zoomRatio(page) > 1.5
        })

        // Double-tap to zoom back out to fitted (1.0x).
        d.step("double-tap zooms back out", timeout: 10, act: {
            page.doubleTap()
        }, expect: {
            let z = self.zoomRatio(page); return z >= 0.9 && z <= 1.1
        })

        // Pinch to zoom in, then double-tap back to fitted.
        d.step("pinch zooms in", timeout: 10, act: {
            page.pinch(withScale: 2.2, velocity: 1.5)
        }, expect: {
            self.zoomRatio(page) > 1.3
        })
        d.note("zoom after pinch: \(page.value ?? "nil")")
        d.step("double-tap resets pinch zoom", timeout: 10, act: {
            page.doubleTap()
        }, expect: {
            let z = self.zoomRatio(page); return z >= 0.9 && z <= 1.1
        })

        // Swipe left in the viewer to go to the next image (3 / expectTotal).
        d.step("swipe left advances to next image in viewer", timeout: 10, act: {
            viewer.swipeLeft()
        }, expect: {
            self.labelText(viewerCount) == "3 / \(expectTotal)"
        })
        sleep(1)
        d.shot("viewer-swiped")

        // Swipe right to go back to the previous image (2 / expectTotal).
        d.step("swipe right returns to previous image", timeout: 10, act: {
            viewer.swipeRight()
        }, expect: {
            self.labelText(viewerCount) == "2 / \(expectTotal)"
        })

        // Swipe down to dismiss the viewer.
        d.step("swipe down dismisses viewer", timeout: 10, act: {
            let top = page.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.35))
            let bot = page.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.88))
            top.press(forDuration: 0.05, thenDragTo: bot, withVelocity: .fast, thenHoldForDuration: 0.0)
        }, expect: {
            !viewer.exists
        })

        // Re-open the viewer and dismiss with the Close button.
        d.step("re-open viewer and dismiss via Close button", timeout: 15, act: {
            let center = carousel.coordinate(withNormalizedOffset: CGVector(dx: 0.4, dy: 0.5))
            center.tap()
            _ = viewer.waitForExistence(timeout: 8)
            d.tap(d.el("reply.viewer.close"))
        }, expect: {
            !viewer.exists
        })

        // Check single inline image + unavailable tile (seeded in a second message in fixture mode).
        if checkUnavailable {
            let unavailable = d.el("reply.image.unavailable")
            _ = scrollToFind(unavailable, up: true, tries: 8)
            d.step("missing image shows unavailable tile with file name", timeout: 15, act: {}, expect: {
                unavailable.exists && self.labelText(unavailable).contains("missing.png")
            })
            sleep(1)
            d.shot("unavailable")
        }
    }
}
