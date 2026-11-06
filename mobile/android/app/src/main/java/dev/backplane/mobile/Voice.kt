package dev.backplane.mobile

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.MicNone
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat

// The composer's mic: the phone records itself.
// The first press asks for the microphone if it must, starts recording and
// sends "voice" with "client"; the next (or the strip's Stop) ends it.
@Composable
fun VoiceMicButton(m: AppModel, v: VoiceView) {
    val ctx = LocalContext.current
    val live = v.on || v.state.isNotEmpty()
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok -> if (ok) m.voicePress(false) }
    IconButton(onClick = {
        if (live) m.voicePress(true)
        else if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) m.voicePress(false)
        else ask.launch(Manifest.permission.RECORD_AUDIO)
    }, modifier = Modifier.testTag("thread.mic")) {
        Icon(if (live) Icons.Filled.Mic else Icons.Filled.MicNone, if (live) v.stop else v.mic,
            tint = if (live) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.outline)
    }
}

// Dictation over the composer, as calm as the web's strip: a dot, one word
// for the state, the level and Stop, never the words (they appear only in
// the draft); a note when something is wrong; after a segment its fixes,
// each a chip that undoes it; the word to clarify, with its choices.
@Composable
fun VoiceBar(m: AppModel, v: VoiceView) {
    val shown = v.on || v.note.isNotEmpty() || v.fixes.isNotEmpty() || v.ask != null
    if (!shown) return
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (v.note.isNotEmpty()) Row(verticalAlignment = Alignment.CenterVertically) {
            Text(v.note, Modifier.weight(1f), style = MaterialTheme.typography.labelMedium, maxLines = 3, overflow = TextOverflow.Ellipsis,
                color = if (v.bad) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.tertiary)
            IconButton(onClick = { m.act("voice-err") }) { Icon(Icons.Filled.Close, null) }
        }
        if (v.on) Row(Modifier.testTag("voice.strip"), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Box(Modifier.size(8.dp).background(MaterialTheme.colorScheme.error, CircleShape))
            Text(v.status, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.outline, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Box(Modifier.width(72.dp).height(4.dp).background(MaterialTheme.colorScheme.surfaceVariant)) {
                Box(Modifier.fillMaxHeight().fillMaxWidth(v.level.coerceIn(0, 100) / 100f).background(MaterialTheme.colorScheme.primary))
            }
            Spacer(Modifier.weight(1f))
            TextButton(onClick = { m.voicePress(true) }, modifier = Modifier.testTag("voice.stop")) { Text(v.stop) }
        }
        if (v.fixes.isNotEmpty()) Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            for (f in v.fixes) OutlinedButton(onClick = { m.act(f.action, f.value) }, modifier = Modifier.semantics { contentDescription = f.tip }) {
                Text(f.label, style = MaterialTheme.typography.labelMedium)
            }
        }
        v.ask?.let { a ->
            Column(Modifier.testTag("voice.ask"), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(a.text, style = MaterialTheme.typography.labelMedium)
                if (a.fixing) a.field?.let { f ->
                    var text by remember(f.name) { mutableStateOf(f.value) }
                    OutlinedTextField(text, { text = it; m.field(f.name, it) }, Modifier.fillMaxWidth().testTag("voice.spelling"),
                        placeholder = { Text(f.hint) }, singleLine = true,
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                        keyboardActions = KeyboardActions(onDone = { m.act("voice-fixed") }))
                }
                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    for (b in a.buttons) {
                        if (b.primary) Button(onClick = { m.act(b.action, b.value) }) { Text(b.label, style = MaterialTheme.typography.labelMedium) }
                        else OutlinedButton(onClick = { m.act(b.action, b.value) }) { Text(b.label, style = MaterialTheme.typography.labelMedium) }
                    }
                }
            }
        }
    }
}
