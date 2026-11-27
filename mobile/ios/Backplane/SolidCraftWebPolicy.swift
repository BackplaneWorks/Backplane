import Foundation

struct SolidCraftBundleManifest: Decodable, Equatable {
    var version: String
    var js: String
    var wasm: String
    var css: String
    var wasmSize: Int
    var jsSize: Int
}

enum SolidCraftWebPolicy {
    /// Deterministic store ID for the SolidCraft Web persistent WKWebsiteDataStore
    /// (persists OPFS and the .wasm HTTP cache without touching `.default()`).
    static let dataStoreID = UUID(uuidString: "7B6E3A54-4C92-4E2A-9D8F-2E1A0F6B3C12")!

    static func normalizedPort(_ url: URL) -> Int? {
        if let port = url.port { return port }
        switch url.scheme?.lowercased() {
        case "https": return 443
        case "http": return 80
        default: return nil
        }
    }

    static func isSameOrigin(_ url: URL, _ base: URL) -> Bool {
        guard let s1 = url.scheme?.lowercased(),
              let s2 = base.scheme?.lowercased(),
              let h1 = url.host?.lowercased(),
              let h2 = base.host?.lowercased(),
              !s1.isEmpty, !h1.isEmpty else { return false }
        return s1 == s2 && h1 == h2 && normalizedPort(url) == normalizedPort(base)
    }

    static func initialRequest(url: URL, token: String) -> URLRequest {
        var req = URLRequest(url: url)
        if !token.isEmpty {
            req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        return req
    }

    static func cookieProperties(url: URL, token: String, clear: Bool = false) -> [HTTPCookiePropertyKey: Any] {
        var props: [HTTPCookiePropertyKey: Any] = [
            .name: "bp_token",
            .value: clear ? "" : token,
            .domain: url.host ?? "",
            .path: "/",
            HTTPCookiePropertyKey("HttpOnly"): "TRUE",
        ]
        if url.scheme?.lowercased() == "https" {
            props[.secure] = "TRUE"
        }
        if clear {
            props[.expires] = Date(timeIntervalSince1970: 0)
        }
        return props
    }
}

enum SolidCraftBundleSupport {
    static let securityHeaders =
        "Content-Security-Policy: default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; " +
        "style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; font-src 'self' data:; " +
        "connect-src 'self' blob: data:; worker-src 'self' blob:; child-src 'self' blob:; " +
        "object-src 'none'; frame-ancestors 'self'; base-uri 'self'; form-action 'self'\r\n" +
        "X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n" +
        "Cross-Origin-Resource-Policy: same-origin\r\n"

    static func parseManifest(_ data: Data) -> SolidCraftBundleManifest? {
        guard let m = try? JSONDecoder().decode(SolidCraftBundleManifest.self, from: data),
              !m.js.isEmpty, !m.wasm.isEmpty,
              isSafeRelativePath(m.js), isSafeRelativePath(m.wasm),
              (m.css.isEmpty || isSafeRelativePath(m.css)) else {
            return nil
        }
        return m
    }

    static func isSafeRelativePath(_ rel: String) -> Bool {
        guard !rel.isEmpty,
              !rel.hasPrefix("/"),
              !rel.contains("\\"),
              !rel.contains("\0") else { return false }
        let parts = rel.split(separator: "/", omittingEmptySubsequences: false)
        if parts.contains(where: { $0.isEmpty || $0 == "." || $0 == ".." || $0.hasPrefix(".") }) {
            return false
        }
        if rel == "sw.js" || rel == "manifest.json" || rel == "index.html" || rel == "bp-boot.js" {
            return false
        }
        return true
    }

    static func acceptsGzip(_ header: String) -> Bool {
        header.split(separator: ",").contains { item in
            let parts = item.split(separator: ";").map { $0.trimmingCharacters(in: .whitespaces) }
            guard let enc = parts.first, enc.caseInsensitiveCompare("gzip") == .orderedSame else { return false }
            for p in parts.dropFirst() {
                if let qStr = p.lowercased().stripPrefixCompat("q="),
                   let q = Double(qStr.trimmingCharacters(in: .whitespaces)),
                   q <= 0 {
                    return false
                }
            }
            return true
        }
    }

    static func mimeType(for rel: String) -> (String, Bool) {
        let lower = rel.lowercased()
        if lower.hasSuffix(".wasm") { return ("application/wasm", true) }
        if lower.hasSuffix(".js") || lower.hasSuffix(".mjs") { return ("application/javascript; charset=utf-8", true) }
        if lower.hasSuffix(".css") { return ("text/css; charset=utf-8", true) }
        if lower.hasSuffix(".html") { return ("text/html; charset=utf-8", false) }
        if lower.hasSuffix(".svg") { return ("image/svg+xml", true) }
        if lower.hasSuffix(".png") { return ("image/png", true) }
        if lower.hasSuffix(".ttf") { return ("font/ttf", true) }
        if lower.hasSuffix(".otf") { return ("font/otf", true) }
        if lower.hasSuffix(".woff2") { return ("font/woff2", true) }
        if lower.hasSuffix(".json") { return ("application/json; charset=utf-8", false) }
        if lower.hasSuffix(".txt") || lower.hasSuffix(".md") { return ("text/plain; charset=utf-8", false) }
        return ("application/octet-stream", false)
    }

    static func progressPercent(done: Int64, total: Int64) -> Int? {
        guard total > 0, done >= 0 else { return nil }
        return min(100, max(0, Int((done * 100) / total)))
    }

    static func buildHubURL(hubBase: URL, route: String, rawQuery: String) -> URL? {
        guard var comps = URLComponents(url: hubBase, resolvingAgainstBaseURL: false) else { return nil }
        comps.path = route
        comps.percentEncodedQuery = rawQuery.isEmpty ? nil : rawQuery
        return comps.url
    }

    private static func htmlEscape(_ s: String) -> String {
        var out = ""
        out.reserveCapacity(s.count)
        for ch in s {
            switch ch {
            case "&": out += "&amp;"
            case "<": out += "&lt;"
            case ">": out += "&gt;"
            case "\"": out += "&quot;"
            case "'": out += "&#39;"
            default: out.append(ch)
            }
        }
        return out
    }

    static func renderIndexHTML(_ m: SolidCraftBundleManifest) -> String {
        let cssLink = m.css.isEmpty ? "" : "  <link rel=\"stylesheet\" href=\"./\(htmlEscape(m.css))\">\n"
        let jsEsc = htmlEscape(m.js)
        return """
        <!DOCTYPE html>
        <html lang="en">
        <head>
          <meta charset="utf-8">
          <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
          <meta name="referrer" content="no-referrer">
          <title>SolidCraft Web (Bundled)</title>
        \(cssLink)  <style>
            html, body { margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: #262930; }
            #solidcraft_canvas { width: 100%; height: 100%; display: block; touch-action: none; }
          </style>
          <link rel="modulepreload" href="./\(jsEsc)">
        </head>
        <body>
          <canvas id="solidcraft_canvas"></canvas>
          <div id="solidcraft_loading" role="status"><img src="./loading-logo.svg" alt=""><span class="sr-only">Loading SolidCraft&hellip;</span></div>
          <script type="module" src="./bp-boot.js"></script>
        </body>
        </html>
        """
    }

    static func renderBootJS(_ m: SolidCraftBundleManifest) -> String {
        let jsJSON = (try? String(data: JSONEncoder().encode("./\(m.js)"), encoding: .utf8)) ?? "\"./solidcraft-web.js\""
        let wasmJSON = (try? String(data: JSONEncoder().encode("./\(m.wasm)"), encoding: .utf8)) ?? "\"./solidcraft-web_bg.wasm\""
        return """
        window.__bpSolidCraftBootStarted = true;
        const JS_ENTRY = \(jsJSON);
        const WASM_ENTRY = \(wasmJSON);
        const DESIGN_EXTS = new Set([".sc", ".solidcraft"]);
        const CAD_EXTS = new Set([".sc", ".solidcraft", ".step", ".stp", ".stl", ".3mf", ".iges", ".igs", ".obj", ".glb"]);

        const params = new URLSearchParams(window.location.search);
        let currentThread = params.get("thread") || "";
        let currentPath = params.get("path") || "";
        const initialTheme = params.get("theme") || "";
        const readOnly = params.get("readonly") === "1";
        const INSTANCE_ID = Math.random().toString(36).slice(2, 10);
        const STALE_BRIDGE_MS = 60 * 60 * 1000;

        function bridgeTs(name, prefix) {
          if (!name.startsWith(prefix)) return 0;
          const rest = name.slice(prefix.length);
          const ts = parseInt(rest.split("-")[0], 10);
          return Number.isFinite(ts) ? ts : 0;
        }

        function notifyHost(msg) {
          try { window.webkit?.messageHandlers?.backplane?.postMessage(msg); } catch {}
        }

        function reportStage(stage, detail = "", done = 0, total = 0) {
          notifyHost({ type: "bp:solidcraft:stage", stage, detail, done, total });
        }

        function seedPrefs(theme) {
          try {
            const raw = window.localStorage.getItem("solidcraft.prefs");
            let parsed = {};
            if (raw) {
              try { parsed = JSON.parse(raw) || {}; } catch { parsed = {}; }
            }
            const prev = (parsed && typeof parsed.preferences === "object" && parsed.preferences) || {};
            parsed.preferences = {
              ...prev,
              ask_import_options: false,
              show_discord_button: false,
              show_donate_button: false,
              check_updates: false,
              usage_statistics: false,
              show_home_on_startup: false,
              recover_after_browser_leave: false,
              keep_unsaved_in_browser: false,
              ...(theme === "light" || theme === "dark" ? { viewport_theme: theme } : {}),
            };
            window.localStorage.setItem("solidcraft.prefs", JSON.stringify(parsed));
            window.localStorage.setItem("usage_statistics", JSON.stringify({ install_id: "", notice_ok: true, days: {}, forget: [] }));
          } catch {}
        }

        function extOf(p) {
          const i = p.lastIndexOf(".");
          return i >= 0 ? p.slice(i).toLowerCase() : "";
        }

        function baseOf(p) {
          const s = p.replace(/\\\\/g, "/");
          const i = s.lastIndexOf("/");
          return i >= 0 ? s.slice(i + 1) : s;
        }

        function safeOpfsName(prefix, path) {
          const base = baseOf(path) || "part.sc";
          const ext = extOf(base);
          const stem = (ext ? base.slice(0, -ext.length) : base).replace(/[^a-zA-Z0-9._-]/g, "_").slice(0, 48) || "part";
          return `${prefix}${stem}${ext || ".sc"}`;
        }

        function encodeLocalBytes(bytes) {
          try {
            const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
            return "t" + text;
          } catch {
            const hex = new Array(bytes.length);
            for (let i = 0; i < bytes.length; i++) {
              hex[i] = bytes[i].toString(16).padStart(2, "0");
            }
            return "x" + hex.join("");
          }
        }

        function decodeLocalBytes(raw) {
          if (typeof raw !== "string" || !raw) return null;
          if (raw[0] === "t") return new TextEncoder().encode(raw.slice(1));
          if (raw[0] === "x") {
            const hex = raw.slice(1);
            const out = new Uint8Array(Math.floor(hex.length / 2));
            for (let i = 0; i < out.length; i++) {
              out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16) || 0;
            }
            return out;
          }
          return null;
        }

        async function opfsRoot() {
          if (!navigator.storage || typeof navigator.storage.getDirectory !== "function") return null;
          try { return await navigator.storage.getDirectory(); } catch { return null; }
        }

        async function clearBridgeFiles() {
          const cutoff = Date.now() - STALE_BRIDGE_MS;
          const isStaleBridge = (n) => {
            const t = bridgeTs(n, "bp-active-") || bridgeTs(n, "bp-save-");
            return t > 0 && t < cutoff;
          };
          const dir = await opfsRoot();
          if (dir) {
            try {
              for await (const [name] of dir.entries()) {
                if (isStaleBridge(name)) await dir.removeEntry(name).catch(() => {});
              }
            } catch {}
          }
          try {
            const toRemove = [];
            for (let i = 0; i < window.localStorage.length; i++) {
              const k = window.localStorage.key(i);
              if (!k || !k.startsWith("solidcraft.file.")) continue;
              const name = k.slice("solidcraft.file.".length);
              if (isStaleBridge(name)) toRemove.push(k);
            }
            for (const k of toRemove) window.localStorage.removeItem(k);
          } catch {}
        }

        async function writeOpfsViaWorker(name, bytes) {
          const workerSrc = `self.onmessage = async (ev) => {
            try {
              const root = await navigator.storage.getDirectory();
              const fh = await root.getFileHandle(ev.data.name, { create: true });
              if (typeof fh.createSyncAccessHandle !== "function") {
                throw new Error("no createSyncAccessHandle");
              }
              const access = await fh.createSyncAccessHandle();
              try {
                access.truncate(0);
                access.write(ev.data.bytes, { at: 0 });
                access.flush();
              } finally {
                access.close();
              }
              self.postMessage({ ok: true });
            } catch (err) {
              self.postMessage({ ok: false, error: err instanceof Error ? err.message : String(err) });
            }
          };`;
          const blobUrl = URL.createObjectURL(new Blob([workerSrc], { type: "application/javascript" }));
          try {
            await new Promise((resolve, reject) => {
              const w = new Worker(blobUrl);
              const copy = new Uint8Array(bytes);
              w.onmessage = (ev) => {
                w.terminate();
                if (ev.data && ev.data.ok) resolve();
                else reject(new Error((ev.data && ev.data.error) || "OPFS worker write failed"));
              };
              w.onerror = (ev) => {
                w.terminate();
                reject(new Error(ev.message || "OPFS worker error"));
              };
              w.postMessage({ name, bytes: copy }, [copy.buffer]);
            });
          } finally {
            URL.revokeObjectURL(blobUrl);
          }
        }

        async function writeBridgeFile(name, bytes) {
          const dir = await opfsRoot();
          if (dir) {
            try {
              const fh = await dir.getFileHandle(name, { create: true });
              if (typeof fh.createWritable === "function") {
                const w = await fh.createWritable();
                await w.write(bytes);
                await w.close();
                return;
              }
              if (typeof Worker === "function") {
                await writeOpfsViaWorker(name, bytes);
                return;
              }
            } catch {
              await dir.removeEntry(name).catch(() => {});
            }
          }
          window.localStorage.setItem("solidcraft.file." + name, encodeLocalBytes(bytes));
        }

        async function readBridgeEntry(name) {
          const dir = await opfsRoot();
          if (dir) {
            try {
              const fh = await dir.getFileHandle(name, { create: false });
              const file = await fh.getFile();
              const buf = await file.arrayBuffer();
              return { bytes: new Uint8Array(buf), stamp: file.lastModified || 0 };
            } catch {}
          }
          const raw = decodeLocalBytes(window.localStorage.getItem("solidcraft.file." + name));
          return raw ? { bytes: raw, stamp: 0 } : null;
        }

        async function removeBridgeFile(name) {
          const dir = await opfsRoot();
          if (dir) await dir.removeEntry(name).catch(() => {});
          try { window.localStorage.removeItem("solidcraft.file." + name); } catch {}
        }

        async function fetchProjectFile(thread, path) {
          const url = `./files/get?thread=${encodeURIComponent(thread)}&path=${encodeURIComponent(path)}`;
          const resp = await fetch(url, { credentials: "same-origin" });
          if (!resp.ok) {
            throw new Error(`Could not load ${path} (HTTP ${resp.status})`);
          }
          return new Uint8Array(await resp.arrayBuffer());
        }

        async function waitForApi(timeoutMs = 30000) {
          const deadline = performance.now() + timeoutMs;
          while (performance.now() < deadline) {
            if (window.solidcraft && typeof window.solidcraft.execute === "function" && typeof window.solidcraft.request === "function") {
              return window.solidcraft;
            }
            await new Promise((r) => setTimeout(r, 50));
          }
          throw new Error("SolidCraft Web API timed out");
        }

        async function openStagedOpfs(api, opfsName, timeoutMs = 15000) {
          const deadline = performance.now() + timeoutMs;
          let lastErr = null;
          while (performance.now() < deadline) {
            try {
              await api.execute("doc.open", { path: `/opfs/${opfsName}` });
              return;
            } catch (err) {
              lastErr = err;
              await new Promise((r) => setTimeout(r, 100));
            }
          }
          throw lastErr || new Error("Timed out opening staged CAD file");
        }

        async function applyEmbeddedPrefs(api, theme) {
          const prefs = {
            ask_import_options: false,
            show_discord_button: false,
            show_donate_button: false,
            check_updates: false,
            usage_statistics: false,
            show_home_on_startup: false,
          };
          if (theme === "light" || theme === "dark") prefs.viewport_theme = theme;
          await api.request("ui.prefs", prefs).catch(() => {});
        }

        async function waitForExportedBytes(saveName, timeoutMs = 10000) {
          const deadline = performance.now() + timeoutMs;
          let lastLen = -1;
          let lastStamp = -1;
          while (performance.now() < deadline) {
            const entry = await readBridgeEntry(saveName);
            if (entry && entry.bytes.byteLength > 0) {
              if (entry.bytes.byteLength === lastLen && entry.stamp === lastStamp) {
                return entry.bytes;
              }
              lastLen = entry.bytes.byteLength;
              lastStamp = entry.stamp;
              await new Promise((r) => setTimeout(r, 150));
              continue;
            }
            await new Promise((r) => setTimeout(r, 80));
          }
          const finalEntry = await readBridgeEntry(saveName);
          return finalEntry && finalEntry.bytes.byteLength > 0 && finalEntry.bytes.byteLength === lastLen ? finalEntry.bytes : null;
        }

        let saving = false;

        async function saveCurrent() {
          if (saving) return;
          if (readOnly) {
            notifyHost({ type: "bp:solidcraft:error", op: "save", error: "Read-only copy: edits cannot be saved." });
            return;
          }
          if (!currentThread || !currentPath) {
            notifyHost({ type: "bp:solidcraft:error", op: "save", error: "No project CAD file is open." });
            return;
          }
          const ext = extOf(currentPath);
          if (!CAD_EXTS.has(ext)) {
            notifyHost({ type: "bp:solidcraft:error", op: "save", error: `Unsupported CAD extension: ${ext}` });
            return;
          }
          saving = true;
          notifyHost({ type: "bp:solidcraft:saving", thread: currentThread, path: currentPath });
          const saveName = safeOpfsName(`bp-save-${Date.now()}-${INSTANCE_ID}-`, currentPath);
          try {
            const api = await waitForApi(5000);
            let rev = null;
            try {
              const r = await api.request("document.revision", {});
              if (r && typeof r.revision === "number") rev = r.revision;
            } catch {}
            if (DESIGN_EXTS.has(ext)) {
              await api.execute("file.save_as", { path: `/opfs/${saveName}`, overwrite: true });
            } else {
              await api.execute("file.export", { path: `/opfs/${saveName}` });
            }
            let bytes = await waitForExportedBytes(saveName, 10000);
            if (!bytes && DESIGN_EXTS.has(ext)) {
              const exp = await api.request("document.export", {});
              if (exp && typeof exp.text === "string") {
                bytes = new TextEncoder().encode(exp.text);
                if (typeof exp.revision === "number") rev = exp.revision;
              }
            }
            if (!bytes || bytes.byteLength === 0) {
              throw new Error("SolidCraft produced an empty file");
            }
            const url = `./save?thread=${encodeURIComponent(currentThread)}&path=${encodeURIComponent(currentPath)}`;
            const resp = await fetch(url, {
              method: "POST",
              credentials: "same-origin",
              headers: { "Content-Type": "application/octet-stream" },
              body: bytes,
            });
            let payload = null;
            try { payload = await resp.json(); } catch {}
            if (!resp.ok || (payload && payload.ok === false)) {
              throw new Error(payload?.error || `Save failed (HTTP ${resp.status})`);
            }
            if (DESIGN_EXTS.has(ext) && typeof rev === "number") {
              await api.request("document.saved", { revision: rev }).catch(() => {});
            }
            notifyHost({ type: "bp:solidcraft:saved", thread: currentThread, path: currentPath, size: bytes.byteLength });
          } catch (err) {
            notifyHost({ type: "bp:solidcraft:error", op: "save", error: err instanceof Error ? err.message : String(err) });
          } finally {
            saving = false;
            await removeBridgeFile(saveName);
          }
        }

        window.__bpSolidCraft = { save: saveCurrent };

        async function fetchWasmWithProgress() {
          const resp = await fetch(WASM_ENTRY, { credentials: "same-origin" });
          if (!resp.ok) {
            throw new Error(`Could not load SolidCraft WASM (HTTP ${resp.status})`);
          }
          const totalHeader = Number(resp.headers.get("X-Uncompressed-Length")) || Number(resp.headers.get("Content-Length")) || 0;
          if (!resp.body || typeof ReadableStream !== "function" || typeof Response !== "function") {
            return resp;
          }
          const reader = resp.body.getReader();
          let done = 0;
          let lastReport = 0;
          const stream = new ReadableStream({
            async pull(controller) {
              const { value, done: eof } = await reader.read();
              if (eof) {
                reportStage("wasm", "", done, totalHeader || done);
                controller.close();
                return;
              }
              done += value.byteLength;
              const now = performance.now();
              if (now - lastReport >= 150) {
                lastReport = now;
                reportStage("wasm", "", done, totalHeader);
              }
              controller.enqueue(value);
            },
            cancel(reason) {
              return reader.cancel(reason);
            }
          });
          return new Response(stream, { status: resp.status, headers: { "Content-Type": "application/wasm" } });
        }

        function sliceView(view) {
          return new Uint8Array(view.buffer, view.byteOffset, view.byteLength).slice();
        }

        function normalizeWriteChunk(chunk) {
          if (ArrayBuffer.isView(chunk)) {
            if (chunk.byteOffset !== 0 || chunk.byteLength !== chunk.buffer.byteLength) {
              return sliceView(chunk);
            }
            return chunk;
          }
          if (chunk && typeof chunk === "object" && ArrayBuffer.isView(chunk.data)) {
            const d = chunk.data;
            if (d.byteOffset !== 0 || d.byteLength !== d.buffer.byteLength) {
              return Object.assign({}, chunk, { data: sliceView(d) });
            }
          }
          return chunk;
        }

        // iOS WebKit's FileSystemWritableFileStream.write ignores ArrayBufferView.byteOffset/byteLength
        // and writes the entire backing 100+ MB WASM heap ArrayBuffer unless sliced to a tight buffer.
        function patchWritableProto(proto) {
          if (!proto || typeof proto.write !== "function" || proto.__bpSlicePatched) return;
          const origWrite = proto.write;
          proto.write = function (data) {
            return origWrite.call(this, normalizeWriteChunk(data));
          };
          proto.__bpSlicePatched = true;
        }

        if (typeof FileSystemWritableFileStream === "function") {
          patchWritableProto(FileSystemWritableFileStream.prototype);
        }
        if (typeof FileSystemFileHandle === "function" && typeof FileSystemFileHandle.prototype?.createWritable === "function") {
          const origCreateWritable = FileSystemFileHandle.prototype.createWritable;
          FileSystemFileHandle.prototype.createWritable = async function (...args) {
            const writable = await origCreateWritable.apply(this, args);
            try { patchWritableProto(Object.getPrototypeOf(writable)); } catch {}
            return writable;
          };
        }

        (async () => {
          const t0 = performance.now();
          try {
            const caps = {
              isSecureContext: Boolean(window.isSecureContext),
              hasWebGPU: Boolean(navigator.gpu),
              hasOPFS: Boolean(navigator.storage && typeof navigator.storage.getDirectory === "function"),
              hasWorker: typeof Worker === "function",
            };
            notifyHost({ type: "bp:solidcraft:diag", level: "caps", message: JSON.stringify(caps) });
            seedPrefs(initialTheme);
            await clearBridgeFiles();

            let stagedName = "";
            let preloadError = "";
            if (currentThread && currentPath) {
              reportStage("fetch", baseOf(currentPath));
              try {
                const bytes = await fetchProjectFile(currentThread, currentPath);
                stagedName = safeOpfsName(`bp-active-${Date.now()}-${INSTANCE_ID}-`, currentPath);
                await writeBridgeFile(stagedName, bytes);
              } catch (err) {
                preloadError = err instanceof Error ? err.message : String(err);
              }
            }

            reportStage("engine");
            const mod = await import(JS_ENTRY);
            const wasmResp = await fetchWasmWithProgress();
            reportStage("init");
            const wasm = await mod.default({ module_or_path: wasmResp });
            window.wasmBindings = mod;
            window.dispatchEvent(new CustomEvent("TrunkApplicationStarted", { detail: { wasm } }));
            const api = await waitForApi(30000);
            await applyEmbeddedPrefs(api, initialTheme);
            if (stagedName) {
              reportStage("open", baseOf(currentPath));
              await openStagedOpfs(api, stagedName, 15000);
              await api.request("ui.view", { view: "fit", animate: false }).catch(() => {});
              await removeBridgeFile(stagedName);
            } else if (preloadError) {
              notifyHost({ type: "bp:solidcraft:error", op: "open", error: preloadError });
            }
            notifyHost({
              type: "bp:solidcraft:ready",
              thread: currentThread,
              path: currentPath,
              loadMs: Math.round(performance.now() - t0),
            });
          } catch (err) {
            notifyHost({
              type: "bp:solidcraft:error",
              op: "boot",
              error: err instanceof Error ? err.message : String(err),
            });
          }
        })();
        """
    }
}

private extension String {
    func stripPrefixCompat(_ prefix: String) -> String? {
        hasPrefix(prefix) ? String(dropFirst(prefix.count)) : nil
    }
}
