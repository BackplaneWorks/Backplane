package dev.backplane.mobile

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Folder
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

// Every icon the hub may name (native/host/src/present/badge.rs ICONS) has its own glyph here; any other name is the folder, never a blank.
class BadgeGlyphsTest {
    private fun hubIcons(): List<String> {
        val src = generateSequence(File("").absoluteFile) { it.parentFile }.map { File(it, "native/host/src/present/badge.rs") }.first { it.exists() }.readText()
        val body = src.substringAfter("pub const ICONS: &[&str] = &[").substringBefore("];")
        return body.lines().joinToString(" ") { it.substringBefore("//") }.split(',').map { it.trim().trim('"') }.filter { it.isNotEmpty() }
    }
    @Test fun everyIconTheHubOffersHasAGlyph() {
        val names = hubIcons(); assertEquals(20, names.size)
        for (n in names) assertTrue("$n has no glyph", BADGE_GLYPHS.containsKey(n))
        assertEquals(names.toSet(), BADGE_GLYPHS.keys)
    }
    @Test fun anUnknownNameIsTheFolder() { assertEquals(Icons.Default.Folder, badgeGlyph("skull")); assertEquals(Icons.Default.Folder, badgeGlyph("")) }
}
