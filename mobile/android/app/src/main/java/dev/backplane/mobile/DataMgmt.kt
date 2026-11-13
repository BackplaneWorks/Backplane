package dev.backplane.mobile

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

// The management panels beside the phone's viewers (the desktop's strip under the pane; docs/MANAGEMENT.md): a row of tabs under the board or
// schematic (BOM, Checks, Lifecycle, Fab, with the hub's badges) opens a panel over the lower part of the screen, the drawing still in view above
// it. Everything comes from the hub's `mgmt.*` and `kicad.checks.*` info (DataClient.mgmt); the rules are the hub's, the app only lays rows out.

/** The theme's tones (DataViews' dok/dwarn/dfaint, from assets/web/style.css); bad is the scheme's error. */
@Composable private fun tone(name: String): Color = when (name) { "ok" -> dok; "warn" -> dwarn; "bad" -> dcs.error; "faint" -> dfaint; "accent" -> dcs.primary; else -> dcs.onSurfaceVariant }

object Mgmt {
    /** "C2, C4, C37 +1": the references that fit in `max` characters, then how many more. */
    fun refsFit(refs: List<String>, max: Int): String {
        val all = refs.joinToString(", "); if (all.length <= max) return all
        var out = ""
        for ((i, r) in refs.withIndex()) {
            val next = if (out.isEmpty()) r else "$out, $r"
            if (next.length + " +${refs.size - i - 1}".length > max) return if (out.isEmpty()) "${refs[0]} +${refs.size - 1}" else "$out +${refs.size - i}"
            out = next
        }
        return out
    }
    /** 1060770 as "1.1M", 12400 as "12k". */
    fun abbrev(n: Long): String = when {
        n < 0 -> "?"; n < 1000 -> n.toString()
        n < 1_000_000 -> (n / 1000.0).let { if (it < 10) "%.1fk".format(it).replace(".0k", "k") else "%.0fk".format(it) }
        else -> (n / 1e6).let { if (it < 10) "%.1fM".format(it).replace(".0M", "M") else "%.0fM".format(it) }
    }
    /** $1.20, $0.50, $0.0029 */
    fun money(v: Double): String { if (v >= 1) return "$" + "%.2f".format(v); val t = "%.4f".format(v).trimEnd('0'); return if (t.substringAfter('.').length < 2) "$" + "%.2f".format(v) else "$$t" }
    /** A footprint's package ("C_0402_1005Metric" → 0402, "QFN-48-1EP_6x6mm" → QFN-48), else its name without the library. */
    fun pkg(fp: String): String {
        val n = fp.substringAfter(':')
        n.split('_').firstOrNull { it in setOf("0201", "0402", "0603", "0805", "1206", "1210", "1812", "2010", "2512") }?.let { return it }
        val head = n.substringBefore('_'); val fam = head.takeWhile { it.isLetter() }.uppercase()
        if (fam in setOf("QFN", "DFN", "SOT", "SOIC", "SOP", "SSOP", "TSSOP", "MSOP", "QFP", "LQFP", "TQFP", "BGA", "WLCSP", "SOD", "TO", "SON", "LGA")) {
            val p = head.split('-'); return if (p.size >= 2 && p[1].all { it.isDigit() }) "${p[0]}-${p[1]}" else p[0]
        }
        // not a package people name: its first two words ("C_Disc", "D_DO-35", "R_Axial"), the rest is on the desktop
        return n.split('_').take(2).joinToString("_")
    }
    /** A line's stock in words and tone: "717k" ok, "low 12" warn, "out 0" bad, "…" checking, "?" unknown; null with nothing to look up. */
    fun stock(st: JSONObject?): Pair<String, String>? {
        val s = st?.optString("status").orEmpty(); if (s.isEmpty()) return null
        val n = st!!.optLong("stock", -1)
        return when (s) { "checking" -> "…" to "faint"; "unknown" -> "?" to "faint"; "out" -> Msg.gpui_mgmt_bom_stock_out(abbrev(maxOf(n, 0))) to "bad"; "low" -> Msg.gpui_mgmt_bom_stock_low(abbrev(n)) to "warn"; else -> abbrev(n) to "ok" }
    }
    /** The references of a finding's names ("GND · SW3.1 · R4"): parts, and the part of a pin. */
    fun findingRefs(names: String): List<String> = names.split('·').map { it.trim() }.mapNotNull { w -> val r = w.substringBefore('.'); if (Regex("^[A-Z#]{1,6}[0-9]{1,6}[A-Z]?$").matches(r)) r else null }.distinct()
    fun strs(a: JSONArray?): List<String> = (0 until (a?.length() ?: 0)).map { a!!.optString(it) }
    fun objs(a: JSONArray?): List<JSONObject> = (0 until (a?.length() ?: 0)).mapNotNull { a!!.optJSONObject(it) }
}

/** The tabs under the drawing, and the open panel above them. `project`: the viewer's project path (the root is enough). */
@Composable
internal fun MgmtHost(project: String, modifier: Modifier = Modifier) {
    val online = DataClient.conn == DataClient.Conn.ONLINE
    LaunchedEffect(project, online) { if (project.isNotEmpty() && online && DataClient.has("mgmt")) runCatching { DataClient.mgmtWatch(project) } }
    val d = DataClient.mgmtDigest
    val state = if (d.isEmpty()) null else DataClient.mgmt["mgmt.state.$d"]
    // the panels this app draws (a panel the hub has that the phone has no view for stays on the desktop)
    val panels = Mgmt.objs(state?.optJSONArray("panels")).filter { it.optString("id") in setOf("bom", "checks", "life", "fab") }
    if (panels.isEmpty()) return
    var open by rememberSaveable(project) { mutableStateOf("") }
    Column(modifier.fillMaxWidth()) {
        if (open.isNotEmpty()) {
            Column(Modifier.fillMaxWidth().fillMaxHeight(.55f).background(dcs.background).testTag("mgmt.panel.$open")) {
                DLine()
                when (open) { "bom" -> BomPanel(); "checks" -> ChecksPanel(); "life" -> LifePanel(); "fab" -> FabPanel(); else -> {} }
            }
        }
        DLine()
        Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
            panels.forEach { p ->
                val id = p.optString("id"); val on = id == open
                val badge = DataClient.mgmtData(id)?.optJSONObject("badge"); val n = badge?.optInt("n") ?: 0
                Row(Modifier.clickable { open = if (on) "" else id; if (open.isEmpty()) DataClient.lightRefs = emptyList() }.background(if (on) dcs.background else dcs.surfaceVariant)
                    .padding(horizontal = 14.dp, vertical = 12.dp).testTag("mgmt.tab.$id"), verticalAlignment = Alignment.CenterVertically) {
                    DMono(Tr.t(p.optString("label")), if (on) dcs.onSurface else dcs.onSurfaceVariant, 12, bold = on)
                    if (n > 0) { Spacer(Modifier.width(6.dp)); val t = tone(if (badge?.optString("tone") == "bad") "bad" else "warn")
                        DMono(if (n > 99) "99+" else n.toString(), t, 10, Modifier.border(BorderStroke(1.dp, t)).padding(horizontal = 4.dp, vertical = 1.dp)) }
                }
            }
            Spacer(Modifier.weight(1f))
            if (open.isNotEmpty()) Box(Modifier.size(44.dp).clickable { open = ""; DataClient.lightRefs = emptyList() }.testTag("mgmt.close"), contentAlignment = Alignment.Center) {
                Icon(Icons.Default.Close, Msg.common_close, tint = dcs.onSurfaceVariant, modifier = Modifier.size(18.dp)) }
        }
    }
}

// ---- BOM: two-line rows (references and MPN; quantity, value, package, price, class), the stock in words; a tap lights the parts, a long press edits
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun BomPanel() {
    val d = DataClient.mgmtData("bom")
    if (d == null) { DMono(Msg.gpui_mgmt_bom_reading, dcs.onSurfaceVariant, 12, Modifier.padding(12.dp)); return }
    val lines = Mgmt.objs(d.optJSONArray("lines"))
    var q by remember { mutableStateOf("") }
    var editing by remember { mutableStateOf<JSONObject?>(null) }
    var sel by remember { mutableStateOf("") }
    Column(Modifier.fillMaxSize()) {
        // what it holds, where from, the cost a board, and the filter
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DMono(Msg.gpui_mgmt_bom_summary(d.optInt("parts"), lines.size), dcs.onSurfaceVariant, 11)
            val live = d.optString("source") == "kicad"
            DMono(if (live) Msg.history_live else Msg.gpui_mgmt_bom_saved_state, if (live) dcs.primary else dcs.onSurfaceVariant, 10, Modifier.border(BorderStroke(1.dp, if (live) dcs.primary else dcs.outlineVariant)).padding(horizontal = 5.dp, vertical = 1.dp))
            val priced = d.optInt("priced"); val looked = d.optInt("lookups")
            if (priced > 0 && d.optInt("checking") == 0) DMono(if (priced < looked) Msg.gpui_mgmt_bom_cost_least(Mgmt.money(d.optDouble("cost"))) else Msg.gpui_mgmt_bom_cost(Mgmt.money(d.optDouble("cost"))), dcs.onSurfaceVariant, 11)
            if (d.optBoolean("paused")) DMono(Msg.gpui_mgmt_bom_paused, tone("faint"), 11)
        }
        // a long-pressed line's editor, in the panel (not a box over it): the drawing stays in view
        val ed = editing
        if (ed != null) BomEdit(ed) { editing = null }
        else Box(Modifier.padding(horizontal = 12.dp)) { DField(q, { q = it }, Msg.gpui_mgmt_filter, Modifier.testTag("mgmt.bom.filter")) }
        DLine()
        val shown = lines.filter { l -> q.isBlank() || q.lowercase().split(' ').filter { it.isNotEmpty() }.all { w -> (w == "dnp" && l.optBoolean("dnp")) ||
            listOf(Mgmt.strs(l.optJSONArray("refs")).joinToString(" "), l.optString("value"), l.optString("footprint"), l.optString("mpn"), l.optString("fab")).joinToString(" ").lowercase().contains(w) } }
        val started = lines.any { it.optString("mpn").isNotEmpty() || it.optString("fab").isNotEmpty() }
        LazyColumn(Modifier.weight(1f).testTag("mgmt.bom.rows")) {
            items(shown, key = { it.optString("id") }) { l ->
                val id = l.optString("id"); val refs = Mgmt.strs(l.optJSONArray("refs")); val dnp = l.optBoolean("dnp"); val st = l.optJSONObject("stock")
                val faint = if (dnp) tone("faint") else dcs.onSurface
                Column(Modifier.fillMaxWidth().background(if (sel == id) dcs.surfaceVariant else dcs.background)
                    .combinedClickable(onClick = { sel = id; DataClient.lightRefs = refs }, onLongClick = { editing = l })
                    .padding(horizontal = 12.dp, vertical = 8.dp).testTag("mgmt.bom.row.$id")) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        DMono(Mgmt.refsFit(refs, 18), faint, 13, Modifier.weight(1f), bold = true)
                        if (dnp) DMono(Msg.gpui_mgmt_bom_dnp, tone("faint"), 10, Modifier.border(BorderStroke(1.dp, dcs.outlineVariant)).padding(horizontal = 4.dp, vertical = 1.dp))
                        else Mgmt.stock(st)?.let { (t, tn) -> DMono(t, tone(tn), 12) }
                    }
                    Row(Modifier.padding(top = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                        val mpn = l.optString("mpn")
                        val desc = listOf("${l.optInt("qty")}×", l.optString("value"), Mgmt.pkg(l.optString("footprint"))).filter { it.isNotEmpty() }.joinToString(" · ")
                        DMono(desc, dcs.onSurfaceVariant, 11)
                        Spacer(Modifier.width(8.dp))
                        if (mpn.isNotEmpty()) DMono(mpn, faint, 11, Modifier.weight(1f)) else if (started && !dnp) DMono(Msg.gpui_mgmt_bom_nompn, tone("warn"), 11, Modifier.weight(1f)) else Spacer(Modifier.weight(1f))
                        if (!dnp && st != null && st.has("price")) DMono(Mgmt.money(st.optDouble("price")), dcs.onSurfaceVariant, 11)
                        if (!dnp && st?.optString("class").orEmpty().isNotEmpty()) { Spacer(Modifier.width(6.dp)); DMono(Tr.t(st!!.optString("class")), tone("faint"), 10) }
                    }
                }
                DLine()
            }
        }
    }
}

/** A line's MPN and DNP, written as the person's own edit (the same call as the desktop's): into the open KiCad, else the saved schematic. */
@Composable
private fun BomEdit(l: JSONObject, close: () -> Unit) {
    val scope = rememberCoroutineScope()
    val refs = Mgmt.strs(l.optJSONArray("refs"))
    var mpn by remember(l) { mutableStateOf(l.optString("mpn")) }
    var said by remember(l) { mutableStateOf("") }
    var last by remember(l) { mutableStateOf<JSONObject?>(null) }
    run {
        Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 12.dp, vertical = 10.dp).testTag("mgmt.bom.edit"), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                DMono(Mgmt.refsFit(refs, 28), dcs.onSurface, 13, Modifier.weight(1f), bold = true)
                Box(Modifier.size(36.dp).clickable(onClick = close).testTag("mgmt.bom.edit.close"), contentAlignment = Alignment.Center) { Icon(Icons.Default.Close, Msg.common_close, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp)) }
            }
            DMono(listOf(l.optString("value"), Mgmt.pkg(l.optString("footprint"))).filter { it.isNotEmpty() }.joinToString(" · "), dcs.onSurfaceVariant, 11)
            DField(mpn, { mpn = it }, Msg.gpui_mgmt_bom_mpn_ph, Modifier.testTag("mgmt.bom.edit.mpn"))
            fun write(field: String, value: String, undo: Boolean = false) = scope.launch {
                said = Msg.gpui_mgmt_bom_writing
                runCatching { DataClient.bomEdit(refs, field, value) }.onSuccess { r -> last = if (undo) null else r; said = when { undo -> Msg.gpui_mgmt_bom_putback; r.optInt("changed") == 0 -> Msg.gpui_mgmt_bom_nochange
                    r.optString("via") == "kicad" && r.optString("undo") == "kicad" -> Msg.gpui_mgmt_bom_inkicad; r.optString("via") == "kicad" -> Msg.gpui_mgmt_bom_inkicad_save; else -> Msg.gpui_mgmt_bom_saved_file } }
                    .onFailure { said = Tr.t(it.message ?: "") }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                DBtn(Msg.common_save, true, mpn != l.optString("mpn"), Modifier.testTag("mgmt.bom.edit.save")) { write("mpn", mpn.trim()) }
                DBtn(Msg.gpui_mgmt_bom_dnp, l.optBoolean("dnp"), modifier = Modifier.testTag("mgmt.bom.edit.dnp")) { write("dnp", if (l.optBoolean("dnp")) "no" else "yes") }
                // the panel's own undo: the old value written back the same way
                last?.takeIf { it.optInt("changed") > 0 }?.let { r -> DBtn(Msg.gpui_mgmt_bom_undo, false, modifier = Modifier.testTag("mgmt.bom.edit.undo")) { write(r.optString("field"), r.optString("was"), undo = true) } }
            }
            if (said.isNotEmpty()) DMono(said, dcs.onSurfaceVariant, 11, lines = 3)
        }
    }
}

// ---- Checks: DRC/ERC and every other source, by rule, worst first; a tap lights the parts, "Fix with agent" sends the finding to the thread
@Composable
private fun ChecksPanel() {
    val d = DataClient.mgmtData("checks")
    if (d == null) { DMono(Msg.android_nothing, dcs.onSurfaceVariant, 12, Modifier.padding(12.dp)); return }
    val scope = rememberCoroutineScope()
    var src by remember { mutableStateOf("") }
    var sel by remember { mutableStateOf("") }
    var said by remember { mutableStateOf("") }
    val groups = Mgmt.objs(d.optJSONArray("groups"))
    val sources = groups.map { it.optString("source") }.distinct()
    val label = { s: String -> d.optJSONObject("sources")?.optJSONObject(s)?.optString("label")?.takeIf { it.isNotEmpty() } ?: s.uppercase() }
    val judged = { s: String -> d.optJSONObject("sources")?.optJSONObject(s)?.optBoolean("judged") == true }
    Column(Modifier.fillMaxSize()) {
        // the sources as filters: DRC, ERC, the review's rules, the agent-judged ones (marked)
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            DBtn(Msg.android_mgmt_all, src.isEmpty(), modifier = Modifier.testTag("mgmt.checks.src.all")) { src = "" }
            sources.forEach { s -> DBtn(label(s) + if (judged(s)) " · " + Msg.android_mgmt_judged else "", src == s, modifier = Modifier.testTag("mgmt.checks.src.$s")) { src = if (src == s) "" else s } }
        }
        if (said.isNotEmpty()) DMono(said, dcs.onSurfaceVariant, 11, Modifier.padding(horizontal = 12.dp))
        DLine()
        LazyColumn(Modifier.weight(1f).testTag("mgmt.checks.rows")) {
            groups.filter { src.isEmpty() || it.optString("source") == src }.forEach { g ->
                val sev = g.optString("severity"); val tn = if (sev == "error") "bad" else if (sev == "warning") "warn" else "faint"
                item(key = "g:" + g.optString("source") + g.optString("rule")) { val t = tone(tn)
                    Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        DDot(t, 7); Spacer(Modifier.width(8.dp))
                        DMono(label(g.optString("source")) + " · " + g.optString("rule"), dcs.onSurface, 12, Modifier.weight(1f), bold = true)
                        DMono(g.optInt("n").toString(), t, 11)
                    }
                }
                items(Mgmt.objs(g.optJSONArray("rows")), key = { "r:" + it.optString("id") }) { r ->
                    val id = r.optString("id")
                    Column(Modifier.fillMaxWidth().background(if (sel == id) dcs.surfaceVariant else dcs.background).clickable {
                        sel = id; DataClient.lightRefs = Mgmt.findingRefs(r.optString("names")); scope.launch { runCatching { DataClient.checksFocus(id) } }
                    }.padding(horizontal = 12.dp, vertical = 8.dp).testTag("mgmt.checks.row.$id")) {
                        DMono(Tr.t(r.optString("title")), dcs.onSurface, 12, lines = 2)
                        if (r.optString("names").isNotEmpty() || sel == id) Row(verticalAlignment = Alignment.CenterVertically) {
                            DMono(r.optString("names"), dcs.onSurfaceVariant, 11, Modifier.weight(1f))
                            if (sel == id) DMono(Msg.android_mgmt_fix, dcs.primary, 11, Modifier.clickable {
                                scope.launch { runCatching { DataClient.checksFix(id) }.onSuccess { text -> if (text.isNotEmpty()) { val tid = DataClient.viewerThread; if (tid.isNotEmpty()) DataClient.send(tid, text); said = Msg.android_mgmt_sent } }.onFailure { said = Tr.t(it.message ?: "") } }
                            }.padding(start = 8.dp).testTag("mgmt.checks.fix.$id"))
                        }
                    }
                    DLine()
                }
            }
        }
    }
}

@Composable private fun LifePanel() { DMono(Msg.android_nothing, dcs.onSurfaceVariant, 12, Modifier.padding(12.dp)) }
@Composable private fun FabPanel() { DMono(Msg.android_nothing, dcs.onSurfaceVariant, 12, Modifier.padding(12.dp)) }
