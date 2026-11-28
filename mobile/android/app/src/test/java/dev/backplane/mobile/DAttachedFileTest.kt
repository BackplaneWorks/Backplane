package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Test

// Choose a file: the words and the `[attachment: <path>]` lines sent (the desktop's way).
class DAttachedFileTest {
    private val pdf = DAttachedFile("data sheet.pdf", "/h/attachments/t1/k1-data_sheet.pdf")
    private val csv = DAttachedFile("bom.csv", "/h/attachments/t1/k2-bom.csv")
    @Test fun wordsThenOneLinePerFile() {
        assertEquals("read this\n\n[attachment: /h/attachments/t1/k1-data_sheet.pdf]\n[attachment: /h/attachments/t1/k2-bom.csv]", DAttachedFile.join("read this  ", listOf(pdf, csv)))
    }
    @Test fun aFileAloneIsJustItsLine() { assertEquals("[attachment: /h/attachments/t1/k2-bom.csv]", DAttachedFile.join("  ", listOf(csv))) }
    @Test fun noFilesLeavesTheText() { assertEquals("hi ", DAttachedFile.join("hi ", emptyList())) }
}
