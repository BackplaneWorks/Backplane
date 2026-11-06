package dev.backplane.mobile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

// A reply's Markdown for the data/1 app: paragraphs, headings, bullet and numbered lists, fenced code, quotes, and inline code, bold, italics and links.
// (Tables and images come with the next pass; the text of a table row shows as plain text.)
sealed interface MdBlock {
    data class P(val text: String) : MdBlock
    data class H(val level: Int, val text: String) : MdBlock
    data class Li(val marker: String, val text: String) : MdBlock
    data class Code(val text: String) : MdBlock
    data class Quote(val text: String) : MdBlock
    object Rule : MdBlock
}

fun mdBlocks(src: String): List<MdBlock> {
    val out = ArrayList<MdBlock>(); val para = StringBuilder()
    fun flush() { if (para.isNotEmpty()) { out += MdBlock.P(para.toString().trimEnd()); para.setLength(0) } }
    val lines = src.split('\n'); var i = 0
    while (i < lines.size) {
        val l = lines[i]; val t = l.trim()
        when {
            t.startsWith("```") -> { flush(); val b = StringBuilder(); i++; while (i < lines.size && !lines[i].trim().startsWith("```")) { b.append(lines[i]).append('\n'); i++ }; out += MdBlock.Code(b.toString().trimEnd('\n')) }
            t.isEmpty() -> flush()
            Regex("^#{1,6} .*").matches(t) -> { flush(); val n = t.takeWhile { it == '#' }.length; out += MdBlock.H(n, t.drop(n).trim()) }
            Regex("^[-*_]{3,}$").matches(t) -> { flush(); out += MdBlock.Rule }
            Regex("^[-*+] .*").matches(t) -> { flush(); out += MdBlock.Li("•", t.drop(2)) }
            Regex("^\\d+[.)] .*").matches(t) -> { flush(); val m = Regex("^(\\d+[.)]) (.*)").find(t)!!; out += MdBlock.Li(m.groupValues[1], m.groupValues[2]) }
            t.startsWith("> ") -> { flush(); out += MdBlock.Quote(t.drop(2)) }
            else -> { if (para.isNotEmpty()) para.append('\n'); para.append(l) }
        }
        i++
    }
    flush(); return out
}

// inline: `code`, **bold**, *italic* / _italic_, [label](url) (the label)
fun mdInline(s: String, code: androidx.compose.ui.graphics.Color, codeBg: androidx.compose.ui.graphics.Color, link: androidx.compose.ui.graphics.Color): AnnotatedString = buildAnnotatedString {
    var i = 0
    while (i < s.length) {
        val c = s[i]
        fun closing(tok: String): Int = s.indexOf(tok, i + tok.length).let { if (it > i + tok.length - 1 && it >= 0) it else -1 }
        when {
            c == '`' && closing("`") > 0 -> { val e = closing("`"); withStyle(SpanStyle(fontFamily = BpMono, color = code, background = codeBg, fontSize = 13.sp)) { append(s.substring(i + 1, e)) }; i = e + 1 }
            s.startsWith("**", i) && closing("**") > 0 -> { val e = closing("**"); withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(mdInline(s.substring(i + 2, e), code, codeBg, link)) }; i = e + 2 }
            (c == '*' || c == '_') && closing(c.toString()) > i + 1 && (i == 0 || !s[i - 1].isLetterOrDigit()) -> { val e = closing(c.toString()); withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { append(mdInline(s.substring(i + 1, e), code, codeBg, link)) }; i = e + 1 }
            c == '[' && Regex("^\\[([^\\]]+)\\]\\(([^)]+)\\)").find(s.substring(i)) != null -> {
                val m = Regex("^\\[([^\\]]+)\\]\\(([^)]+)\\)").find(s.substring(i))!!
                withStyle(SpanStyle(color = link)) { append(m.groupValues[1]) }; i += m.value.length
            }
            else -> { append(c); i++ }
        }
    }
}

@Composable
fun MdText(src: String, modifier: Modifier = Modifier) {
    val dcs = MaterialTheme.colorScheme
    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        for (b in mdBlocks(src)) when (b) {
            is MdBlock.P -> Text(mdInline(b.text, dcs.onSurface, dcs.surfaceContainerHigh, dcs.primary), color = dcs.onSurface, fontSize = 13.sp, lineHeight = 19.sp, fontFamily = BpMono)
            is MdBlock.H -> Text(mdInline(b.text, dcs.onSurface, dcs.surfaceContainerHigh, dcs.primary), color = dcs.onSurface, fontSize = (20 - 2 * b.level.coerceAtMost(3)).sp, fontWeight = FontWeight.SemiBold, fontFamily = BpMono)
            is MdBlock.Li -> Row { Text(b.marker, Modifier.width(22.dp), color = dcs.onSurfaceVariant, fontSize = 13.sp, fontFamily = BpMono); Text(mdInline(b.text, dcs.onSurface, dcs.surfaceContainerHigh, dcs.primary), color = dcs.onSurface, fontSize = 13.sp, lineHeight = 19.sp, fontFamily = BpMono) }
            is MdBlock.Code -> Text(b.text, Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(8.dp), color = dcs.onSurface, fontFamily = BpMono, fontSize = 12.sp)
            is MdBlock.Quote -> Row(Modifier.height(IntrinsicSize.Min)) { Box(Modifier.width(2.dp).fillMaxHeight().background(dcs.outline)); Spacer(Modifier.width(8.dp)); Text(mdInline(b.text, dcs.onSurface, dcs.surfaceContainerHigh, dcs.primary), color = dcs.onSurfaceVariant, fontSize = 13.sp, fontFamily = BpMono) }
            MdBlock.Rule -> Box(Modifier.fillMaxWidth().height(1.dp).background(dcs.outlineVariant))
        }
    }
}
