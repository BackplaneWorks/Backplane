package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Test

// A sheet found by a search is listed under its parent path, without the top sheet every row is under (the desktop's sheets.rs `path`).
class SheetPathTest {
    private fun s(depth: Int, name: String) = DataClient.Sheet(depth, false, name, "/p/$name.kicad_sch")

    @Test fun aMatchIsListedUnderItsParentPath() {
        val sheets = listOf(s(0, "root"), s(1, "Power"), s(2, "Regulators"), s(3, "LDO"), s(2, "Battery"), s(1, "MCU"))
        assertEquals(listOf("", "", "Power", "Power / Regulators", "Power", ""), sheets.indices.map { sheetPath(sheets, it) })
        assertEquals("", sheetPath(sheets, 9))
    }
}
