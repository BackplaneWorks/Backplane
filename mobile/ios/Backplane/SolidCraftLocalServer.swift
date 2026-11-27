import Foundation
import Network

final class SolidCraftLocalServer {
    let bundleDir: URL
    let manifest: SolidCraftBundleManifest
    let hubBase: URL
    let token: String
    let nonce: String

    private let queue = DispatchQueue(label: "dev.backplane.solidcraft.localserver")
    private let session: URLSession
    private var listener: NWListener?
    private var connections: [ObjectIdentifier: NWConnection] = [:]
    private var stopped = false

    static func bundleDirectory() -> URL? {
        guard let dir = Bundle.main.resourceURL?.appendingPathComponent("SolidCraftWeb.bundle", isDirectory: true),
              let data = try? Data(contentsOf: dir.appendingPathComponent("manifest.json")),
              SolidCraftBundleSupport.parseManifest(data) != nil else {
            return nil
        }
        return dir
    }

    init?(bundleDir: URL, hubBase: URL, token: String) {
        guard let data = try? Data(contentsOf: bundleDir.appendingPathComponent("manifest.json")),
              let manifest = SolidCraftBundleSupport.parseManifest(data) else {
            return nil
        }
        self.bundleDir = bundleDir.standardizedFileURL
        self.manifest = manifest
        self.hubBase = hubBase
        self.token = token
        var bytes = [UInt8](repeating: 0, count: 16)
        for i in 0..<bytes.count { bytes[i] = UInt8.random(in: 0...255) }
        self.nonce = bytes.map { String(format: "%02x", $0) }.joined()
        let cfg = URLSessionConfiguration.ephemeral
        cfg.httpShouldSetCookies = false
        cfg.urlCache = nil
        cfg.requestCachePolicy = .reloadIgnoringLocalCacheData
        self.session = URLSession(configuration: cfg)
    }

    deinit {
        stop()
    }

    func start(thread: String, path: String, light: Bool, readOnly: Bool) async throws -> URL {
        let port: UInt16 = try await withCheckedThrowingContinuation { cont in
            let params = NWParameters.tcp
            params.allowLocalEndpointReuse = true
            params.requiredLocalEndpoint = NWEndpoint.hostPort(host: NWEndpoint.Host("127.0.0.1"), port: .any)
            let listener: NWListener
            do {
                listener = try NWListener(using: params)
            } catch {
                cont.resume(throwing: error)
                return
            }
            self.listener = listener
            var settled = false
            listener.stateUpdateHandler = { [weak self] state in
                guard !settled else { return }
                switch state {
                case .ready:
                    if let p = listener.port?.rawValue, p > 0 {
                        settled = true
                        cont.resume(returning: p)
                    }
                case .failed(let err):
                    settled = true
                    self?.stop()
                    cont.resume(throwing: err)
                case .cancelled:
                    settled = true
                    cont.resume(throwing: URLError(.cancelled))
                default:
                    break
                }
            }
            listener.newConnectionHandler = { [weak self] conn in
                self?.accept(conn)
            }
            listener.start(queue: self.queue)
        }

        var comps = URLComponents()
        comps.scheme = "http"
        comps.host = "127.0.0.1"
        comps.port = Int(port)
        comps.path = "/sc/\(nonce)/"
        var q = [
            URLQueryItem(name: "thread", value: thread),
            URLQueryItem(name: "path", value: path),
            URLQueryItem(name: "theme", value: light ? "light" : "dark"),
        ]
        if readOnly { q.append(URLQueryItem(name: "readonly", value: "1")) }
        comps.queryItems = q
        guard let url = comps.url else { throw URLError(.badURL) }
        return url
    }

    func stop() {
        queue.async { [weak self] in
            guard let self, !self.stopped else { return }
            self.stopped = true
            self.listener?.cancel()
            self.listener = nil
            for conn in self.connections.values {
                conn.cancel()
            }
            self.connections.removeAll()
            self.session.invalidateAndCancel()
        }
    }

    private func accept(_ conn: NWConnection) {
        let key = ObjectIdentifier(conn)
        connections[key] = conn
        conn.stateUpdateHandler = { [weak self] st in
            if case .failed = st { self?.connections.removeValue(forKey: key) }
            else if case .cancelled = st { self?.connections.removeValue(forKey: key) }
        }
        conn.start(queue: queue)
        readHead(conn, buffer: Data())
    }

    private func readHead(_ conn: NWConnection, buffer: Data) {
        conn.receive(minimumIncompleteLength: 1, maximumLength: 65536) { [weak self] data, _, isComplete, err in
            guard let self, !self.stopped else { conn.cancel(); return }
            if err != nil { self.close(conn); return }
            var buf = buffer
            if let data { buf.append(data) }
            if let range = buf.range(of: Data("\r\n\r\n".utf8)) {
                let headData = buf.subdata(in: 0..<range.lowerBound)
                let bodyPrefix = buf.subdata(in: range.upperBound..<buf.count)
                self.handleHead(conn, headData: headData, bodyPrefix: bodyPrefix)
                return
            }
            if isComplete || buf.count >= 65536 {
                self.sendText(conn, status: "400 Bad Request", body: "bad request")
                return
            }
            self.readHead(conn, buffer: buf)
        }
    }

    private func handleHead(_ conn: NWConnection, headData: Data, bodyPrefix: Data) {
        guard let headText = String(data: headData, encoding: .utf8) else {
            sendText(conn, status: "400 Bad Request", body: "invalid headers")
            return
        }
        let lines = headText.components(separatedBy: "\r\n")
        guard let requestLine = lines.first else {
            sendText(conn, status: "400 Bad Request", body: "empty request")
            return
        }
        let parts = requestLine.split(separator: " ", omittingEmptySubsequences: false)
        guard parts.count >= 2 else {
            sendText(conn, status: "400 Bad Request", body: "malformed request line")
            return
        }
        let method = String(parts[0]).uppercased()
        let target = String(parts[1])
        var headers: [String: String] = [:]
        for line in lines.dropFirst() {
            if let idx = line.firstIndex(of: ":") {
                let k = line[..<idx].trimmingCharacters(in: .whitespaces).lowercased()
                let v = line[line.index(after: idx)...].trimmingCharacters(in: .whitespaces)
                headers[k] = v
            }
        }

        let splitTarget = target.split(separator: "?", maxSplits: 1, omittingEmptySubsequences: false)
        let route = String(splitTarget[0])
        let rawQuery = splitTarget.count > 1 ? String(splitTarget[1]) : ""

        let basePrefix = "/sc/\(nonce)"
        if route == basePrefix {
            let loc = rawQuery.isEmpty ? "\(basePrefix)/" : "\(basePrefix)/?\(rawQuery)"
            let head = "HTTP/1.1 302 Found\r\nLocation: \(loc)\r\nContent-Length: 0\r\nCache-Control: private, no-store\r\nConnection: close\r\n\(SolidCraftBundleSupport.securityHeaders)\r\n"
            sendAndClose(conn, Data(head.utf8))
            return
        }
        guard route.hasPrefix(basePrefix + "/") else {
            sendText(conn, status: "404 Not Found", body: "not found")
            return
        }
        let rel = String(route.dropFirst(basePrefix.count + 1))

        if rel == "files/get" {
            guard method == "GET" || method == "HEAD" else {
                sendText(conn, status: "405 Method Not Allowed", body: "GET or HEAD only")
                return
            }
            proxyFileGet(conn, method: method, rawQuery: rawQuery)
            return
        }
        if rel == "save" {
            guard method == "POST" || method == "PUT" else {
                sendText(conn, status: "405 Method Not Allowed", body: "POST or PUT only")
                return
            }
            if headers["transfer-encoding"] != nil {
                sendJSON(conn, status: "411 Length Required", body: Data("{\"ok\":false,\"error\":\"Content-Length required\"}".utf8))
                return
            }
            guard let clStr = headers["content-length"], let contentLength = Int(clStr), contentLength >= 0 else {
                sendJSON(conn, status: "411 Length Required", body: Data("{\"ok\":false,\"error\":\"Content-Length required\"}".utf8))
                return
            }
            let maxBytes = 64 << 20
            guard contentLength <= maxBytes else {
                NSLog("[SolidCraftWeb local] save rejected: %d bytes > %d", contentLength, maxBytes)
                sendJSON(conn, status: "413 Payload Too Large", body: Data("{\"ok\":false,\"error\":\"file too large\"}".utf8))
                return
            }
            readBody(conn, prefix: bodyPrefix, targetLength: contentLength) { [weak self] body in
                self?.proxySave(conn, method: method, rawQuery: rawQuery, body: body)
            }
            return
        }

        guard method == "GET" || method == "HEAD" else {
            sendText(conn, status: "405 Method Not Allowed", body: "GET or HEAD only")
            return
        }
        let headOnly = method == "HEAD"
        if rel.isEmpty || rel == "index.html" {
            let html = Data(SolidCraftBundleSupport.renderIndexHTML(manifest).utf8)
            sendData(conn, status: "200 OK", ctype: "text/html; charset=utf-8", extra: "Cache-Control: private, no-store\r\n", body: html, headOnly: headOnly)
            return
        }
        if rel == "bp-boot.js" {
            let js = Data(SolidCraftBundleSupport.renderBootJS(manifest).utf8)
            sendData(conn, status: "200 OK", ctype: "application/javascript; charset=utf-8", extra: "Cache-Control: private, no-store\r\n", body: js, headOnly: headOnly)
            return
        }

        serveBundledAsset(conn, rel: rel, acceptEncoding: headers["accept-encoding"] ?? "", headOnly: headOnly)
    }

    private func readBody(_ conn: NWConnection, prefix: Data, targetLength: Int, completion: @escaping (Data) -> Void) {
        if prefix.count >= targetLength {
            completion(prefix.prefix(targetLength))
            return
        }
        var acc = prefix
        func pump() {
            let remaining = targetLength - acc.count
            if remaining <= 0 {
                completion(acc)
                return
            }
            conn.receive(minimumIncompleteLength: 1, maximumLength: min(remaining, 262144)) { [weak self] data, _, isComplete, err in
                guard let self, !self.stopped else { conn.cancel(); return }
                if let data { acc.append(data) }
                if acc.count >= targetLength {
                    completion(acc.prefix(targetLength))
                    return
                }
                if err != nil || isComplete {
                    self.sendJSON(conn, status: "400 Bad Request", body: Data("{\"ok\":false,\"error\":\"incomplete body\"}".utf8))
                    return
                }
                pump()
            }
        }
        pump()
    }

    private func serveBundledAsset(_ conn: NWConnection, rel: String, acceptEncoding: String, headOnly: Bool) {
        guard SolidCraftBundleSupport.isSafeRelativePath(rel) else {
            sendText(conn, status: "404 Not Found", body: "not found")
            return
        }
        let plainURL = bundleDir.appendingPathComponent(rel).standardizedFileURL
        let rootPath = bundleDir.path.hasSuffix("/") ? bundleDir.path : bundleDir.path + "/"
        guard plainURL.path.hasPrefix(rootPath) else {
            sendText(conn, status: "404 Not Found", body: "not found")
            return
        }

        let gzURL = bundleDir.appendingPathComponent(rel + ".gz").standardizedFileURL
        let wantGz = SolidCraftBundleSupport.acceptsGzip(acceptEncoding)
        let fm = FileManager.default
        let useGz = wantGz && gzURL.path.hasPrefix(rootPath) && fm.fileExists(atPath: gzURL.path)
        let chosenURL = useGz ? gzURL : plainURL
        guard let attrs = try? fm.attributesOfItem(atPath: chosenURL.path),
              (attrs[.type] as? FileAttributeType) == .typeRegular,
              let fileSize = (attrs[.size] as? NSNumber)?.int64Value else {
            sendText(conn, status: "404 Not Found", body: "not found")
            return
        }

        let (ctype, immutable) = SolidCraftBundleSupport.mimeType(for: rel)
        let cache = immutable ? "Cache-Control: private, max-age=31536000, immutable\r\n" : "Cache-Control: private, no-cache\r\n"
        let rawSize: Int64? = if rel == manifest.wasm && manifest.wasmSize > 0 {
            Int64(manifest.wasmSize)
        } else if rel == manifest.js && manifest.jsSize > 0 {
            Int64(manifest.jsSize)
        } else if !useGz {
            fileSize
        } else {
            nil
        }

        var extra = cache
        if useGz {
            extra += "Content-Encoding: gzip\r\nVary: Accept-Encoding\r\n"
        }
        if let rawSize {
            extra += "X-Uncompressed-Length: \(rawSize)\r\n"
        }

        let head = "HTTP/1.1 200 OK\r\nContent-Type: \(ctype)\r\nContent-Length: \(fileSize)\r\nConnection: close\r\n\(SolidCraftBundleSupport.securityHeaders)\(extra)\r\n"
        if headOnly || fileSize == 0 {
            sendAndClose(conn, Data(head.utf8))
            return
        }
        guard let handle = try? FileHandle(forReadingFrom: chosenURL) else {
            sendText(conn, status: "404 Not Found", body: "not found")
            return
        }
        conn.send(content: Data(head.utf8), completion: .contentProcessed { [weak self] err in
            guard let self, !self.stopped, err == nil else {
                try? handle.close()
                self?.close(conn)
                return
            }
            self.streamFile(conn, handle: handle)
        })
    }

    private func streamFile(_ conn: NWConnection, handle: FileHandle) {
        let chunkSize = 64 * 1024
        let chunk = (try? handle.read(upToCount: chunkSize)) ?? Data()
        if chunk.isEmpty {
            try? handle.close()
            close(conn)
            return
        }
        conn.send(content: chunk, completion: .contentProcessed { [weak self] err in
            guard let self, !self.stopped, err == nil else {
                try? handle.close()
                self?.close(conn)
                return
            }
            self.streamFile(conn, handle: handle)
        })
    }

    private func proxyFileGet(_ conn: NWConnection, method: String, rawQuery: String) {
        guard let url = SolidCraftBundleSupport.buildHubURL(hubBase: hubBase, route: "/files/get", rawQuery: rawQuery) else {
            sendText(conn, status: "400 Bad Request", body: "bad hub URL")
            return
        }
        var req = URLRequest(url: url)
        req.httpMethod = method
        if !token.isEmpty { req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        session.dataTask(with: req) { [weak self] data, resp, err in
            guard let self, !self.stopped else { conn.cancel(); return }
            if let err {
                self.sendText(conn, status: "502 Bad Gateway", body: err.localizedDescription)
                return
            }
            let code = (resp as? HTTPURLResponse)?.statusCode ?? 502
            let ctype = (resp as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Type") ?? "application/octet-stream"
            let body = data ?? Data()
            self.sendData(conn, status: "\(code) \(HTTPURLResponse.localizedString(forStatusCode: code))", ctype: ctype, extra: "Cache-Control: private, no-store\r\n", body: body, headOnly: method == "HEAD")
        }.resume()
    }

    private func proxySave(_ conn: NWConnection, method: String, rawQuery: String, body: Data) {
        guard let url = SolidCraftBundleSupport.buildHubURL(hubBase: hubBase, route: "/solidcraft-web/save", rawQuery: rawQuery) else {
            sendJSON(conn, status: "400 Bad Request", body: Data("{\"ok\":false,\"error\":\"bad hub URL\"}".utf8))
            return
        }
        var req = URLRequest(url: url)
        req.httpMethod = method
        req.httpBody = body
        req.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
        if !token.isEmpty { req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        session.dataTask(with: req) { [weak self] data, resp, err in
            guard let self, !self.stopped else { conn.cancel(); return }
            if let err {
                let msg = err.localizedDescription.replacingOccurrences(of: "\"", with: "'")
                self.sendJSON(conn, status: "502 Bad Gateway", body: Data("{\"ok\":false,\"error\":\"\(msg)\"}".utf8))
                return
            }
            let code = (resp as? HTTPURLResponse)?.statusCode ?? 502
            let replyBody = data ?? Data()
            if code >= 400 {
                NSLog("[SolidCraftWeb local] save proxy HTTP %d (%d bytes)", code, body.count)
            }
            self.sendJSON(conn, status: "\(code) \(HTTPURLResponse.localizedString(forStatusCode: code))", body: replyBody)
        }.resume()
    }

    private func sendText(_ conn: NWConnection, status: String, body: String) {
        sendData(conn, status: status, ctype: "text/plain; charset=utf-8", extra: "Cache-Control: private, no-store\r\n", body: Data(body.utf8), headOnly: false)
    }

    private func sendJSON(_ conn: NWConnection, status: String, body: Data) {
        sendData(conn, status: status, ctype: "application/json", extra: "Cache-Control: private, no-store\r\n", body: body, headOnly: false)
    }

    private func sendData(_ conn: NWConnection, status: String, ctype: String, extra: String, body: Data, headOnly: Bool) {
        let head = "HTTP/1.1 \(status)\r\nContent-Type: \(ctype)\r\nContent-Length: \(body.count)\r\nConnection: close\r\n\(SolidCraftBundleSupport.securityHeaders)\(extra)\r\n"
        var out = Data(head.utf8)
        if !headOnly && !body.isEmpty {
            out.append(body)
        }
        sendAndClose(conn, out)
    }

    private func sendAndClose(_ conn: NWConnection, _ data: Data) {
        conn.send(content: data, completion: .contentProcessed { [weak self] _ in
            self?.close(conn)
        })
    }

    private func close(_ conn: NWConnection) {
        let key = ObjectIdentifier(conn)
        queue.async { [weak self] in
            self?.connections.removeValue(forKey: key)
            conn.cancel()
        }
    }
}
