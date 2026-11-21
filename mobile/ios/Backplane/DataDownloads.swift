import SwiftUI
import UIKit

// Downloads of files on the hub's machine (docs/file-links.md): a file a reply names (`/file?path=`), a share (`/s/<token>/<name>`) or a Files row
// (`/files/get`), over HTTP on the hub the app is connected to, with the device's token. Streamed into `<name>.part` in the app's caches, never held in
// memory; after a dropped connection it goes on from where it stopped (FileFetch: Range and If-Range, 206 appends, 200 starts over). When it is whole the
// system's share sheet comes up (Save to Files, Open in another app), and the row keeps a Share button for it.
@MainActor
@Observable
final class DDownloads {
    static let shared = DDownloads()
    struct Row: Identifiable, Equatable {
        enum State { case run, done, fail }
        var id: String; var name: String; var have: Int64; var total: Int64; var state: State; var note = ""; var file: URL?
        var fill: Double { total > 0 ? min(1, max(0, Double(have) / Double(total))) : 0 }
    }
    var rows: [Row] = []
    @ObservationIgnored private var tasks: [String: Task<Void, Never>] = [:]
    @ObservationIgnored private var asks: [String: (route: String, query: [URLQueryItem])] = [:]

    /// A file a reply names: `/file?path=` (and `thread=` for a thread on another machine).
    func file(_ f: DFileLink, thread: String) {
        start(route: "/file", query: FileFetch.fileQuery(path: f.path, thread: thread), name: f.name.isEmpty ? (f.path as NSString).lastPathComponent : f.name, size: f.size)
    }
    /// A share written in a message: its own address, no query.
    func share(_ route: String) { start(route: route, query: [], name: FileFetch.shareName(route), size: -1) }
    /// A file of the Files tab: `/files/get` (the tab's own containment).
    func project(thread: String, path: String) {
        start(route: "/files/get", query: [URLQueryItem(name: "thread", value: thread), URLQueryItem(name: "path", value: path)], name: (path as NSString).lastPathComponent, size: -1)
    }

    func start(route: String, query: [URLQueryItem], name given: String, size: Int64) {
        let name = FileFetch.safeName(given)
        let key = route + "?" + query.map { "\($0.name)=\($0.value ?? "")" }.joined(separator: "&")
        if tasks[key] != nil { return }
        asks[key] = (route, query)
        put(Row(id: key, name: name, have: 0, total: size, state: .run))
        let before = stopping.removeValue(forKey: key)
        tasks[key] = Task { [weak self] in
            // a cancelled run of the same file finishes (its last write, its file closed) before this one measures the part
            await before?.value
            let out = await DDownloads.run(key: key, route: route, query: query, name: name, size: size) { have, total in
                Task { @MainActor in if let self, self.tasks[key] != nil { self.put(Row(id: key, name: name, have: have, total: total, state: .run)) } }
            }
            guard let self, !Task.isCancelled else { return }
            self.tasks[key] = nil; self.put(out)
            if out.state == .done, let f = out.file { DDownloads.present(f) }
        }
    }
    @ObservationIgnored private var stopping: [String: Task<Void, Never>] = [:]
    func cancel(_ key: String) {
        if let t = tasks.removeValue(forKey: key) { t.cancel(); stopping[key] = t; Task { [weak self] in await t.value; if self?.stopping[key] == t { self?.stopping[key] = nil } } }
        rows.removeAll { $0.id == key }
    }
    func dismiss(_ key: String) { if tasks[key] == nil { rows.removeAll { $0.id == key } } }
    func retry(_ key: String) { guard let r = rows.first(where: { $0.id == key }), let a = asks[key] else { return }; start(route: a.route, query: a.query, name: r.name, size: r.total) }
    private func put(_ r: Row) { if let i = rows.firstIndex(where: { $0.id == r.id }) { rows[i] = r } else { rows.append(r) } }

    private static func dir(_ key: String) -> URL {
        let h = key.utf8.reduce(UInt64(1469598103934665603)) { ($0 ^ UInt64($1)) &* 1099511628211 }
        return FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("downloads").appendingPathComponent(String(h, radix: 16))
    }
    /// Older downloads in the caches go after a day (never one running: each touches its folder as it starts).
    private static func sweep(keep: URL, live: Set<URL>) {
        let fm = FileManager.default; let root = keep.deletingLastPathComponent(); let day = Date().addingTimeInterval(-86400)
        try? fm.setAttributes([.modificationDate: Date()], ofItemAtPath: keep.path)
        let live = Set(live.map(\.standardizedFileURL.path))
        for d in (try? fm.contentsOfDirectory(at: root, includingPropertiesForKeys: [.contentModificationDateKey])) ?? [] where !live.contains(d.standardizedFileURL.path) {
            if let m = try? d.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate, m < day { try? fm.removeItem(at: d) }
        }
    }

    nonisolated private static func bytes(_ u: URL) -> Int64 { ((try? FileManager.default.attributesOfItem(atPath: u.path))?[.size] as? NSNumber)?.int64Value ?? 0 }

    private static func run(key: String, route: String, query: [URLQueryItem], name: String, size: Int64, progress: @escaping @Sendable (Int64, Int64) -> Void) async -> Row {
        // the file and its part in a folder of their own, the tag beside it: no name the hub sends can be the tag's
        let fm = FileManager.default; let d = dir(key); let files = d.appendingPathComponent("f"); let part = files.appendingPathComponent(name + ".part"); let tagFile = d.appendingPathComponent("etag")
        try? fm.createDirectory(at: files, withIntermediateDirectories: true)
        sweep(keep: d, live: Set((shared.tasks.keys.map { $0 } + shared.stopping.keys.map { $0 }).map(dir)).union([d]))
        let out = await FileFetchRun.fetch(where: { @MainActor in
            guard let link = DataClient.shared.link, let base = Pairing.http(link, path: "/", query: query, token: false), let url = FileFetch.url(base: base, route: route) else { return nil }
            return FileFetchRun.Where(url: url, token: FileFetch.isShare(route) ? nil : Pairing.token(link))
        }, part: part, tagFile: tagFile, size: size, offline: "Not connected to the hub.", failed: "That link did not open.", progress: progress)
        switch out {
        case .fail(let why): return Row(id: key, name: name, have: bytes(part), total: size, state: .fail, note: why)
        case .whole:
            let done = files.appendingPathComponent(name)
            try? fm.removeItem(at: done)
            do { try fm.moveItem(at: part, to: done) } catch { return Row(id: key, name: name, have: bytes(part), total: size, state: .fail, note: error.localizedDescription) }
            try? fm.removeItem(at: tagFile)
            let n = bytes(done)
            return Row(id: key, name: name, have: n, total: n, state: .done, file: done)
        }
    }

    /// The system's share sheet for a finished file: Save to Files, AirDrop, Open in another app.
    static func present(_ f: URL) {
        let vc = UIActivityViewController(activityItems: [f], applicationActivities: nil)
        let top = UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.keyWindow?.rootViewController }.first
        var at = top; while let p = at?.presentedViewController { at = p }
        vc.popoverPresentationController?.sourceView = at?.view
        if let v = at?.view { vc.popoverPresentationController?.sourceRect = CGRect(x: v.bounds.midX, y: v.bounds.maxY - 80, width: 1, height: 1) }
        at?.present(vc, animated: true)
    }
}

/// The downloads under way and just done, over the screen's foot: each with its progress, then Share, or the hub's words and Retry.
struct DDownloadStrip: View {
    var d = DDownloads.shared
    private func mb(_ n: Int64) -> String { n < 0 ? "?" : n < 1024 ? "\(n) B" : n < 1 << 20 ? "\(n / 1024) KB" : String(format: "%.1f MB", Double(n) / 1048576) }
    var body: some View {
        if !d.rows.isEmpty {
            VStack(alignment: .leading, spacing: 0) {
                ForEach(d.rows) { r in
                    VStack(alignment: .leading, spacing: 6) {
                        HStack(spacing: 6) {
                            Text(r.name).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                            switch r.state {
                            case .run:
                                Text(r.total > 0 ? Tr.android_download_of(have: mb(r.have), total: mb(r.total)) : mb(r.have)).dmono(11).foregroundStyle(BackplaneDesign.dim)
                                Button(Tr.t("Cancel")) { d.cancel(r.id) }.buttonStyle(DButton(compact: true)).accessibilityIdentifier("download.cancel")
                            case .done:
                                Button(Tr.t("Share")) { if let f = r.file { DDownloads.present(f) } }.buttonStyle(DButton(primary: true, compact: true)).accessibilityIdentifier("download.share")
                                Button { d.dismiss(r.id) } label: { Image(systemName: "xmark") }.buttonStyle(DButton(compact: true)).accessibilityIdentifier("download.close")
                            case .fail:
                                Button(Tr.t("Retry")) { d.retry(r.id) }.buttonStyle(DButton(compact: true)).accessibilityIdentifier("download.retry")
                                Button { d.dismiss(r.id) } label: { Image(systemName: "xmark") }.buttonStyle(DButton(compact: true)).accessibilityIdentifier("download.close")
                            }
                        }
                        if r.state == .run {
                            GeometryReader { g in ZStack(alignment: .leading) { Rectangle().fill(BackplaneDesign.line); Rectangle().fill(BackplaneDesign.accent).frame(width: g.size.width * r.fill) } }.frame(height: 3)
                        }
                        if !r.note.isEmpty { Text(Tr.t(r.note)).dmono(11).foregroundStyle(r.state == .fail ? BackplaneDesign.bad : BackplaneDesign.dim).lineLimit(3) }
                    }.padding(.horizontal, 12).padding(.vertical, 8).accessibilityIdentifier("download.\(r.name)")
                }
            }
            .background(BackplaneDesign.raised).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
            .padding(.bottom, 118).padding(.horizontal, 12).accessibilityIdentifier("downloads")
        }
    }
}
