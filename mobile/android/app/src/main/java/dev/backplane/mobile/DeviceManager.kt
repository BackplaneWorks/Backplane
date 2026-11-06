package dev.backplane.mobile

import android.content.Intent
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.net.Uri
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody
import org.json.JSONObject
import java.util.Date
import java.util.concurrent.TimeUnit

@OptIn(ExperimentalMaterial3Api::class, androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun DeviceManager(link: String, close: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var devices by remember { mutableStateOf<List<JSONObject>>(emptyList()) }
    var invitations by remember { mutableStateOf<List<JSONObject>>(emptyList()) }
    var invitation by remember { mutableStateOf<JSONObject?>(null) }
    var status by remember { mutableStateOf("") }
    var revoke by remember { mutableStateOf<JSONObject?>(null) }
    var uninvite by remember { mutableStateOf<JSONObject?>(null) }
    var now by remember { mutableStateOf(System.currentTimeMillis()) }
    val client = remember { OkHttpClient.Builder().followRedirects(false).followSslRedirects(false).callTimeout(8, TimeUnit.SECONDS).build() }
    suspend fun request(path: String, method: String = "GET", id: String? = null, invitationId: String? = null): JSONObject = withContext(Dispatchers.IO) {
        val url = Pairing.http(link, path, token = false) ?: throw IllegalStateException()
        val token = Uri.parse(Pairing.socket(link)).getQueryParameter("token") ?: throw IllegalStateException()
        val b = Request.Builder().url(url).header("Authorization", "Bearer $token").header("X-Backplane-Client", "native")
            .header("X-Backplane-Scheme", Uri.parse(url).scheme ?: "http")
        if (method == "POST") b.post(RequestBody.create(null, ByteArray(0)))
        if (id != null) b.header("X-Backplane-Device", id)
        if (invitationId != null) b.header("X-Backplane-Invitation", invitationId)
        client.newCall(b.build()).execute().use { r ->
            if (!r.isSuccessful) throw IllegalStateException()
            JSONObject(r.body?.string() ?: "{}")
        }
    }
    suspend fun refresh() {
        try { val r = request("/auth/devices");val a = r.getJSONArray("devices")
            devices = (0 until a.length()).map { a.getJSONObject(it) }.filter { !it.optBoolean("revoked") }
            val i = r.optJSONArray("invitations")
            invitations = if (i == null) emptyList() else (0 until i.length()).map { i.getJSONObject(it) }
        } catch (_: Exception) { status = "Could not load devices. Check your connection and pairing." }
    }
    fun copy(text: String) {
        (ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("Backplane invitation", text))
        status = "Copied"
    }
    BackHandler { close() }
    LaunchedEffect(link) { refresh() }
    LaunchedEffect(invitation) { while (invitation != null) { now = System.currentTimeMillis();kotlinx.coroutines.delay(1000) } }
    Scaffold(topBar = { TopAppBar(title = { Text("Devices") }, navigationIcon = { TextButton(onClick = close) { Text("Back") } }) }) { pad ->
        Column(Modifier.padding(pad).padding(20.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Text("Add another computer or phone. Invitations work once and expire in 10 minutes.")
            if (status.isNotEmpty()) Text(status)
            Button(onClick = { scope.launch {
                status = "Creating invitation…"
                try { invitation = request("/auth/invitations", "POST");status = "Invitation ready. Share only with the device you want to authorize." }
                catch (_: Exception) { status = "Could not create invitation. Check the hub connection." }
            } }) { Text("Add a device") }
            invitation?.let { invite ->
                if (now < invite.optLong("expiresAt")) {
                    val addresses = remember(invite) { inviteAddresses(invite) }
                    var pick by remember(invite) { mutableIntStateOf(0) }
                    val at = addresses[pick.coerceIn(0, addresses.size - 1)]
                    Text(invite.optString("code"), style = MaterialTheme.typography.headlineLarge)
                    // which of the hub's addresses the code and link name
                    // (the reply's addresses; a hub without them sends one)
                    if (addresses.size > 1) FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        addresses.forEachIndexed { i, a ->
                            FilterChip(selected = i == pick, onClick = { pick = i }, label = { Text(addressLabel(a, addresses)) })
                        }
                    }
                    Text("Hub address: " + at.url)
                    if (!at.listening) Text("Not reachable yet: the hub does not answer at this address.", color = MaterialTheme.colorScheme.error)
                    Text("Expires in " + ((invite.optLong("expiresAt") - now + 59999) / 60000) + " minutes")
                    QrCode(at.qr.split('\n').filter { it.isNotEmpty() })
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = { copy(at.link) }) { Text("Copy link") }
                        Button(onClick = { copy(invite.optString("code")) }) { Text("Copy code") }
                        Button(onClick = {
                            val share = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, at.link)
                            ctx.startActivity(Intent.createChooser(share, "Connect to Backplane"))
                        }) { Text("Share") }
                    }
                } else Text("Expired — create a new invitation.")
            }
            Text("Paired devices", style = MaterialTheme.typography.titleLarge)
            if (devices.isEmpty()) Text("Existing pairings are preserved. New devices appear here after connecting.")
            devices.forEach { d ->
                Text(d.optString("name"), style = MaterialTheme.typography.titleMedium)
                Text("Last seen " + Date(d.optLong("lastSeen")).toString())
                TextButton(onClick = { revoke = d }) { Text("Revoke") }
                HorizontalDivider()
            }
            // every pairing link this hub issued: when, used or not and by
            // which device; revoking a used one revokes that device too
            Text("Pairing links", style = MaterialTheme.typography.titleLarge)
            if (invitations.isEmpty()) Text("Links made with Add a device, and invites made in Bots to link a machine, appear here.")
            invitations.forEach { x ->
                Text((if (x.optString("kind") == "machine") "Machine invite · " else "Device invitation · ") + Date(x.optLong("createdAt")).toString(), style = MaterialTheme.typography.titleMedium)
                Text(invitationState(x))
                if (x.optString("state") == "open" || (x.optString("state") == "used" && x.optString("device").isNotEmpty() && !x.optBoolean("deviceRevoked")))
                    TextButton(onClick = { uninvite = x }) { Text("Revoke") }
                HorizontalDivider()
            }
        }
    }
    if (uninvite != null) AlertDialog(onDismissRequest = { uninvite = null }, title = { Text("Revoke this link?") },
        text = { Text(if (uninvite?.optString("state") == "used") (if (uninvite?.optString("kind") == "machine") "The machine it linked (" else "The device it paired (") + (uninvite?.optString("deviceName")?.ifEmpty { "unnamed" } ?: "unnamed") + (if (uninvite?.optString("kind") == "machine") ") is unlinked too." else ") loses access too.") else "It will no longer pair anything.") },
        confirmButton = { TextButton(onClick = { val id = uninvite?.optString("id");uninvite = null;scope.launch {
            try { request("/auth/revoke", "POST", invitationId = id);refresh() } catch (_: Exception) { status = "Could not revoke this link." }
        } }) { Text("Revoke") } }, dismissButton = { TextButton(onClick = { uninvite = null }) { Text("Cancel") } })
    if (revoke != null) AlertDialog(onDismissRequest = { revoke = null }, title = { Text("Revoke access?") }, text = { Text("This device will need a new invitation to reconnect.") },
        confirmButton = { TextButton(onClick = { val id = revoke?.optString("id");revoke = null;scope.launch {
            try { request("/auth/revoke", "POST", id);refresh() } catch (_: Exception) { status = "Could not revoke this device." }
        } }) { Text("Revoke") } }, dismissButton = { TextButton(onClick = { revoke = null }) { Text("Cancel") } })
}

// One of the hub's addresses an invitation can name (/auth/invitations'
// addresses: tailnet, each local network address, this computer, in that
// order), with its own link and code; listening false: the hub does not
// answer there yet (the computer's Settings turns it on)
data class InviteAddress(val kind: String, val url: String, val link: String, val qr: String, val listening: Boolean)

// the invitation's addresses; an older hub's reply has only the first one's
// link, code and route
fun inviteAddresses(invite: JSONObject): List<InviteAddress> {
    val a = invite.optJSONArray("addresses")
    val got = if (a == null) emptyList() else (0 until a.length()).mapNotNull { a.optJSONObject(it) }
        .filter { it.optString("link").isNotEmpty() && it.optString("qr").isNotEmpty() }
        .map { InviteAddress(it.optString("kind"), it.optString("url"), it.optString("link"), it.optString("qr"), it.optBoolean("listening", true)) }
    return got.ifEmpty { listOf(InviteAddress("", invite.optJSONArray("routes")?.optString(0) ?: "", invite.optString("link"), invite.optString("qr"), true)) }
}

// a chip's name: the kind, and the host where one kind has several (two
// local networks)
fun addressLabel(a: InviteAddress, all: List<InviteAddress>): String {
    val name = when (a.kind) { "tailnet" -> "Tailscale"; "lan" -> "Local network"; "local" -> "This computer"; else -> Uri.parse(a.url).host ?: a.url }
    return if (all.count { it.kind == a.kind } > 1) name + " " + (Uri.parse(a.url).host ?: a.url) else name
}

// what became of a pairing link (the hub's /auth/devices "invitations")
fun invitationState(x: JSONObject): String = when (x.optString("state")) {
    "used" -> "Used " + Date(x.optLong("usedAt")).toString() + " by " + x.optString("deviceName").ifEmpty { if (x.optString("kind") == "machine") "a machine" else "a device" } +
        (if (x.optBoolean("deviceRevoked")) (if (x.optString("kind") == "machine") " (unlinked)" else " (revoked)") else "")
    "revoked" -> if (x.optString("device").isNotEmpty()) "Revoked with " + x.optString("deviceName").ifEmpty { if (x.optString("kind") == "machine") "the machine it linked" else "the device it paired" } else "Revoked, never used"
    "expired" -> "Expired, never used"
    else -> "Not used yet · expires " + Date(x.optLong("expiresAt")).toString()
}
