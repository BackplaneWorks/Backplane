import Foundation

// One download, start to whole (docs/file-links.md, "Resuming downloads"): the requests FileFetch plans, each answer's body streamed straight into the
// `.part` file, never held in memory. Foundation only, so mobile/ios/Tests/FileFetchLiveTests.swift runs this same code against a live hub.
enum FileFetchRun {
    /// Where to ask, as it is now (the hub the app is connected to may change between tries); nil: not connected.
    struct Where { var url: URL; var token: String? }
    enum Out: Equatable { case whole(URL); case fail(String) }
    /// Each answer's head as it comes (the Range asked, or "-", and the status): for FileFetchLiveTests only.
    nonisolated(unsafe) static var heard: ((String, Int) -> Void)?
    nonisolated static func bytes(_ u: URL) -> Int64 { ((try? FileManager.default.attributesOfItem(atPath: u.path))?[.size] as? NSNumber)?.int64Value ?? 0 }

    static func fetch(where: @escaping () async -> Where?, part: URL, tagFile: URL, size: Int64, offline: String, failed: String,
                      progress: @escaping @Sendable (Int64, Int64) -> Void = { _, _ in }) async -> Out {
        let fm = FileManager.default
        var tries = 0; var total = size
        while !Task.isCancelled {
            guard let w = await `where`() else { return .fail(offline) }
            let have = fm.fileExists(atPath: part.path) ? bytes(part) : 0
            let tag = (try? String(contentsOf: tagFile, encoding: .utf8)) ?? ""
            let ask = FileFetch.ask(have: have, etag: tag)
            var req = URLRequest(url: w.url); req.timeoutInterval = 60
            if let t = w.token { req.setValue("Bearer " + t, forHTTPHeaderField: "Authorization") }
            for (k, v) in ask { req.setValue(v, forHTTPHeaderField: k) }
            let one = DFetchOne(part: part, tagFile: tagFile, from: ask.isEmpty ? 0 : have, progress: progress)
            let (step, got, err) = await one.go(req)
            if case .write(_, let t) = step, t >= 0 { total = t }
            if got > 0 { tries = 0 }   // it moved on: a dropped connection starts its count again
            switch step {
            case .fail(let why): return .fail(why)
            case .again:
                try? fm.removeItem(at: part); try? fm.removeItem(at: tagFile)
                tries += 1; if tries >= FileFetch.tries { return .fail(failed) }; continue
            case .done: return .whole(part)
            case .write:
                if err == nil, FileFetch.complete(have: bytes(part), total: total) || total < 0 { return .whole(part) }
                // the connection dropped, or the body ended short (the file changed): ask for the rest
                tries += 1; if tries >= FileFetch.tries { return .fail(err?.localizedDescription ?? failed) }
                try? await Task.sleep(nanoseconds: UInt64(FileFetch.backoff(tries) * 1e9))
            }
        }
        return .fail(failed)
    }
}

/// One request of a download: the answer's head decides (FileFetch.step) whether its body is written to the `.part` (appended, or from the start),
/// and the body goes straight to the file as it comes. A refusal's body (the hub's words) is kept, at most 4 KiB.
final class DFetchOne: NSObject, URLSessionDataDelegate, @unchecked Sendable {
    let part: URL; let tagFile: URL; let from: Int64; let progress: @Sendable (Int64, Int64) -> Void
    private var handle: FileHandle?; private var step: FileFetch.Step = .fail("That link did not open."); private var code = 0
    private var refusal = Data(); private var got: Int64 = 0; private var at: Int64 = 0; private var total: Int64 = -1; private var shown = Date.distantPast
    private var cont: CheckedContinuation<(FileFetch.Step, Int64, Error?), Never>?
    private var task: URLSessionDataTask?
    private var stopped = false
    init(part: URL, tagFile: URL, from: Int64, progress: @escaping @Sendable (Int64, Int64) -> Void) { self.part = part; self.tagFile = tagFile; self.from = from; self.progress = progress }

    func go(_ req: URLRequest) async -> (FileFetch.Step, Int64, Error?) {
        let q = OperationQueue(); q.maxConcurrentOperationCount = 1
        let s = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: q)
        defer { s.finishTasksAndInvalidate() }
        return await withTaskCancellationHandler {
            await withCheckedContinuation { c in q.addOperation {
                // a cancel that came before this start is kept, not lost: the request is never made
                if self.stopped || Task.isCancelled { c.resume(returning: (.fail("cancelled"), 0, nil)); return }
                self.cont = c; let t = s.dataTask(with: req); self.task = t; t.resume() } }
        } onCancel: { q.addOperation { self.stopped = true; self.task?.cancel() } }
    }

    func urlSession(_ s: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse, completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        let h = response as? HTTPURLResponse; code = h?.statusCode ?? 0
        FileFetchRun.heard?(dataTask.originalRequest?.value(forHTTPHeaderField: "Range") ?? "-", code)
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

