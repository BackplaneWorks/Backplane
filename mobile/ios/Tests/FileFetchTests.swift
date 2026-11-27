import Foundation

// swiftc FileFetch.swift DataLinkGrammar.swift main.swift (scripts/mobile/ios-unit-tests.sh): which link is a file to download, its address, and how a stopped download goes on
var failed = 0
func check(_ ok: Bool, _ what: String) { if !ok { failed += 1; print("FAIL", what) } }
let tok = "0123456789abcdef0123456789abcdef0123456789abcdef"
let files = [DFileLink(path: "/home/user/p/mfg/gerbers.zip", name: "gerbers.zip", size: 123, open: "download"), DFileLink(path: "~/p/board.kicad_pcb", name: "board.kicad_pcb", size: 9, open: "view"), DFileLink(path: "/home/user/p/docs/My Doc.pdf", name: "My Doc.pdf", size: 456, open: "view")]
func kinds(_ t: String) -> [String] { dLinkRuns(t).compactMap { run in run.url.map { u in (dPathFrom(u).map { "path:" + $0 } ?? u.absoluteString) + "|" + run.text } } }

// shares
check(FileFetch.shareRoute("/s/\(tok)/gerbers.zip") == "/s/\(tok)/gerbers.zip", "share")
check(FileFetch.shareRoute("/s/\(tok)/POWER_TEST.md?inline=1#top") == "/s/\(tok)/POWER_TEST.md", "share strips query and fragment")
check(FileFetch.shareRoute("/s/\(tok)/my%20case.step") == "/s/\(tok)/my%20case.step", "share, encoded name")
check(FileFetch.shareRoute("/s/short/gerbers.zip") == nil, "short token")
check(FileFetch.shareRoute("/s/\(tok)/a/b.zip") == nil, "a folder in the name")
for bad in ["..\\..\\file?path=%2Fx", "..", ".", "a?b", "a#b", "a\\b", "%zz", "a b"] { check(FileFetch.shareRoute("/s/\(tok)/\(bad)") == nil, "not a share: \(bad)") }
check(FileFetch.isShare("/s/\(tok)/a.zip") && !FileFetch.isShare("/file"), "is share")
let base = URL(string: "http://h:3787/")!
check(FileFetch.url(base: base, route: "/s/\(tok)/my%20case.step")?.absoluteString == "http://h:3787/s/\(tok)/my%20case.step", "share url kept as written \(String(describing: FileFetch.url(base: base, route: "/s/\(tok)/my%20case.step")))")
check(FileFetch.url(base: base, route: "/s/\(tok)/..") == nil, "no share url for ..")
check(FileFetch.url(base: URL(string: "http://h:3787/?path=%2Fa%20b")!, route: "/file")?.absoluteString == "http://h:3787/file?path=%2Fa%20b", "file url")
check(kinds("get /s/\(tok)/my%20case.step.") == ["path:/s/\(tok)/my%20case.step|/s/\(tok)/my%20case.step"], "share in text \(kinds("get /s/\(tok)/my%20case.step."))")
check(kinds("(/s/\(tok)/README)") == ["path:/s/\(tok)/README|/s/\(tok)/README"], "share without an extension \(kinds("(/s/\(tok)/README)"))")
check(FileFetch.shareName("/s/\(tok)/my%20case.step") == "my case.step", "share name")
check(FileFetch.safeName("a/b") == "a_b" && FileFetch.safeName("..") == "download", "safe name")

// a tapped path finds its file
check(FileFetch.find("/home/user/p/mfg/gerbers.zip", in: files)?.name == "gerbers.zip", "find")
check(FileFetch.find("file:///home/user/p/mfg/gerbers.zip", in: files)?.name == "gerbers.zip", "find file://")
check(FileFetch.find("file:///home/user/p/docs/My%20Doc.pdf", in: files)?.name == "My Doc.pdf", "find file:// encoded")
check(FileFetch.find("file://localhost/home/user/p/docs/My%20Doc.pdf#page=2", in: files)?.name == "My Doc.pdf", "find file:// localhost fragment")
check(FileFetch.find("~/p/board.kicad_pcb:12", in: files)?.name == "board.kicad_pcb", "find with line")
check(FileFetch.find("~/p/board.kicad_pcb:3:4", in: files)?.name == "board.kicad_pcb", "find with line and column")
check(FileFetch.find("/home/user/p/mfg/gerbers.zip#L4-L9", in: files)?.name == "gerbers.zip", "find with anchor")
check(FileFetch.find("/home/user/p/mfg/other.zip", in: files) == nil && FileFetch.find("mfg/gerbers.zip", in: files) == nil, "no file")

// the address
check(FileFetch.fileQuery(path: "/a b.zip", thread: "t12") == [URLQueryItem(name: "path", value: "/a b.zip")], "own thread")
check(FileFetch.fileQuery(path: "~/x.zip", thread: "desk~t9") == [URLQueryItem(name: "path", value: "~/x.zip"), URLQueryItem(name: "thread", value: "desk~t9")], "far thread")

// an entry's files on the wire
let j = #"{"path":"/x/y.zip","name":"y.zip","size":104857600,"project":"pa","absPath":"/real/x/y.zip"}"#
let f = try! JSONDecoder().decode(DFileLink.self, from: Data(j.utf8))
check(f == DFileLink(path: "/x/y.zip", name: "y.zip", size: 104857600, open: "download", project: "pa", absPath: "/real/x/y.zip"), "decode \(f)")
check(FileFetch.find("/real/x/y.zip", in: [f])?.project == "pa", "find by absPath")

// resume planning
check(FileFetch.ask(have: 0, etag: "\"1-2\"").isEmpty, "nothing yet")
check(FileFetch.ask(have: 500, etag: "").isEmpty, "no tag: the start again")
check(FileFetch.ask(have: 500, etag: "\"64-abc\"") == ["Range": "bytes=500-", "If-Range": "\"64-abc\""], "the rest")
check(FileFetch.step(code: 206, have: 500, contentRange: "bytes 500-104857599/104857600", contentLength: 104857100) == .write(from: 500, total: 104857600), "206 appends")
check(FileFetch.step(code: 206, have: 500, contentRange: "bytes 0-99/100", contentLength: 100) == .again, "206 that does not join")
check(FileFetch.step(code: 206, have: 500, contentRange: nil, contentLength: 100) == .again, "206 without a range")
check(FileFetch.step(code: 200, have: 500, contentRange: nil, contentLength: 2048) == .write(from: 0, total: 2048), "200 restarts")
check(FileFetch.step(code: 416, have: 2048, contentRange: "bytes */2048", contentLength: 0) == .done, "416: already whole")
check(FileFetch.step(code: 416, have: 4096, contentRange: "bytes */2048", contentLength: 0) == .again, "416: no good")
check(FileFetch.step(code: 403, have: 0, contentRange: nil, contentLength: 23, body: "that file is not shared\n") == .fail("that file is not shared"), "403 words")
check(FileFetch.step(code: 502, have: 0, contentRange: nil, contentLength: 0) == .fail("HTTP 502"), "no words")
check(FileFetch.complete(have: 100, total: 100) && !FileFetch.complete(have: 99, total: 100) && !FileFetch.complete(have: 100, total: -1), "complete")
check(FileFetch.backoff(1) == 1 && FileFetch.backoff(2) == 2 && FileFetch.backoff(9) == 30, "backoff")

if failed == 0 { print("file fetch: ok") }
exit(failed == 0 ? 0 : 1)
