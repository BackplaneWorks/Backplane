import Foundation

// Device-local only. Never derive a public report from a screen, hub or account.
struct ImprovementsDraft: Codable, Equatable {
    enum Target: String, Codable { case github = "issue", `internal` }
    var target: Target = .github
    var text = ""
    var report = ""
    var evidenceDetails = ""
    var evidenceReviewed = false
    var screenshotConsent = false
    // Receipt is written before handing off to the OS. A relaunch never opens again.
    var opened = false

    init() {}
    func canOpen(_ preview: ImprovementsPreview, hasImage: Bool) -> Bool {
        !opened && evidenceReviewed && (hasImage || !evidenceDetails.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            && !report.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && preview.body == report && preview.destination != nil
    }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        // A pre-target saved draft belongs to the existing internal workflow.
        target = try c.decodeIfPresent(Target.self, forKey: .target) ?? .internal
        text = try c.decodeIfPresent(String.self, forKey: .text) ?? ""
        report = try c.decodeIfPresent(String.self, forKey: .report) ?? ""
        evidenceDetails = try c.decodeIfPresent(String.self, forKey: .evidenceDetails) ?? ""
        evidenceReviewed = try c.decodeIfPresent(Bool.self, forKey: .evidenceReviewed) ?? false
        screenshotConsent = try c.decodeIfPresent(Bool.self, forKey: .screenshotConsent) ?? false
        opened = try c.decodeIfPresent(Bool.self, forKey: .opened) ?? false
    }
}

struct ImprovementsPreview: Decodable {
    let body: String
    let url: String

    // Fail closed on a missing/stale bridge or a destination other than the
    // official prefilled issue form. The URL must encode the displayed body.
    var destination: URL? {
        guard let c = URLComponents(string: url), c.scheme == "https", c.host == "github.com",
              c.path == "/BackplaneWorks/Backplane/issues/new", c.user == nil, c.password == nil,
              c.port == nil, c.fragment == nil,
              url.utf8.count <= 8000,
              c.queryItems?.allSatisfy({ $0.name == "body" || $0.name == "title" }) == true,
              c.queryItems?.filter({ $0.name == "title" }).count == 1,
              c.queryItems?.first(where: { $0.name == "title" })?.value == "Backplane improvement",
              c.queryItems?.filter({ $0.name == "body" }).count == 1,
              c.queryItems?.first(where: { $0.name == "body" })?.value == body else { return nil }
        return c.url
    }
}

struct ImprovementsStore {
    let directory: URL
    init(directory: URL? = nil) {
        self.directory = directory ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Improvements", isDirectory: true)
    }
    private var draftURL: URL { directory.appendingPathComponent("draft.json") }
    var imageURL: URL { directory.appendingPathComponent("screenshot") }
    func read() throws -> ImprovementsDraft {
        guard FileManager.default.fileExists(atPath: draftURL.path) else { return ImprovementsDraft() }
        return try JSONDecoder().decode(ImprovementsDraft.self, from: Data(contentsOf: draftURL))
    }
    func save(_ draft: ImprovementsDraft) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try JSONEncoder().encode(draft).write(to: draftURL, options: .atomic)
    }
    func removeImage() { try? FileManager.default.removeItem(at: imageURL) }
}
