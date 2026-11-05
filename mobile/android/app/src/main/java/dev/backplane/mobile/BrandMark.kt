package dev.backplane.mobile

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp

// The brand mark of the web client as it shows at phone width (style.css .topbar .brand-mark: 18 px; the desktop sidebar's is 24), the same art in the theme's colours
// (dark: favicon.svg / apple-touch-icon art on #111410; light: favicon-light.svg, white with a black mark,
// commit 80cc30e499), `contain`, vector, with the connection square (.conn: 7 px, ok green or bad red,
// ringed by the panel colour, right -2 px bottom -1 px).
@Composable
fun BrandMark(online: Boolean, modifier: Modifier = Modifier) {
    val light = MaterialTheme.colorScheme.background.luminance() > 0.5f
    val ring = if (light) Color(0xFFF1F0EC) else Color(0xFF13161B)
    val ok = if (light) Color(0xFF1C8550) else Color(0xFF58C28A)
    val bad = if (light) Color(0xFFC93434) else Color(0xFFF06A6A)
    Box(modifier.size(18.dp)) {
        Image(painterResource(if (light) R.drawable.brand_light else R.drawable.brand_dark), "Backplane",
            Modifier.size(18.dp), contentScale = ContentScale.Fit)
        // .conn is 7 px with a 1 px border (a 5 px square inside) and a 1 px ring outside: 9 px of panel colour,
        // its right edge 3 px beyond the mark and its bottom 2 px (right: -2px; bottom: -1px)
        Box(Modifier.align(androidx.compose.ui.Alignment.BottomEnd).offset(3.dp, 2.dp).size(9.dp).background(ring)) {
            Box(Modifier.align(androidx.compose.ui.Alignment.Center).size(5.dp).background(if (online) ok else bad))
        }
    }
}
