package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class PlotMeshTest {
    // the hub's plot::mesh_id: FNV-1a 64 of the wire bytes; the same values as its test
    @Test fun aModelsIdIsTheHubs() {
        assertEquals("cbf29ce484222325", PlotMesh.id(ByteArray(0)))
        assertEquals("af63dc4c8601ec8c", PlotMesh.id("a".toByteArray()))
    }

    // one triangle as the hub writes it: colour, then nine zigzag deltas
    private fun one(): Map<String, Any?> = mapOf("key" to "2|/r|", "n" to 1L, "box3" to listOf(0L, 0L, 0L, 1L, 1L, 0L), "mesh" to byteArrayOf(5, 0, 0, 0, 2, 0, 0, 1, 2, 0))

    @Test fun aCachedModelReadsBackAsTheWireDid() {
        val dir = kotlin.io.path.createTempDirectory("meshdisk").toFile()
        MeshDisk.dir = dir
        val o = one()
        assertNull(MeshDisk.load("hub|t1|3d|"))
        MeshDisk.save("hub|t1|3d|", "2|/r|", 1, listOf(0L, 0L, 0L, 1L, 1L, 0L), o["mesh"] as ByteArray)
        val back = MeshDisk.load("hub|t1|3d|")
        assertNotNull(back)
        assertEquals("2|/r|", back!!["key"])
        val a = PlotMesh.decode(o); val b = PlotMesh.decode(back)
        assertEquals(a.colors.toList(), b.colors.toList()); assertEquals(a.verts.toList(), b.verts.toList())
        assertNull("another view has its own", MeshDisk.load("hub|t2|3d|"))
        for (i in 0 until 12) MeshDisk.save("hub|t$i|3d|", "k", 1, emptyList(), byteArrayOf(1))
        assertEquals("the newest few are kept", 8, dir.listFiles { f -> f.name.endsWith(".bin") }!!.size)
        dir.deleteRecursively()
    }
}
