package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** A sections project row folds (present/sections.rs `project_row`: `fold`, `folded`, `under`): the chevron shows while something is under it or it is folded. */
class DSecRowFoldTest {
    private fun row(j: String) = DSecRow.of(JSONObject(j))
    @Test fun aRowWithThreadsUnderItHasAChevronAndItsAction() {
        val r = row("""{"id":"h|p1","key":"k1","title":"Backplane","fold":"project-fold","folded":false,"under":4,"running":4,"needsCount":1,"threads":[{"id":"h|t1","title":"a"}]}""")
        assertTrue(r.canFold); assertFalse(r.folded); assertEquals("project-fold", r.fold); assertEquals(1, r.needsCount); assertEquals(4, r.running); assertEquals(1, r.threads.size)
    }
    @Test fun aFoldedRowKeepsItsCountsAndHasNoThreads() {
        val r = row("""{"id":"h|p1","key":"k1","fold":"project-fold","folded":true,"under":4,"running":4,"threads":[]}""")
        assertTrue(r.canFold); assertTrue(r.folded); assertEquals(4, r.running); assertTrue(r.threads.isEmpty())
    }
    @Test fun nothingUnderAnOpenRowOrAnOlderHubMeansNoChevron() {
        assertFalse(row("""{"id":"h|p1","key":"k1","fold":"project-fold","folded":false,"under":0}""").canFold)
        assertFalse(row("""{"id":"h|p1","key":"k1","under":3}""").canFold)
    }
}
