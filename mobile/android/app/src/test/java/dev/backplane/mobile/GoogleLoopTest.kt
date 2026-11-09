package dev.backplane.mobile

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class GoogleLoopTest {
    private val r = GoogleLoop.redirect(41234)
    @Test fun redirectIsLoopback() = assertEquals("http://127.0.0.1:41234/oauth/google", r)

    @Test fun opensOnlyItsOwnRedirect() {
        val enc = "http%3A%2F%2F127.0.0.1%3A41234%2Foauth%2Fgoogle"
        assertTrue(GoogleLoop.matches("https://accounts.google.com/o/oauth2/v2/auth?client_id=x&redirect_uri=$enc&state=s", r))
        assertTrue(GoogleLoop.matches("https://accounts.google.com/a?redirect_uri=$enc", r))
        assertFalse(GoogleLoop.matches("https://accounts.google.com/a?redirect_uri=http%3A%2F%2F127.0.0.1%3A41235%2Foauth%2Fgoogle", r))
        assertFalse(GoogleLoop.matches("https://accounts.google.com/a?redirect_uri=$enc%2Fx", r))
        assertFalse(GoogleLoop.matches("https://accounts.google.com/a?x=redirect_uri%3D$enc", r))
        assertFalse(GoogleLoop.matches("https://accounts.google.com/a", r))
        assertFalse(GoogleLoop.matches("", r))
    }

    @Test fun requestLine() {
        assertEquals("/oauth/google?code=4%2Fabc&state=s", GoogleLoop.requestPath("GET /oauth/google?code=4%2Fabc&state=s HTTP/1.1\r\n"))
        assertNull(GoogleLoop.requestPath("POST /oauth/google?x=1 HTTP/1.1"))
        assertNull(GoogleLoop.requestPath(""))
        assertNull(GoogleLoop.requestPath("GET"))
    }

    @Test fun onlyTheCatchEndsASignIn() {
        assertTrue(GoogleLoop.isCatch("/oauth/google?code=x&state=y"))
        assertFalse(GoogleLoop.isCatch("/oauth/google"))
        assertFalse(GoogleLoop.isCatch("/oauth/google?"))
        assertFalse(GoogleLoop.isCatch("/favicon.ico"))
        assertFalse(GoogleLoop.isCatch("/x/oauth/google?code=1"))
        assertFalse(GoogleLoop.isCatch(null))
    }

    @Test fun whichAccountsLoopBack() {
        assertTrue(GoogleLoop.wants("google:new", false))
        assertTrue(GoogleLoop.wants("google:ab12", false))
        assertTrue(GoogleLoop.wants("x:1", true))
        assertFalse(GoogleLoop.wants("claude:work", false))
    }
}
