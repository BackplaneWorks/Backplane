package dev.backplane.mobile

// What in a message's text is a link, as the phone finds it (the iOS app's DataLinkGrammar.swift is the same rules): web addresses, backplane:// links, emails,
// phone numbers and file paths (src/x.rs:12). Pure Kotlin, so a unit test runs it without the app. The hub decides what a backplane:// link or a path opens
// (link.resolve); this only says where the link is and what to hand it.

/** A link in running text: [end] is just past it, [url] what a tap hands on: a web address, mailto:, tel:, backplane://, or `bppath:<path[:line]>`. */
data class LinkHit(val end: Int, val url: String)

const val PATH_SCHEME = "bppath:"

private val reEmail = Regex("^[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\\.[A-Za-z0-9-]+)*\\.[A-Za-z]{2,}$")
private val rePath = Regex("^(?:~/|/|\\./|\\.\\./)?(?:[A-Za-z0-9_.@+-]+/)+[A-Za-z0-9_@+-][A-Za-z0-9_.@+-]*\\.[A-Za-z0-9]{1,10}(?::\\d{1,6}){0,2}(?:#L\\d{1,6}(?:-L?\\d{1,6})?)?$")
private val reFile = Regex("^[A-Za-z0-9_@+-][A-Za-z0-9_.@+-]*\\.(?:rs|swift|kt|kts|java|js|jsx|ts|tsx|py|go|c|h|cc|cpp|hpp|m|mm|md|txt|toml|json|yaml|yml|lock|sh|css|html|sql|kicad_pcb|kicad_sch|kicad_pro|kicad_sym|kicad_mod|step|stp|pdf|csv|log|ini|cfg|gradle|xml|svg|png|jpg)(?::\\d{1,6}){0,2}(?:#L\\d{1,6}(?:-L?\\d{1,6})?)?$")
private val rePhone = Regex("^(?:(?:\\+\\d{1,3}[ .-]?)?(?:\\(\\d{3}\\)[ .-]?|\\d{3}[ .-])\\d{3}[ .-]\\d{4}(?!\\d)|\\+\\d{1,3}(?:[ .-]?\\d{2,4}){2,4}(?!\\d))")

fun isPathText(t: String) = !t.startsWith("http") && !t.contains("://") && (rePath.matches(t) || reFile.matches(t))
fun pathLink(p: String) = PATH_SCHEME + p
fun pathOf(url: String): String? = if (url.startsWith(PATH_SCHEME)) url.removePrefix(PATH_SCHEME) else null

private fun boundary(s: String, i: Int) = i == 0 || s[i - 1].isWhitespace() || s[i - 1] in "([{<\"'`,;:"

/** The email, phone number or path that starts at s[i]. Web addresses are [webLink]'s. */
fun autoLink(s: String, i: Int): LinkHit? {
    if (i >= s.length || !boundary(s, i)) return null
    val c = s[i]
    if (!(c.isLetterOrDigit() || c in "+(/.~_@")) return null
    var e = i; while (e < s.length && !s[e].isWhitespace() && e - i < 300) e++
    var tok = s.substring(i, e)
    while (tok.isNotEmpty() && tok.last() in ".,;:!?)]}>'\"*`") {
        if (tok.last() == ')' && tok.count { it == '(' } >= tok.count { it == ')' }) break
        tok = tok.dropLast(1)
    }
    if (tok.isNotEmpty()) {
        if (reEmail.matches(tok)) return LinkHit(i + tok.length, "mailto:$tok")
        if (isPathText(tok)) return LinkHit(i + tok.length, pathLink(tok))
    }
    if (c == '+' || c == '(' || c.isDigit()) {
        val m = rePhone.find(s.substring(i, minOf(s.length, i + 24)))
        if (m != null) {
            val digits = m.value.filter { it.isDigit() }
            if (digits.length in 7..15) return LinkHit(i + m.value.length, "tel:" + (if (c == '+') "+" else "") + digits)
        }
    }
    return null
}

/** An http(s):// or backplane:// address that starts at s[i] (trailing marks and an unmatched ) stay out). */
fun webLink(s: String, i: Int): LinkHit? {
    if (i > 0 && s[i - 1].isLetterOrDigit()) return null
    if (!(s.startsWith("http://", i) || s.startsWith("https://", i) || s.startsWith("backplane://", i))) return null
    var e = i; while (e < s.length && !s[e].isWhitespace() && s[e] !in "<>\"'") e++
    while (e > i && s[e - 1] in ".,;:!?)]}*_") { if (s[e - 1] == ')' && s.substring(i, e).count { it == '(' } >= s.substring(i, e).count { it == ')' }) break; e-- }
    return LinkHit(e, s.substring(i, e))
}

/** Plain text cut into runs (text, link or null): a message that is not Markdown (a user's, a room post). */
fun linkRuns(text: String): List<Pair<String, String?>> {
    val out = ArrayList<Pair<String, String?>>(); val plain = StringBuilder(); var i = 0
    fun flush() { if (plain.isNotEmpty()) { out += plain.toString() to null; plain.setLength(0) } }
    while (i < text.length) {
        val h = webLink(text, i) ?: autoLink(text, i)
        if (h != null) { flush(); out += text.substring(i, h.end) to h.url; i = h.end; continue }
        plain.append(text[i]); i++
    }
    flush(); return out
}
