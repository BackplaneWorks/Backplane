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
        tasks[key] = Task { [weak self] in
            let out = await DDownloads.run(key: key, route: route, query: query, name: name, size: size) { have, total in
                Task { @MainActor in if let self, self.tasks[key] != nil { self.put(Row(id: key, name: name, have: have, total: total, state: .run)) } }
            }
            guard let self, !Task.isCancelled else { return }
            self.tasks[key] = nil; self.put(out)
            if out.state == .done, let f = out.file { DDownloads.present(f) }
        }
    }
    func cancel(_ key: String) { tasks.removeValue(forKey: key)?.cancel(); rows.removeAll { $0.id == key } }
    func dismiss(_ key: String) { if tasks[key] == nil { rows.removeAll { $0.id == key } } }
    func retry(_ key: String) { guard let r = rows.first(where: { $0.id == key }), let a = asks[key] else { return }; start(route: a.route, query: a.query, name: r.name, size: r.total) }
    private func put(_ r: Row) { if let i = rows.firstIndex(where: { $0.id == r.id }) { rows[i] = r } else { rows.append(r) } }

    private static func dir(_ key: String) -> URL {
        let h = key.utf8.reduce(UInt64(1469598103934665603)) { ($0 ^ UInt64($1)) &* 1099511628211 }
        return FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("downloads").appendingPathComponent(String(h, radix: 16))
    }
    /// Older downloads in the caches go after a day (never the one running).
    private static func sweep(keep: URL) {
        let fm = FileManager.default; let root = keep.deletingLastPathComponent(); let day = Date().addingTimeInterval(-86400)
        for d in (try? fm.contentsOfDirectory(at: root, includingPropertiesForKeys: [.contentModificationDateKey])) ?? [] where d != keep {
            if let m = try? d.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate, m < day { try? fm.removeItem(at: d) }
        }
    }

    nonisolated private static func bytes(_ u: URL) -> Int64 { ((try? FileManager.default.attributesOfItem(atPath: u.path))?[.size] as? NSNumber)?.int64Value ?? 0 }

    private static func run(key: String, route: String, query: [URLQueryItem], name: String, size: Int64, progress: @escaping @Sendable (Int64, Int64) -> Void) async -> Row {
        let fm = FileManager.default; let d = dir(key); let part = d.appendingPathComponent(name + ".part"); let tagFile = d.appendingPathComponent("etag")
        try? fm.createDirectory(at: d, withIntermediateDirectories: true); sweep(keep: d)
        func fail(_ why: String) -> Row { Row(id: key, name: name, have: bytes(part), total: size, state: .fail, note: why) }
        var tries = 0; var total = size
        while !Task.isCancelled {
            guard let link = DataClient.shared.link, let url = Pairing.http(link, path: route, query: query, token: false) else { return fail("Not connected to the hub.") }
            let have = fm.fileExists(atPath: part.path) ? bytes(part) : 0
            let tag = (try? String(contentsOf: tagFile, encoding: .utf8)) ?? ""
            let ask = FileFetch.ask(have: have, etag: tag)
            var req = URLRequest(url: url); req.timeoutInterval = 60
            if let t = Pairing.token(link) { req.setValue("Bearer " + t, forHTTPHeaderField: "Authorization") }
            for (k, v) in ask { req.setValue(v, forHTTPHeaderField: k) }
            let one = DFetchOne(part: part, tagFile: tagFile, from: ask.isEmpty ? 0 : have, progress: progress)
            let (step, got, err) = await one.go(req)
            if case .write(_, let t) = step, t >= 0 { total = t }
            if got > 0 { tries = 0 }   // it moved on: a dropped connection starts its count again
            switch step {
            case .fail(let why): return fail(why)
            case .again:
                try? fm.removeItem(at: part); try? fm.removeItem(at: tagFile)
                tries += 1; if tries >= FileFetch.tries { return fail("That link did not open.") }; continue
            case .done: break
            case .write:
                if err != nil || !(FileFetch.complete(have: bytes(part), total: total) || total < 0) {
                    // the connection dropped, or the body ended short (the file changed): ask for the rest
                    tries += 1; if tries >= FileFetch.tries { return fail(err?.localizedDescription ?? "That link did not open.") }
                    try? await Task.sleep(nanoseconds: UInt64(FileFetch.backoff(tries) * 1e9)); continue
                }
            }
            let done = d.appendingPathComponent(name)
            try? fm.removeItem(at: done)
            do { try fm.moveItem(at: part, to: done) } catch { return fail(error.localizedDescription) }
            try? fm.removeItem(at: tagFile)
            let n = bytes(done)
            return Row(id: key, name: name, have: n, total: n, state: .done, file: done)
        }
        return fail("That link did not open.")
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

/// One request of a download: the answer's head decides (FileFetch.step) whether its body is written to the `.part` (appended, or from the start),
/// and the body goes straight to the file as it comes. A refusal's body (the hub's words) is kept, at most 4 KiB.
private final class DFetchOne: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    let part: URL; let tagFile: URL; let from: Int64; let progress: @Sendable (Int64, Int64) -> Void
    private var handle: FileHandle?; private var step: FileFetch.Step = .fail("That link did not open."); private var code = 0
    private var refusal = Data(); private var got: Int64 = 0; private var at: Int64 = 0; private var total: Int64 = -1; private var shown = Date.distantPast
    private var cont: CheckedContinuation<(FileFetch.Step, Int64, Error?), Never>?
    private var task: URLSessionDataTask?
    init(part: URL, tagFile: URL, from: Int64, progress: @escaping @Sendable (Int64, Int64) -> Void) { self.part = part; self.tagFile = tagFile; self.from = from; self.progress = progress }

    func go(_ req: URLRequest) async -> (FileFetch.Step, Int64, Error?) {
        let q = OperationQueue(); q.maxConcurrentOperationCount = 1
        let s = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: q)
        defer { s.finishTasksAndInvalidate() }
        return await withTaskCancellationHandler {
            await withCheckedContinuation { c in q.addOperation { self.cont = c; let t = s.dataTask(with: req); self.task = t; t.resume() } }
        } onCancel: { q.addOperation { self.task?.cancel() } }
    }

    func urlSession(_ s: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse, completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        let h = response as? HTTPURLResponse; code = h?.statusCode ?? 0
        guard (200..<300).contains(code) || code == 416 else { completionHandler(.allow); return }   // a refusal: its words come as the body
        step = FileFetch.step(code: code, have: from, contentRange: h?.value(forHTTPHeaderField: "Content-Range"), contentLength: response.expectedContentLength)
        guard case .write(let a, let t) = step else { completionHandler(.cancel); return }
        let fm = FileManager.default
        if a == 0 || !fm.fileExists(atPath: part.path) { fm.createFile(atPath: part.path, contents: nil) }
        if let tag = h?.value(forHTTPHeaderField: "ETag") { try? tag.write(to: tagFile, atomically: true, encoding: .utf8) } else { try? fm.removeItem(at: tagFile) }
        guard let fh = try? FileHandle(forWritingTo: part) else { step = .fail("That link did not open."); completionHandler(.cancel); return }
        if a > 0 { _ = try? fh.seekToEnd() } else { try? fh.truncate(atOffset: 0) }
        handle = fh; at = a
        total = t >= 0 ? t : (response.expectedContentLength >= 0 ? a + response.expectedContentLength : -1)
        step = .write(from: a, total: total)
        completionHandler(.allow)
    }
    func urlSession(_ s: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard let fh = handle else { if refusal.count < 4096 { refusal.append(data.prefix(4096 - refusal.count)) }; return }
        do { try fh.write(contentsOf: data) } catch { dataTask.cancel(); return }
        got += Int64(data.count)
        if Date().timeIntervalSince(shown) > 0.25 { shown = Date(); progress(at + got, total) }
    }
    func urlSession(_ s: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        try? handle?.close(); handle = nil
        if !(200..<300).contains(code) && code != 416 && code != 0 { step = FileFetch.step(code: code, have: from, contentRange: nil, contentLength: 0, body: String(decoding: refusal, as: UTF8.self)) }
        // a cancel this request asked for (a .done or .again head) is not an error
        let e: Error? = { if case .write = step { return error }; return nil }()
        if code == 0, let error { cont?.resume(returning: (.write(from: from, total: -1), 0, error)); cont = nil; return }
        if case .write = step { progress(at + got, total) }
        cont?.resume(returning: (step, got, e)); cont = nil
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
