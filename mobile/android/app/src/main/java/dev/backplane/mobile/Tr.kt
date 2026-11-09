package dev.backplane.mobile

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/** The app's language and its messages (i18n/en.msg, i18n/ja.msg through Msg.kt, scripts/i18n-android.py).
 *  `lang` is "en" or "ja": the device's choice (AppSettings.lang: auto, en, ja) with auto following the system's language.
 *  It is Compose state, so a screen that read a message redraws when it changes. Rendered messages are cached per language. */
object Tr {
    /** The system's language, read at start and on each configuration change (MainActivity). */
    var system by mutableStateOf(java.util.Locale.getDefault().language)
    /** A JVM test sets this to skip AppSettings. */
    internal var fixed: String? = null
    val lang: String get() = fixed ?: resolve(AppSettings.lang, system)
    fun resolve(choice: String, systemLanguage: String) = when (choice) { "ja" -> "ja"; "en" -> "en"; else -> if (systemLanguage == "ja") "ja" else "en" }

    private val lock = Any()
    private var cacheLang = ""
    private val plainCache = HashMap<Int, String>()
    private val textCache = object : LinkedHashMap<String, String>(256, .75f, true) { override fun removeEldestEntry(e: MutableMap.MutableEntry<String, String>) = size > 4000 }
    private fun ready(l: String) { if (l != cacheLang) { plainCache.clear(); textCache.clear(); cacheLang = l } }

    /** A message without arguments. */
    fun s(i: Int): String { val l = lang; synchronized(lock) { ready(l); return plainCache.getOrPut(i) { render(nodes(l, i), emptyMap(), l) } } }
    /** A message with its arguments, in Msg's order. */
    fun f(i: Int, values: Array<Any>): String {
        val l = lang; val names = Msg.argNames[i] ?: emptyArray()
        return render(nodes(l, i), names.indices.associate { names[it] to values[it] }, l)
    }
    /** A message by key, for keys chosen at run time (a hub's word picks one): the key itself when there is none. */
    fun key(k: String): String { val i = index[k] ?: return k; return if (Msg.argNames.containsKey(i)) k else s(i) }
    fun has(k: String) = index.containsKey(k)

    /** English display text in the current language: an exact catalog message, or a templated one with what it captured;
     *  anything else (names, agent text, paths) as it came. */
    fun t(text: String): String {
        val l = lang
        if (l == "en" || text.isBlank() || text.length > 600) return text
        synchronized(lock) { ready(l); textCache[text]?.let { return it } }
        val out = translate(text, l) ?: text
        synchronized(lock) { ready(l); textCache[text] = out }
        return out
    }
    /** The same for text the app or hub shows in capitals ("BOTS"): matched without case. */
    fun tUpper(text: String): String { val r = t(text); if (r != text || lang == "en") return r; return upperExact[text]?.let { s(it) } ?: text }

    // ---- the table
    private val index: Map<String, Int> by lazy { Msg.keys.withIndex().associate { it.value to it.index } }
    private val exact: Map<String, Int> by lazy { HashMap<String, Int>().also { m -> Msg.exactText.forEachIndexed { n, t -> m[t] = Msg.exactKey[n] } } }
    private val upperExact: Map<String, Int> by lazy { HashMap<String, Int>().also { m -> Msg.exactText.forEachIndexed { n, t -> m.putIfAbsent(t.uppercase(), Msg.exactKey[n]) } } }
    private val parsedEn = HashMap<Int, List<Node>>(); private val parsedJa = HashMap<Int, List<Node>>()
    private fun nodes(l: String, i: Int): List<Node> = synchronized(parsedEn) {
        if (l == "ja") parsedJa.getOrPut(i) { Icu.parse(Msg.ja[i]) } else parsedEn.getOrPut(i) { Icu.parse(Msg.en[i]) }
    }
    private val variants: List<Icu.Variant> by lazy { Msg.templates.flatMap { i -> Icu.variants(i, Icu.parse(Msg.en[i])) }.sortedByDescending { it.letters } }

    private fun translate(text: String, l: String): String? {
        whole(text, l)?.let { return it }
        // a line of parts the hub joined ("Paired · 3 days ago"): each part on its own, the rest as it came
        if (text.contains(" · ")) {
            val parts = text.split(" · "); val done = parts.map { p -> whole(p, l) }
            if (done.any { it != null }) return parts.indices.joinToString(" · ") { done[it] ?: parts[it] }
        }
        return null
    }
    private fun whole(text: String, l: String, depth: Int = 0): String? {
        exact[text]?.let { return s(it) }
        for (v in variants) {
            if (!text.contains(v.anchor)) continue
            val got = v.match(text) ?: continue
            // what it captured may be catalog text itself ("Download failed: {error}" with the hub's own error): that too, once
            if (depth == 0) for ((k, x) in got) if (x is String && !x.startsWith("\u0000")) whole(x, l, 1)?.let { got[k] = it }
            return render(nodes(l, v.key), got, l)
        }
        return null
    }

    internal fun render(ns: List<Node>, a: Map<String, Any>, l: String, hash: Long? = null): String {
        val b = StringBuilder()
        for (n in ns) when (n) {
            is Node.Text -> b.append(n.s)
            is Node.Arg -> b.append(a[n.name]?.let { if (it is Number) number(it.toLong()) else it.toString() } ?: "{${n.name}}")
            is Node.Num -> b.append(a[n.name]?.let { num(it)?.let(::number) ?: it.toString() } ?: "{${n.name}}")
            is Node.Hash -> b.append(hash?.let(::number) ?: "#")
            is Node.Plural -> {
                val v = num(a[n.name]) ?: 0L
                val c = n.cases.firstOrNull { it.first == "=$v" } ?: n.cases.firstOrNull { it.first == category(v, l) } ?: n.cases.first { it.first == "other" }
                b.append(render(c.second, a, l, v))
            }
            is Node.Select -> {
                val v = a[n.name]?.toString() ?: ""
                val c = n.cases.firstOrNull { it.first == v && v != "other" } ?: n.cases.first { it.first == "other" }
                b.append(render(c.second, a, l, hash))
            }
        }
        return b.toString()
    }
    private fun num(v: Any?): Long? = when (v) { is Number -> v.toLong(); is String -> v.replace(",", "").toLongOrNull(); else -> null }
    private fun category(n: Long, l: String) = if (l == "ja") "other" else if (n == 1L) "one" else "other"
    fun number(n: Long): String = if (n < 0) "-" + number(-n) else n.toString().reversed().chunked(3).joinToString(",").reversed()
}

/** A parsed message. */
sealed interface Node {
    data class Text(val s: String) : Node
    data class Arg(val name: String) : Node
    data class Num(val name: String) : Node
    object Hash : Node
    data class Plural(val name: String, val cases: List<Pair<String, List<Node>>>) : Node
    data class Select(val name: String, val cases: List<Pair<String, List<Node>>>) : Node
}

/** The ICU subset of the catalogs: {a}, {n, number}, {n, plural, =0 {..} one {..} other {..}}, {w, select, a {..} other {..}}, # inside a plural,
 *  apostrophe quoting before {}#| and '' for an apostrophe. The catalog gate (scripts/i18n-android.py) has parsed every message already. */
object Icu {
    fun parse(s: String, inPlural: Boolean = false): List<Node> {
        val out = ArrayList<Node>(); val buf = StringBuilder(); var i = 0
        fun flush() { if (buf.isNotEmpty()) { out += Node.Text(buf.toString()); buf.clear() } }
        while (i < s.length) {
            val c = s[i]
            if (c == '\'') {
                val nx = s.getOrNull(i + 1)
                if (nx == '\'') { buf.append('\''); i += 2; continue }
                if (nx != null && nx in "{}#|") {
                    var j = i + 1
                    while (j < s.length) { if (s[j] == '\'') { if (s.getOrNull(j + 1) == '\'') { buf.append('\''); j += 2; continue }; break }; buf.append(s[j]); j++ }
                    i = j + 1; continue
                }
                buf.append('\''); i++; continue
            }
            if (c == '#' && inPlural) { flush(); out += Node.Hash; i++; continue }
            if (c == '{') {
                flush(); val j = close(s, i); val parts = s.substring(i + 1, j).split(",", limit = 3).map { it.trim() }
                val name = parts[0]
                out += when {
                    parts.size == 1 -> Node.Arg(name)
                    parts.size == 2 && parts[1] == "number" -> Node.Num(name)
                    parts.size == 3 && (parts[1] == "plural" || parts[1] == "select") -> {
                        val cases = ArrayList<Pair<String, List<Node>>>(); val body = parts[2]; var k = 0
                        while (k < body.length) {
                            if (body[k].isWhitespace()) { k++; continue }
                            val m = Regex("""(=\d+|[a-z]+)\s*\{""").matchAt(body, k) ?: error("bad case in $s")
                            val start = m.range.last; val e = close(body, start)
                            cases += m.groupValues[1] to parse(body.substring(start + 1, e), inPlural || parts[1] == "plural"); k = e + 1
                        }
                        if (parts[1] == "plural") Node.Plural(name, cases) else Node.Select(name, cases)
                    }
                    else -> error("bad placeholder in $s")
                }
                i = j + 1; continue
            }
            buf.append(c); i++
        }
        flush(); return out
    }
    private fun close(s: String, i: Int): Int {
        var d = 0; var j = i
        while (j < s.length) {
            val c = s[j]
            if (c == '\'' && s.getOrNull(j + 1)?.let { it in "{}#|'" } == true) {
                if (s[j + 1] == '\'') { j += 2; continue }
                j++; while (j < s.length && s[j] != '\'') j++; j++; continue
            }
            if (c == '{') d++ else if (c == '}') { d--; if (d == 0) return j }
            j++
        }
        error("unbalanced braces in $s")
    }

    /** One way a templated English message can read: its literal text and what it captures, as a pattern, and the
     *  arguments a choice's branch implies (a select's case, a plural's =N). */
    class Variant(val key: Int, private val re: Regex, private val groups: List<Pair<String, Boolean>>, private val fixed: Map<String, Any>, val letters: Int, val anchor: String) {
        fun match(text: String): MutableMap<String, Any>? {
            val m = re.matchEntire(text) ?: return null
            val got = HashMap<String, Any>(fixed)
            groups.forEachIndexed { n, (name, isNum) ->
                val raw = m.groupValues[n + 1]
                val v: Any = if (isNum) raw.replace(",", "").toLongOrNull() ?: return null else raw
                val was = got[name]; if (was != null && was != "\u0000other" && was.toString() != v.toString()) return null
                got[name] = v
            }
            return got
        }
    }
    /** Plain placeholders that always hold a count in the catalogs ("{count} {plural, select, …}"): read back as a number. */
    private val counts = setOf("n", "count", "total", "num")
    private sealed interface Tok { data class Lit(val s: String) : Tok; data class Cap(val name: String, val num: Boolean) : Tok }
    private class Path(val toks: List<Tok>, val fixed: Map<String, Any>)

    fun variants(key: Int, ns: List<Node>): List<Variant> {
        val numeric = HashSet<String>(); fun scan(l: List<Node>) { l.forEach { when (it) { is Node.Num -> numeric += it.name; is Node.Plural -> { numeric += it.name; it.cases.forEach { c -> scan(c.second) } }; is Node.Select -> it.cases.forEach { c -> scan(c.second) }; else -> {} } } }
        scan(ns)
        var paths = expand(ns, null, numeric)
        if (paths.size > 64) paths = paths.take(64)
        return paths.mapNotNull { p ->
            val toks = ArrayList<Tok>(); p.toks.forEach { t -> if (t is Tok.Lit && toks.lastOrNull() is Tok.Lit) toks[toks.size - 1] = Tok.Lit((toks.last() as Tok.Lit).s + t.s) else toks += t }
            if (toks.none { it is Tok.Lit }) return@mapNotNull null
            val re = StringBuilder("^"); val groups = ArrayList<Pair<String, Boolean>>()
            toks.forEach { t -> when (t) { is Tok.Lit -> re.append(Regex.escape(t.s)); is Tok.Cap -> { re.append(if (t.num) "(-?\\d[\\d,]*)" else "((?:(?! · ).)+?)"); groups += t.name to t.num } } }
            re.append("$")
            val lits = toks.filterIsInstance<Tok.Lit>().map { it.s }
            val letters = lits.sumOf { s -> s.count { it.isLetter() } }
            if (letters < 4) return@mapNotNull null
            Variant(key, Regex(re.toString(), RegexOption.DOT_MATCHES_ALL), groups, p.fixed, letters, lits.maxBy { it.length })
        }
    }
    private fun expand(ns: List<Node>, plural: String?, numeric: Set<String>): List<Path> {
        var acc = listOf(Path(emptyList(), emptyMap()))
        for (n in ns) {
            val alts: List<Path> = when (n) {
                is Node.Text -> listOf(Path(listOf(Tok.Lit(n.s)), emptyMap()))
                is Node.Arg -> listOf(Path(listOf(Tok.Cap(n.name, n.name in numeric || n.name in counts)), emptyMap()))
                is Node.Num -> listOf(Path(listOf(Tok.Cap(n.name, true)), emptyMap()))
                is Node.Hash -> listOf(Path(listOf(plural?.let { Tok.Cap(it, true) } ?: Tok.Lit("#")), emptyMap()))
                is Node.Plural -> n.cases.flatMap { (c, sub) ->
                    val fix: Map<String, Any> = when { c.startsWith("=") -> mapOf(n.name to c.drop(1).toLong()); c == "one" -> mapOf(n.name to 1L); else -> emptyMap() }
                    expand(sub, n.name, numeric).map { Path(it.toks, fix + it.fixed) }
                        // an "other" branch with no # names no number: it cannot be read back
                        .filter { p -> c.startsWith("=") || c == "one" || p.toks.any { it is Tok.Cap && it.name == n.name } }
                }
                is Node.Select -> n.cases.flatMap { (c, sub) -> expand(sub, plural, numeric).map { Path(it.toks, mapOf<String, Any>(n.name to (if (c == "other") "\u0000other" else c)) + it.fixed) } }
            }
            acc = acc.flatMap { a -> alts.map { b -> Path(a.toks + b.toks, a.fixed + b.fixed) } }
            if (acc.size > 256) acc = acc.take(256)
        }
        return acc
    }
}
