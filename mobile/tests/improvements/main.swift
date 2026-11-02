import Foundation

func expect(_ value: @autoclosure () -> Bool, _ message: String) {
    precondition(value(), message)
}
let source = "  Report + & ? # / 日本語 👩🏽‍💻\r\n\nkeep trailing spaces  \n"
let body = source + "\n\n---\nOS: iOS\nApp version: 1.2.3\n"
let allowed = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.!~*'()")
let encoded = body.addingPercentEncoding(withAllowedCharacters: allowed)!
let url = "https://github.com/BackplaneWorks/Backplane/issues/new?title=Backplane%20improvement&body=" + encoded
expect(ImprovementsPreview(body: body, url: url).destination != nil, "Exact Unicode body must round-trip")
expect(ImprovementsPreview(body: body + " ", url: url).destination == nil, "Reject preview/URL mismatch")
for bad in [url.replacingOccurrences(of: "github.com", with: "github.com.evil.test"),
            url.replacingOccurrences(of: "https:", with: "http:"), url + "&body=other", url + "&token=private",
            url + "#fragment", url.replacingOccurrences(of: "BackplaneWorks", with: "another-owner"),
            url.replacingOccurrences(of: "github.com", with: "name@github.com")] {
    expect(ImprovementsPreview(body: body, url: bad).destination == nil, "Reject unapproved destination/query")
}
expect(ImprovementsDraft().target == .github, "New unsent draft defaults public")
expect(!ImprovementsDraft().screenshotConsent, "Screenshot must start unchecked")
let legacy = try JSONDecoder().decode(ImprovementsDraft.self, from: Data("{\"text\":\"saved internal\"}".utf8))
expect(legacy.target == .internal, "Existing missing-target draft must stay internal")
let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
defer { try? FileManager.default.removeItem(at: dir) }
let store = ImprovementsStore(directory: dir)
var saved = ImprovementsDraft()
saved.target = .internal; saved.text = source; saved.report = body
saved.evidenceDetails = "1. Open the form.\n2. Observe missing text."
saved.evidenceReviewed = true; saved.opened = true
try store.save(saved)
let reread = try store.read()
expect(reread == saved, "Exact draft, target, evidence and pre-open receipt survive restart")
let preview = ImprovementsPreview(body: body, url: url)
expect(!reread.canOpen(preview, hasImage: true), "Saved opening receipt blocks a duplicate open")
saved.opened = false
expect(saved.canOpen(preview, hasImage: false), "Reviewed detailed evidence allows opening")
saved.evidenceReviewed = false
expect(!saved.canOpen(preview, hasImage: true), "Image still needs explicit evidence review")
saved.evidenceReviewed = true; saved.evidenceDetails = ""
expect(!saved.canOpen(preview, hasImage: false), "No evidence blocks opening")
expect(saved.canOpen(preview, hasImage: true), "Explicit image evidence is optional alternative")
try Data("not JSON".utf8).write(to: dir.appendingPathComponent("draft.json"))
do { _ = try store.read(); preconditionFailure("Corrupt saved draft must not become a new public draft") }
catch {}
print("iOS Improvements model checks passed; no browser or GitHub request executed")
