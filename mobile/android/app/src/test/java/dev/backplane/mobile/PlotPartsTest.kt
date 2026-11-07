package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.nio.ByteBuffer
import java.nio.ByteOrder

class PlotPartsTest {
    // one triangle (0,0,0) (1,0,0) (0,1,0), colour 5, as the hub writes a mesh
    private val tri = byteArrayOf(5, 0, 0, 0, 2, 0, 0, 1, 2, 0)
    private fun at(vararg m: FloatArray): ByteArray {
        val b = ByteBuffer.allocate(48 * m.size).order(ByteOrder.LITTLE_ENDIAN)
        for (a in m) for (x in a) b.putFloat(x)
        return b.array()
    }
    // x' = 2x + tx, y' = y + ty, z' = z + tz
    private fun place(tx: Float, ty: Float, tz: Float) = floatArrayOf(2f, 0f, 0f, tx, 0f, 1f, 0f, ty, 0f, 0f, 1f, tz)

    @Test fun anInstanceIsPlacedByItsAffineBeforeTheScale() {
        val m = PlotMesh.decodePart(1, tri, at(place(100f, 200f, 300f)))
        assertEquals(3, m.colors.size)
        // the integers are placed, then 10 µm each
        assertEquals(listOf(1000f, 2000f, 3000f), m.verts.slice(0..2))
        assertEquals(listOf(1020f, 2000f, 3000f), m.verts.slice(6..8))
        assertEquals(listOf(1000f, 2010f, 3000f), m.verts.slice(12..14))
        assertEquals(listOf(5, 5, 5), m.colors.toList())
        // the normal is the placed triangle's (+z)
        assertEquals(1f, m.verts[17], 1e-4f)
    }

    @Test fun everyInstanceOfAPartComesTogether() {
        val m = PlotMesh.decodePart(1, tri, at(place(0f, 0f, 0f), place(10f, 0f, 0f)))
        assertEquals(6, m.colors.size)
        assertEquals(100f, m.verts[18], 1e-3f)
    }

    @Test fun badBytesAreRefused() {
        for ((n, b, a) in listOf(Triple(0, tri, at(place(0f, 0f, 0f))), Triple(1, tri, ByteArray(47)), Triple(1, tri, ByteArray(0)),
            Triple(1, tri.copyOf(9), at(place(0f, 0f, 0f))), Triple(1, tri + byteArrayOf(0), at(place(0f, 0f, 0f))),
            Triple(1, tri, at(place(Float.NaN, 0f, 0f))))) {
            try { PlotMesh.decodePart(n, b, a); fail("accepted n=$n") } catch (_: IllegalArgumentException) {}
        }
    }

    @Test fun aGroupFadesInOver300Ms() {
        assertEquals(0f, PlotMesh.fadeAt(0f), 0f)
        assertEquals(0.5f, PlotMesh.fadeAt(150f), 1e-6f)
        assertEquals(1f, PlotMesh.fadeAt(300f), 0f)
        assertEquals(1f, PlotMesh.fadeAt(5000f), 0f)
        assertEquals(0f, PlotMesh.fadeAt(-10f), 0f)
    }

    @Test fun progressShowsWhileParts_Stream() {
        assertEquals("parts 3/10", PlotMesh.progress(3, 10))
        assertEquals("", PlotMesh.progress(10, 10))
        assertEquals("", PlotMesh.progress(0, 0))
    }

    @Test fun aPartFramesCborDecodesToTheFieldsTheStoreReads() {
        // {1:"plot", 31:key, "part":id, "n":1, "mesh":bytes, "at":bytes, "refs":["R1"], "done":1}
        val out = java.io.ByteArrayOutputStream()
        fun text(s: String) { val b = s.toByteArray(); out.write(0x60 + b.size); out.write(b) }
        fun bytes(b: ByteArray) { out.write(0x40 + b.size); out.write(b) }
        val a = at(place(0f, 0f, 0f))
        out.write(0xA8); out.write(1); text("plot"); out.write(0x18); out.write(31); text("2|/b|")
        text("part"); text("0123456789abcdef"); text("n"); out.write(1); text("mesh"); bytes(tri)
        text("at"); out.write(0x58); out.write(a.size); out.write(a)
        text("refs"); out.write(0x81); text("R1"); text("done"); out.write(1)
        @Suppress("UNCHECKED_CAST") val o = Cbor.decode(out.toByteArray()) as Map<String, Any?>
        assertEquals("2|/b|", o["key"])
        assertTrue(PlotStore.isPlot(out.toByteArray()))
        val m = PlotMesh.decodePart((o["n"] as Long).toInt(), o["mesh"] as ByteArray, o["at"] as ByteArray)
        assertEquals(3, m.colors.size)
    }
}
