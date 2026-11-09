package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Test

// The viewer's tap card and cross-probe words from the hub's piece info (`kind;net=…;ref=…`, kicad/plotgen.rs).
class ProbeTest {
    @Test fun aPadsPartAndNetAreItsFields() {
        val i = "pad 1;net=GND;ref=R1;layer=F.Cu"
        assertEquals("R1", Probe.field(i, "ref")); assertEquals("GND", Probe.field(i, "net")); assertEquals("", Probe.field(i, "width"))
        assertEquals("pad 1 · R1" to "net GND · layer F.Cu", Probe.lines(i))
    }
    @Test fun aTrackHasANetAndNoPartAndSeveralPiecesReadTheFirst() {
        assertEquals("track" to "net VCC · width 0.25", Probe.lines("track;net=VCC;width=0.25\u0000pad 2;net=GND;ref=R2"))
        assertEquals("", Probe.field("track;net=VCC\u0000pad 2;ref=R2", "ref"))
        assertEquals("piece 3,4" to "", Probe.lines("piece 3,4"))
    }
    @Test fun aSchematicPinsPartIsItsSymbolAndTheNetGroupIsNotShown() {
        val i = "pin 1;name=DEC1;type=passive;symbol=U1;g=869301529"
        assertEquals("U1", Probe.part(i)); assertEquals("pin 1 · U1" to "name DEC1 · type passive", Probe.lines(i))
    }
}
