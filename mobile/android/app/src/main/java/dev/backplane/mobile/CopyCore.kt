package dev.backplane.mobile

import org.json.JSONArray

// Copyable's pure part (CopyCoreTest): what of a text is to copy and what gets Run in terminal, and the bytes Run in terminal types.
// The hub decides (native/host/src/md/copy.rs, feature `copy`: an entry's or a post's `copy: [{text, cmd}]`); a hub without the feature gets the same
// rules from CopyRules, a port of copy.rs with its tests, so an older hub's messages still have their buttons.

/** One piece to copy: the exact text, and whether it is a shell command (Run in terminal too). */
data class CopyPiece(val text: String, val cmd: Boolean)

/** `copy: [{text, cmd}]` as the hub sends it; null when the key is absent (the hub said nothing). */
fun parseCopy(a: JSONArray?): List<CopyPiece>? = a?.let { (0 until it.length()).mapNotNull { i -> it.optJSONObject(i) }.map { o -> CopyPiece(o.optString("text"), o.optBoolean("cmd")) } }
fun copyJson(c: List<CopyPiece>): JSONArray = JSONArray(c.map { org.json.JSONObject().put("text", it.text).put("cmd", it.cmd) })

/** What gets a copy button in a message drawn here: every fenced block (cmd: Run in terminal too), and the inline code spans the list names. */
class CopyLookup(private val hub: List<CopyPiece>?) {
    private val byText = hub?.associateBy { it.text }
    /** an inline code span: its piece, or null for no button */
    fun inline(text: String): CopyPiece? =
        if (byText != null) (byText[text] ?: byText[text.trim()]) else CopyRules.inlineKind(text)?.let { CopyPiece(text.trim(), it == "cmd") }
    /** a fenced block (always copyable): whether it is a command */
    fun blockCmd(text: String, lang: String): Boolean =
        if (byText != null) byText[Copy.exact(text)]?.cmd == true else CopyRules.blockIsCommand(Copy.exact(text), lang)
    /** the pieces of a plain text (a notice, a room's post) */
    fun pieces(text: String): List<CopyPiece> = hub ?: CopyRules.spans(text)
    val key: String get() = byText?.keys?.joinToString("\u0001")?.let { "h" + it.hashCode() } ?: "p"

    companion object {
        /** The hub's answer wins when it has the feature: an entry without `copy` then has nothing inline to copy (a reply still arriving: the rules). */
        fun of(copy: List<CopyPiece>?, hubHasCopy: Boolean, live: Boolean = false): CopyLookup =
            CopyLookup(copy ?: if (hubHasCopy && !live) emptyList() else null)
    }
}

object Copy {
    /** How long the check mark stays after a copy. */
    const val SHOWN_MS = 1500L
    /** The text a copy puts on the clipboard: as it is, with no line break after it. */
    fun exact(text: String): String = text.trimEnd('\n', '\r')
    /** The first line of a piece as a row shows it, with … when there is more. */
    fun shown(text: String): String { val t = exact(text); val first = t.substringBefore('\n'); return if (t.contains('\n')) "$first …" else first }
}

/** Run in terminal (the desktop's screens/term.rs `prefill_bytes`, `prefill_ready`, `term_key_of`). */
object RunInTerminal {
    /** How long it waits for a shell that never turns on bracketed paste before it types the command anyway. */
    const val WAIT_MS = 900L

    /** The bytes that type `cmd` at a shell's prompt without running it. A shell with bracketed paste (?2004) gets one paste, its lines kept and none run;
     *  otherwise it must be one line: a `\` continuation is joined and lines become `a && b`. Never an Enter. */
    fun prefillBytes(cmd: String, bpaste: Boolean, paste: (String) -> ByteArray): ByteArray {
        val c = Copy.exact(cmd)
        if (bpaste) return paste(c)
        val joined = c.replace("\\\r\n", " ").replace("\\\n", " ")
        val one = joined.lines().map { it.trim() }.filter { it.isNotEmpty() && !it.startsWith("#") }.joinToString(" && ")
        return one.toByteArray(Charsets.UTF_8).filter { it != 0x1b.toByte() && it != '\r'.code.toByte() && it != '\n'.code.toByte() }.toByteArray()
    }
    fun prefillBytes(cmd: String, vt: Vt): ByteArray = prefillBytes(cmd, vt.bpaste()) { vt.paste(it) }

    /** The shell is at its prompt: it turned on bracketed paste, or it printed something and WAIT_MS went by (4× that whatever it printed).
     *  null: a full-screen program has the screen; nothing is typed into it. */
    fun ready(alt: Boolean, bpaste: Boolean, printed: Boolean, waitedMs: Long): Boolean? {
        if (alt) return null
        return bpaste || (printed && waitedMs >= WAIT_MS) || waitedMs >= WAIT_MS * 4
    }
    fun ready(vt: Vt, waitedMs: Long): Boolean? = ready(vt.alt(), vt.bpaste(), vt.cursor().x > 0 || vt.cursor().y > 0 || vt.history() > 0, waitedMs)

    /** Which chat's shell a page's terminal is (`term.new {chat}`): a thread's (a bot's chat and the uberbot's are threads here), or a room's `room:<id>`. */
    fun threadKey(thread: String): String = thread.substringAfter('|')
    fun roomKey(room: String): String = if (room.isEmpty()) "" else "room:$room"
}

/** md/copy.rs, line for line: what a code span or block is. Used only with a hub that does not send `copy`. */
object CopyRules {
    private val SHELLS = setOf("sh", "bash", "shell", "zsh", "fish", "ksh", "shellscript", "powershell", "pwsh", "ps1", "ps", "cmd", "bat", "batch", "nu", "nushell")
    private val PROGRAMS = setOf(
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
    )
    private fun ws(c: Char) = c.isWhitespace()

    fun blockIsCommand(text: String, lang0: String): Boolean {
        val lang = lang0.trim().lowercase()
        if (lang in SHELLS) return text.isNotBlank()
        if (!(lang.isEmpty() || lang == "text" || lang == "txt" || lang == "plain")) return false
        var any = false; var cont = false
        for (l in text.lines()) {
            val t = l.trim()
            if (cont) { cont = t.endsWith('\\'); continue }
            if (t.isEmpty() || t.startsWith('#')) continue
            if (!lineIsCommand(t)) return false
            any = true; cont = t.endsWith('\\')
        }
        return any
    }

    fun lineIsCommand(line: String): Boolean {
        val t = line.trim().removePrefix("$ ")
        val first = t.split(Regex("\\s+")).filter { it.isNotEmpty() }.dropWhile { assignment(it) }.firstOrNull() ?: return false
        if (first in PROGRAMS) return true
        val pathy = first.startsWith("./") || first.startsWith("../") || first.startsWith("~/") || (first.startsWith('/') && first.substring(1).contains('/')) ||
            (first.contains('/') && (first.endsWith(".sh") || first.endsWith(".py") || first.endsWith(".ps1")))
        return pathy && first.all { it.isAsciiAlnum() || it in "._-/~+" }
    }
    private fun Char.isAsciiAlnum() = this in 'a'..'z' || this in 'A'..'Z' || this in '0'..'9'
    private fun Char.isAsciiAlpha() = this in 'a'..'z' || this in 'A'..'Z'
    private fun assignment(w: String): Boolean {
        val i = w.indexOf('='); if (i < 0) return false
        val k = w.substring(0, i)
        return k.isNotEmpty() && (k[0].isAsciiAlpha() || k[0] == '_') && k.all { it.isAsciiAlnum() || it == '_' }
    }

    /** "cmd", "url", "path", "hash", "id", or null (no button). */
    fun inlineKind(text: String): String? {
        val t = text.trim()
        if (t.isEmpty() || t.contains('\n') || t.codePointCount(0, t.length) > 2000) return null
        if (t.split(Regex("\\s+")).count { it.isNotEmpty() } >= 2 && lineIsCommand(t)) return "cmd"
        if (t.any(::ws)) return null
        if (t.startsWith("http://") || t.startsWith("https://") || t.startsWith("backplane://")) return "url"
        if (t.startsWith('/') || t.startsWith("~/") || t.startsWith("./") || t.startsWith("../") || (t.length > 2 && t[1] == ':' && (t[2] == '\\' || t[2] == '/')) ||
            (t.contains('/') && !t.contains("://") && t.any { it.isAsciiAlpha() })) return "path"
        val hex = t.all { it in '0'..'9' || it in 'a'..'f' }
        if (hex && t.length in 7..40 && t.any { it in '0'..'9' } && t.any { it.isAsciiAlpha() }) return "hash"
        if (t.length >= 3 && t.any { it in '0'..'9' } && t.all { it.isAsciiAlnum() || it in "-_.:@~|" } && t.any { it.isAsciiAlnum() }) return "id"
        return null
    }

    /** Each fenced block whole and each inline code span with a kind, in order; at most 16. */
    fun spans(text: String): List<CopyPiece> {
        val out = ArrayList<CopyPiece>()
        var rest = text
        while (out.size < 16) {
            val i = rest.indexOf('`'); if (i < 0) break
            val after = rest.substring(i)
            if (after.startsWith("```")) {
                val body = after.substring(3)
                val nl = body.indexOf('\n')
                val (info, code) = if (nl >= 0) body.substring(0, nl) to body.substring(nl + 1) else body to ""
                val end = code.indexOf("```"); if (end < 0) break
                val block = code.substring(0, end).trimEnd('\n')
                val lang = info.split(Regex("[\\s,{]")).firstOrNull() ?: ""
                if (block.isNotBlank()) out.add(CopyPiece(block, blockIsCommand(block, lang)))
                rest = code.substring(end + 3); continue
            }
            val inner = after.substring(1)
            val end = inner.indexOfFirst { it == '`' || it == '\n' }; if (end < 0) break
            if (inner[end] == '`') {
                val t = inner.substring(0, end)
                inlineKind(t)?.let { out.add(CopyPiece(t, it == "cmd")) }
                rest = inner.substring(end + 1)
            } else rest = inner.substring(end)
        }
        return out
    }
}
