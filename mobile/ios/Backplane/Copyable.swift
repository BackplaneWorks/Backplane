import Foundation

// The copy button's pure parts (feature copyable; native/host/src/md/copy.rs, native/gpui/src/screens/copyable.rs and term.rs): what text gets a
// copy button and what gets Run in terminal too, the exact text a copy takes, and the bytes Run in terminal types at a shell's prompt.
// The hub decides where it says so (a post's or a notice's `copy: [{text, cmd}]`); the phone parses a reply's Markdown itself (DataMarkdown), so
// the same rules as copy.rs decide there, with the same cases in Tests/CopyableTests.swift.

/// One piece of a text to copy, as the hub lists it: `{text, cmd}` (cmd: Run in terminal too).
struct DCopyPiece: Codable, Equatable, Hashable {
    var text: String
    var cmd: Bool
    enum CodingKeys: String, CodingKey { case text, cmd }
    init(text: String, cmd: Bool) { self.text = text; self.cmd = cmd }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        text = (try? c.decode(String.self, forKey: .text)) ?? ""; cmd = (try? c.decode(Bool.self, forKey: .cmd)) ?? false
    }
}

enum Copyable {
    /// How long the check mark stays after a copy.
    static let shown: TimeInterval = 1.5

    /// The exact text a copy takes: no trailing newline, the rest as it is (a block whole).
    static func exact(_ text: String) -> String {
        var s = Substring(text); while let l = s.last, l == "\n" || l == "\r" || l == "\r\n" { s = s.dropLast() }; return String(s)
    }

    private static let shells: Set<String> = ["sh", "bash", "shell", "zsh", "fish", "ksh", "shellscript", "powershell", "pwsh", "ps1", "ps", "cmd", "bat", "batch", "nu", "nushell"]
    private static let programs: Set<String> = [
        "sudo", "doas", "apt", "apt-get", "apt-cache", "dpkg", "dnf", "yum", "rpm", "pacman", "yay", "zypper", "apk", "brew", "port", "snap", "flatpak", "nix", "nix-shell",
        "winget", "choco", "scoop", "pip", "pip3", "pipx", "uv", "uvx", "poetry", "conda", "mamba", "npm", "npx", "pnpm", "yarn", "bun", "bunx", "deno", "node", "python", "python3",
        "py", "ruby", "gem", "bundle", "go", "cargo", "rustup", "rustc", "make", "cmake", "ninja", "meson", "gcc", "clang", "git", "gh", "glab", "cd", "ls", "cat", "less",
        "head", "tail", "mkdir", "rmdir", "rm", "cp", "mv", "ln", "touch", "chmod", "chown", "chgrp", "find", "grep", "rg", "sed", "awk", "xargs", "tee", "echo", "printf",
        "export", "source", "env", "which", "whereis", "type", "curl", "wget", "ssh", "scp", "rsync", "sftp", "tar", "unzip", "zip", "gzip", "gunzip", "xz", "7z", "docker",
        "podman", "docker-compose", "kubectl", "helm", "systemctl", "journalctl", "service", "loginctl", "usermod", "useradd", "groupadd", "passwd", "kill", "killall", "ps", "top",
        "htop", "df", "du", "free", "lsblk", "lsusb", "lspci", "mount", "umount", "ip", "ping", "dig", "nslookup", "nc", "tailscale", "ufw", "iptables", "open", "xdg-open",
        "code", "vim", "nvim", "nano", "emacs", "tmux", "screen", "watch", "time", "nohup", "kicad-cli", "kicad", "freecad", "freecadcmd", "backplane", "backplane-hub",
        "openocd", "esptool", "esptool.py", "idf.py", "west", "platformio", "pio", "arduino-cli", "avrdude", "dfu-util", "picotool", "probe-rs", "minicom",
        "xcode-select", "xcodebuild", "defaults", "launchctl", "codesign", "spctl", "softwareupdate", "Set-ExecutionPolicy", "Get-ChildItem", "Invoke-WebRequest", "iwr", "irm",
        "adb", "fastboot", "gradle", "./gradlew", "mvn", "java", "javac", "dotnet", "terraform", "aws", "gcloud", "az", "fly", "vercel", "wrangler", "claude", "codex", "bash",
        "sh", "zsh", "fish", "pwsh", "powershell", "cmd", "reg", "setx", "net", "sc", "wsl", "ldconfig", "update-alternatives", "modprobe", "dmesg", "udevadm", "chsh",
    ]
    private static func ws(_ c: Character) -> Bool { c.isWhitespace }
    private static func trim(_ s: Substring) -> Substring {
        var t = s; while let f = t.first, ws(f) { t = t.dropFirst() }; while let l = t.last, ws(l) { t = t.dropLast() }; return t
    }
    private static func alnum(_ c: Character) -> Bool { c.isASCII && (c.isLetter || c.isNumber) }
    private static func lines(_ s: String) -> [Substring] {
        // Rust's str::lines: split on \n, a \r before it dropped, no last empty line
        // (a Swift Character holds "\r\n" whole: make it "\n" first)
        let s = s.replacingOccurrences(of: "\r\n", with: "\n")
        var out = s.split(separator: "\n", omittingEmptySubsequences: false)
        if s.hasSuffix("\n") { out.removeLast() }
        return out
    }

    /// A fenced block's text is a command to type (copy.rs block_is_command).
    static func blockIsCommand(_ text: String, lang: String) -> Bool {
        let l = String(trim(Substring(lang))).lowercased()
        if shells.contains(l) { return !trim(Substring(text)).isEmpty }
        if !(l.isEmpty || l == "text" || l == "txt" || l == "plain") { return false }
        var any = false, cont = false
        for line in lines(text) {
            let t = trim(line)
            if cont { cont = t.hasSuffix("\\"); continue }
            if t.isEmpty || t.hasPrefix("#") { continue }
            if !lineIsCommand(String(t)) { return false }
            any = true; cont = t.hasSuffix("\\")
        }
        return any
    }

    /// One line that starts the way a command does (copy.rs line_is_command).
    static func lineIsCommand(_ line: String) -> Bool {
        let tr = trim(Substring(line))
        let t = tr.hasPrefix("$ ") ? tr.dropFirst(2) : tr
        let words = t.split(whereSeparator: ws).drop(while: { assignment($0) })
        guard let first = words.first.map(String.init) else { return false }
        if programs.contains(first) { return true }
        let pathy = first.hasPrefix("./") || first.hasPrefix("../") || first.hasPrefix("~/") || (first.hasPrefix("/") && first.dropFirst().contains("/"))
            || (first.contains("/") && (first.hasSuffix(".sh") || first.hasSuffix(".py") || first.hasSuffix(".ps1")))
        return pathy && first.allSatisfy { alnum($0) || ".-_/~+".contains($0) }
    }
    private static func assignment(_ w: Substring) -> Bool {
        guard let i = w.firstIndex(of: "=") else { return false }
        let k = w[..<i]
        guard let f = k.first, f.isASCII, f.isLetter || f == "_" else { return false }
        return k.allSatisfy { alnum($0) || $0 == "_" }
    }

    /// What an inline code span is to copy: "cmd", "url", "path", "hash", "id", or nil (copy.rs inline_kind).
    static func inlineKind(_ text: String) -> String? {
        let t = String(trim(Substring(text)))
        if t.isEmpty || t.contains("\n") || t.count > 2000 { return nil }
        if t.split(whereSeparator: ws).count >= 2 && lineIsCommand(t) { return "cmd" }
        if t.contains(where: ws) { return nil }
        if t.hasPrefix("http://") || t.hasPrefix("https://") || t.hasPrefix("backplane://") { return "url" }
        let b = Array(t.utf8)
        if t.hasPrefix("/") || t.hasPrefix("~/") || t.hasPrefix("./") || t.hasPrefix("../") || (b.count > 2 && b[1] == UInt8(ascii: ":") && (b[2] == UInt8(ascii: "\\") || b[2] == UInt8(ascii: "/")))
            || (t.contains("/") && !t.contains("://") && t.contains { $0.isASCII && $0.isLetter }) { return "path" }
        let hex = t.allSatisfy { $0.isHexDigit && !$0.isUppercase && $0.isASCII }
        if hex && (7...40).contains(b.count) && t.contains(where: { $0.isNumber }) && t.contains(where: { $0.isLetter }) { return "hash" }
        if b.count >= 3 && t.contains(where: { $0.isASCII && $0.isNumber }) && t.allSatisfy({ alnum($0) || "-_.:@~|".contains($0) }) { return "id" }
        return nil
    }

    /// The pieces of a plain text to copy: each fenced block whole, each inline code span with a kind, in order, at most 16 (copy.rs spans).
    static func spans(_ text: String) -> [DCopyPiece] {
        var out: [DCopyPiece] = []
        var rest = Substring(text)
        while out.count < 16 {
            guard let i = rest.firstIndex(of: "`") else { break }
            let after = rest[i...]
            if after.hasPrefix("```") {
                let body = after.dropFirst(3)
                let (info, code): (Substring, Substring) = { if let n = body.firstIndex(of: "\n") { return (body[..<n], body[body.index(after: n)...]) }; return (body, "") }()
                guard let end = code.range(of: "```") else { break }
                var block = code[..<end.lowerBound]; while block.hasSuffix("\n") { block = block.dropLast() }
                let lang = info.split(omittingEmptySubsequences: false, whereSeparator: { $0.isWhitespace || $0 == "," || $0 == "{" }).first.map(String.init) ?? ""
                if !trim(block).isEmpty { out.append(DCopyPiece(text: String(block), cmd: blockIsCommand(String(block), lang: lang))) }
                rest = code[end.upperBound...]; continue
            }
            let inner = after.dropFirst()
            guard let end = inner.firstIndex(where: { $0 == "`" || $0 == "\n" }) else { break }
            if inner[end] == "`" {
                let t = String(inner[..<end])
                if let k = inlineKind(t) { out.append(DCopyPiece(text: t, cmd: k == "cmd")) }
                rest = inner[inner.index(after: end)...]
            } else { rest = inner[end...] }
        }
        return out
    }

    /// The copy pieces of an entry: the hub's when it sent them, else the same rules here.
    static func pieces(hub: [DCopyPiece]?, text: String) -> [DCopyPiece] { hub ?? spans(text) }

    /// The hub's list for an entry or post: with feature `copy` the hub sends it whenever there is something (no key: nothing), and its list wins;
    /// an older hub sends none (nil: the rules here decide).
    static func list(_ copy: [DCopyPiece]?, hubHasCopy: Bool) -> [DCopyPiece]? { hubHasCopy ? (copy ?? []) : copy }

    /// A fenced block of a reply: always copyable; Run in terminal when the hub lists its text with cmd, else (no list) by the rules, lang included.
    static func blockCmd(_ text: String, lang: String, hub: [DCopyPiece]?) -> Bool {
        if let h = hub { return h.contains { exact($0.text) == exact(text) && $0.cmd } }
        return blockIsCommand(text, lang: lang)
    }
    /// The inline code spans of a Markdown line (a paragraph, a list item) that get a copy button, in order.
    static func inlinePieces(_ line: String, hub: [DCopyPiece]?) -> [DCopyPiece] {
        let found = spans(line).filter { !$0.text.contains("\n") }
        guard let h = hub else { return found }
        // the hub's list decides: a span it names (with its cmd), nothing else
        return found.compactMap { f in h.first(where: { $0.text == f.text }) }
    }
}

enum ChatTerm {
    /// The chat whose shell the terminal is (term.rs term_key_of): a thread (a project's, a bot's chat, the uberbot's) by its id, a room by
    /// `room:<id>`. A room of a linked machine has no shell here: "".
    static func key(thread: String) -> String { thread }
    static func key(room: String) -> String { room.isEmpty || room.contains("~") ? "" : "room:" + room }
}
