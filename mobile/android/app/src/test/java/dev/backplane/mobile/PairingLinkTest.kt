package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

// The hub's address and its short code as the link the phone pairs with: the "=" of code= stays as it is (Uri.Builder's fragment()
// wrote %3D, and the invitation then had no code)
class PairingLinkTest {
    @Test fun aCodeLinkKeepsItsEquals() {
        assertEquals("http://100.64.0.7:3787/pair#code=ABCD2345", Pairing.codeLink("100.64.0.7:3787", "abcd-2345"))
        assertEquals("http://192.168.1.20:3787/pair#code=ABCD2345", Pairing.codeLink(" 192.168.1.20:3787 ", "ABCD 2345"))
        assertEquals("https://hub.example.ts.net/pair#code=ABCD2345", Pairing.codeLink("https://hub.example.ts.net/settings?x=1#y", "ABCD2345"))
        assertEquals("http://[fd7a::1]:3787/pair#code=ABCD2345", Pairing.codeLink("[fd7a::1]:3787", "ABCD2345"))
    }

    @Test fun aCodeLinkNeedsAHubAndACode() {
        assertNull(Pairing.codeLink("100.64.0.7:3787", "ABCD-234"))
        assertNull(Pairing.codeLink("100.64.0.7:3787", "ABCD-2341"))   // 1 is not in the code's alphabet
        assertNull(Pairing.codeLink("", "ABCD2345"))
        assertNull(Pairing.codeLink(":3787", "ABCD2345"))
        assertNull(Pairing.codeLink("ftp://100.64.0.7", "ABCD2345"))
        assertNull(Pairing.codeLink("http://me@100.64.0.7:3787", "ABCD2345"))
    }

    // the routes of an invitation come through untouched by the code link's rules
    @Test fun anInvitationsRoutes() {
        val inv = "http://100.64.0.7:3787/pair#invite=0123456789abcdef0123456789abcdef&routes=%5B%22http%3A%2F%2F192.168.1.20%3A3787%22%5D"
        assertEquals(listOf("http://100.64.0.7:3787", "http://192.168.1.20:3787"), HubList.routes(inv))
    }
}
