package dev.backplane.mobile

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

// Settings, Accounts (feature "accounts"; docs/PHONE-DATA.md): the hub's Claude and Codex accounts, and their sign-in from this phone. The hub runs the provider's own sign-in and
// decides every word and button here (native/host/src/settings/accounts_phone.rs); the phone opens the sign-in page in the browser and, for Claude, sends back the code that page
// shows. No password is ever typed here. The same screens as the iOS app's DataAccounts.swift.

private fun JSONArray?.objects(): List<JSONObject> = if (this == null) emptyList() else (0 until length()).map { getJSONObject(it) }
private fun JSONArray?.strings(): List<String> = if (this == null) emptyList() else (0 until length()).map { getString(it) }

@Composable
private fun tone(t: String): Color = when (t) { "ok" -> dok; "bad" -> dcs.error; "warn" -> dwarn; else -> dfaint }

@Composable
internal fun AccountsScreen(back: () -> Unit, open: (String) -> Unit) {
    var page by remember { mutableStateOf<JSONObject?>(null) }
    var failure by remember { mutableStateOf("") }
    var adding by remember { mutableStateOf<JSONObject?>(null) }
    var name by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()
    suspend fun load() { try { page = DataClient.call("accounts.page"); failure = "" } catch (e: Exception) { failure = e.message ?: "" } }
    LaunchedEffect(DataClient.conn) { load() }
    Column(Modifier.fillMaxSize()) {
        DBar("Accounts", back)
        Column(Modifier.padding(16.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            val p = page
            if (p == null) DMono(if (failure.isEmpty()) "Asking the hub…" else failure, if (failure.isEmpty()) dfaint else dcs.error, 12)
            else {
                p.optJSONArray("providers").objects().forEach { pv ->
                    DMono(pv.optString("title").uppercase(), dfaint)
                    val accounts = pv.optJSONArray("accounts").objects()
                    if (accounts.isEmpty()) DMono("No accounts", dfaint, 12)
                    accounts.forEach { a -> AccountRow(a) { open(a.optString("id")) } }
                    DBtn(pv.optString("add"), false, modifier = Modifier.testTag("accounts.add.${pv.optString("provider")}")) { name = ""; adding = pv }
                    DLine()
                }
                if (p.optString("error").isNotEmpty()) DMono(p.optString("error"), dcs.error, 11, lines = 4)
                if (failure.isNotEmpty()) DMono(failure, dcs.error, 11, lines = 4)
                DMono(p.optString("note"), dcs.onSurfaceVariant, 11, lines = 3)
            }
        }
    }
    adding?.let { pv ->
        AlertDialog(onDismissRequest = { adding = null }, title = { Text(pv.optString("add"), fontFamily = BpMono, fontSize = 14.sp) },
            text = { DField(name, { name = it }, "Name (Work, Personal)", Modifier.testTag("accounts.add.name")) },
            confirmButton = { TextButton(onClick = {
                adding = null
                scope.launch {
                    try { val r = DataClient.call("account.act", JSONObject().put("action", "add").put("provider", pv.optString("provider")).put("label", name)); open(r.optJSONObject("account")?.optString("id") ?: return@launch) }
                    catch (e: Exception) { failure = e.message ?: "" }
                }
            }) { Text("Add and sign in", fontFamily = BpMono) } },
            dismissButton = { TextButton(onClick = { adding = null }) { Text("Cancel", fontFamily = BpMono) } })
    }
}

@Composable
private fun AccountRow(a: JSONObject, onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth().border(BorderStroke(1.dp, dcs.outlineVariant)).background(dcs.surfaceVariant).clickable(onClick = onClick).padding(10.dp).testTag("accounts.row.${a.optString("id")}"),
        horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Box(Modifier.padding(top = 5.dp)) { DDot(tone(a.optString("tone")), 8) }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(a.optString("label"), Modifier.weight(1f), color = dcs.onSurface, fontFamily = BpMono, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                DMono(a.optString("line"), tone(a.optString("tone")), 11)
            }
            if (a.optString("email").isNotEmpty()) DMono(a.optString("email"), dcs.onSurfaceVariant, 11)
            if (a.optString("limits").isNotEmpty()) DMono(a.optString("limits"), dfaint, 11, lines = 2)
            if (a.optString("hold").isNotEmpty()) DMono(a.optString("hold"), dwarn, 11)
        }
    }
}

/** One account. `start`: an alert's tap opened it, so its sign-in starts at once (unless one is under way or it works again). */
@Composable
internal fun AccountScreen(id: String, start: Boolean, back: () -> Unit) {
    var a by remember(id) { mutableStateOf<JSONObject?>(null) }
    var failure by remember(id) { mutableStateOf("") }
    var code by remember(id) { mutableStateOf("") }
    var confirm by remember(id) { mutableStateOf(false) }
    var started by remember(id) { mutableStateOf(false) }
    var copied by remember(id) { mutableStateOf("") }
    val scope = rememberCoroutineScope(); val ctx = LocalContext.current
    suspend fun load() { try { a = DataClient.call("account.view", JSONObject().put("profile", id)).optJSONObject("account"); failure = "" } catch (e: Exception) { failure = e.message ?: "" } }
    suspend fun act(action: String, extra: JSONObject.() -> Unit = {}) {
        val p = JSONObject().put("profile", id).put("action", action).apply(extra)
        try {
            val r = DataClient.call("account.act", p)
            if (action == "remove") { back(); return }
            a = r.optJSONObject("account"); failure = ""; if (action == "code") code = ""
        } catch (e: Exception) { failure = e.message ?: ""; load() }
    }
    fun copy(label: String, text: String) { (ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText(label, text)); copied = label }
    LaunchedEffect(id, DataClient.conn) {
        load()
        val x = a
        if (start && !started && x != null && x.optJSONObject("signin")?.optBoolean("busy") != true && x.optString("state") != "connected") { started = true; act("signin") }
    }
    // a sign-in under way: the hub is asked again until it is over (the only polling here, and only then)
    val busy = a?.optJSONObject("signin")?.optBoolean("busy") == true || a?.optString("state") == "connecting"
    LaunchedEffect(id, busy) { while (busy) { delay(1500); load(); if (a?.optJSONObject("signin")?.optBoolean("busy") != true && a?.optString("state") != "connecting") break } }
    Column(Modifier.fillMaxSize()) {
        DBar(a?.let { "${it.optString("title")} · ${it.optString("label")}" } ?: "Account", back)
        Column(Modifier.padding(16.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            val x = a
            if (x == null) DMono(if (failure.isEmpty()) "Asking the hub…" else failure, if (failure.isEmpty()) dfaint else dcs.error, 12)
            else {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.testTag("account.state")) { DDot(tone(x.optString("tone")), 8); Spacer(Modifier.width(8.dp)); DMono(x.optString("line"), tone(x.optString("tone")), 13, bold = true) }
                if (x.optString("email").isNotEmpty()) DMono(if (x.optString("plan").isEmpty()) x.optString("email") else "${x.optString("email")} · ${x.optString("plan")}", dcs.onSurfaceVariant, 12)
                if (x.optString("message").isNotEmpty()) DMono(x.optString("message"), dcs.onSurfaceVariant, 12, lines = 4)
                x.optJSONArray("limits").strings().forEach { DMono(it, dfaint, 12) }
                if (x.optString("hold").isNotEmpty()) DMono(x.optString("hold"), dwarn, 12)
                x.optJSONObject("signin")?.let { s ->
                    DLine()
                    Text(s.optString("text"), color = if (s.optBoolean("done")) dok else dcs.onSurface, fontFamily = BpMono, fontSize = 13.sp, modifier = Modifier.testTag("account.signin.text"))
                    val url = s.optString("url")
                    if (url.isNotEmpty()) Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        DBtn("Open sign-in page", true, modifier = Modifier.testTag("account.signin.open")) { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
                        DBtn(if (copied == "link") "Copied" else "Copy link", false) { copy("link", url) }
                    }
                    val c = s.optString("code")
                    if (c.isNotEmpty()) {
                        Text(c, color = dcs.onSurface, fontFamily = BpMono, fontSize = 28.sp, fontWeight = FontWeight.Bold, modifier = Modifier.testTag("account.signin.code"))
                        DBtn(if (copied == "code") "Copied" else "Copy code", false) { copy("code", c) }
                    }
                    if (s.optBoolean("paste")) {
                        DField(code, { code = it }, "Paste the code here", Modifier.testTag("account.signin.field"))
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            DBtn("Paste", false) { (ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).primaryClip?.getItemAt(0)?.coerceToText(ctx)?.toString()?.let { code = it } }
                            DBtn("Send code", true, code.isNotBlank(), Modifier.testTag("account.signin.send")) { scope.launch { act("code") { put("code", code.trim()) } } }
                        }
                    }
                    if (s.optString("error").isNotEmpty()) DMono(s.optString("error"), dcs.error, 12, lines = 4)
                }
                DLine()
                x.optJSONArray("actions").objects().forEach { b ->
                    val action = b.optString("action")
                    DBtn(b.optString("label"), action == "signin" && x.optString("state") == "expired", modifier = Modifier.testTag("account.act.$action"), danger = b.optBoolean("danger")) {
                        if (action == "remove") confirm = true else scope.launch { act(action) }
                    }
                }
                if (failure.isNotEmpty()) DMono(failure, dcs.error, 12, Modifier.testTag("account.error"), lines = 4)
            }
        }
    }
    if (confirm) AlertDialog(onDismissRequest = { confirm = false }, text = { Text(a?.optString("remove") ?: "", fontFamily = BpMono, fontSize = 13.sp) },
        confirmButton = { TextButton(onClick = { confirm = false; scope.launch { act("remove") } }) { Text("Remove", color = dcs.error, fontFamily = BpMono) } },
        dismissButton = { TextButton(onClick = { confirm = false }) { Text("Keep it", fontFamily = BpMono) } })
}
