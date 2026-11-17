package dev.backplane.mobile

import org.junit.Assert.*
import org.junit.Test

// Copyable's pure part: the port of native/host/src/md/copy.rs (its tests, case for case), what gets a button when the hub says and when it does not,
// and Run in terminal's bytes and wait (the desktop's screens/term.rs tests).
class CopyCoreTest {
    @Test fun a_plain_posts_copyable_pieces() {
        val v = CopyRules.spans("Run `sudo apt-get install -y rootlesskit slirp4netns` then see `foo()` and:\n```bash\nmake\nmake install\n```\nhash `3552c7cfd`.")
        assertEquals(listOf(CopyPiece("sudo apt-get install -y rootlesskit slirp4netns", true), CopyPiece("make\nmake install", true), CopyPiece("3552c7cfd", false)), v)
        assertTrue(CopyRules.spans("no code here").isEmpty())
        assertTrue(CopyRules.spans("an `unclosed span").isEmpty())
    }

    @Test fun fenced_blocks_that_hold_commands() {
        val b = CopyRules::blockIsCommand
        assertTrue(b("sudo apt-get install -y rootlesskit slirp4netns", ""))
        assertTrue(b("cd native/host\ncargo test --offline", "text"))
        assertTrue(b("# set up\nexport A=1\n\nscripts/land.sh", ""))
        assertTrue(b("docker run \\\n  --rm \\\n  img", ""))
        assertTrue(b("anything at all", "bash"))
        assertTrue(b("Get-ChildItem", "powershell"))
        assertTrue(b("$ git status", ""))
        assertTrue(b("RUST_LOG=debug ./target/fast/backplane-hub --foreground", ""))
        assertFalse(b("", "sh"))
        assertFalse(b("fn main() {}", "rust"))
        assertFalse(b("git status", "rust"))
        assertFalse(b("{\"a\": 1}", ""))
        assertFalse(b("git status\nOn branch main", ""))
        assertFalse(b("Install the tools first.", ""))
        assertFalse(b("# only a comment", ""))
    }

    @Test fun inline_spans_by_kind() {
        val k = CopyRules::inlineKind
        assertEquals("cmd", k("sudo apt-get install -y rootlesskit slirp4netns"))
        assertEquals("cmd", k("git status"))
        assertEquals("cmd", k("scripts/land.sh --dry"))
        assertNull(k("git"))
        assertEquals("url", k("https://example.com/a"))
        assertEquals("path", k("~/.backplane/home"))
        assertEquals("path", k("/usr/bin/env"))
        assertEquals("path", k("native/host/src/md/copy.rs"))
        assertEquals("path", k("C:\\Users\\h"))
        assertEquals("hash", k("3552c7cfd"))
        assertEquals("hash", k("8e41de6a2c1f0a9b8e41de6a2c1f0a9b8e41de6a"))
        assertEquals("id", k("t1794855320-614"))
        assertEquals("id", k("0.16.1"))
        assertNull(k("Copy")); assertNull(k("foo()")); assertNull(k("a b")); assertNull(k("deadbeef")); assertNull(k(""))
    }

    @Test fun the_hubs_answer_wins_and_the_rules_serve_an_older_hub() {
        val hub = CopyLookup.of(listOf(CopyPiece("git status", true), CopyPiece("make\nmake install", true)), hubHasCopy = true)
        assertEquals(CopyPiece("git status", true), hub.inline("git status"))
        assertNull("the hub did not name it: no button, whatever the rules say", hub.inline("3552c7cfd"))
        assertTrue(hub.blockCmd("make\nmake install\n", ""))
        assertFalse("every block copies; Run only when the hub says it is a command", hub.blockCmd("cargo test", "sh"))
        // a reply with no `copy` from a hub that sends it: nothing inline; one still arriving (live) has no answer yet, so the rules
        assertNull(CopyLookup.of(null, hubHasCopy = true).inline("git status"))
        assertEquals(CopyPiece("git status", true), CopyLookup.of(null, hubHasCopy = true, live = true).inline("git status"))
        // a hub without the feature: the rules
        val old = CopyLookup.of(null, hubHasCopy = false)
        assertEquals(CopyPiece("3552c7cfd", false), old.inline("3552c7cfd")); assertNull(old.inline("foo()"))
        assertTrue(old.blockCmd("cargo test", "sh")); assertFalse(old.blockCmd("fn main() {}", "rust"))
        assertEquals(listOf(CopyPiece("git status", true)), old.pieces("run `git status` now"))
        assertNotEquals(hub.key, old.key)
    }

    @Test fun parse_copy_says_absent_and_empty_apart() {
        assertNull(parseCopy(null))
        assertEquals(emptyList<CopyPiece>(), parseCopy(org.json.JSONArray()))
        val a = org.json.JSONArray("""[{"text":"git status","cmd":true},{"text":"/a/b"}]""")
        assertEquals(listOf(CopyPiece("git status", true), CopyPiece("/a/b", false)), parseCopy(a))
        assertEquals(parseCopy(a), parseCopy(copyJson(parseCopy(a)!!)))
    }

    @Test fun a_copy_is_the_text_with_no_line_break_after_it() {
        assertEquals("sudo apt-get install -y x", Copy.exact("sudo apt-get install -y x\n"))
        assertEquals("a\nb", Copy.exact("a\nb\n\n"))
        assertEquals("  keep spaces  ", Copy.exact("  keep spaces  "))
        assertEquals("line", Copy.exact("line\r\n"))
        assertEquals("make …", Copy.shown("make\nmake install\n"))
        assertEquals("git status", Copy.shown("git status\n"))
    }

    @Test fun run_in_terminal_types_the_command_and_never_enter() {
        val plain = Vt(80, 24)
        assertArrayEquals("sudo apt-get install -y x".toByteArray(), RunInTerminal.prefillBytes("sudo apt-get install -y x\n", plain))
        // no bracketed paste: one line, so nothing runs before the person's Enter
        assertArrayEquals("cd a    && make && make install".toByteArray(), RunInTerminal.prefillBytes("cd a \\\n  && make\n# done\nmake install", plain))
        val b = Vt(80, 24); b.feed("\u001b[?2004h$ ".toByteArray())
        val got = RunInTerminal.prefillBytes("make\nmake install", b)
        assertArrayEquals("\u001b[200~make\rmake install\u001b[201~".toByteArray(), got)
        assertNotEquals('\r'.code.toByte(), got.last())
        assertFalse(RunInTerminal.prefillBytes("a\u001bb\nc", plain).contains(0x1b))
    }

    @Test fun run_in_terminal_waits_for_the_prompt_and_not_into_a_full_screen_program() {
        val v = Vt(80, 24)
        assertEquals(false, RunInTerminal.ready(v, 0))
        assertEquals(true, RunInTerminal.ready(v, RunInTerminal.WAIT_MS * 4))
        v.feed("$ ".toByteArray()); assertEquals(false, RunInTerminal.ready(v, 0)); assertEquals(true, RunInTerminal.ready(v, RunInTerminal.WAIT_MS))
        v.feed("\u001b[?2004h".toByteArray()); assertEquals(true, RunInTerminal.ready(v, 0))
        v.feed("\u001b[?1049h".toByteArray()); assertNull(RunInTerminal.ready(v, RunInTerminal.WAIT_MS))
    }

    @Test fun the_terminal_belongs_to_the_chat_on_screen() {
        assertEquals("t1", RunInTerminal.threadKey("t1"))
        assertEquals("t1", RunInTerminal.threadKey("h1|t1"))
        assertEquals("pcf8636e06a1f3032~t9", RunInTerminal.threadKey("pcf8636e06a1f3032~t9"))
        assertEquals("room:r1", RunInTerminal.roomKey("r1"))
        assertEquals("", RunInTerminal.roomKey(""))
    }
}
