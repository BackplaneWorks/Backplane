package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

// Clone from git (docs/CLONE.md): the hub's answers as the screen reads them, what it sends, and where a finished clone leads.
class CloneCoreTest {
    private fun job(extra: String) = CloneJob.of(JSONObject("""{"id":"c1-1","repo":"o/kit","origin":"github.com/o/kit","dest":"/home/user/Documents/kit","destShown":"~/Documents/kit",
        "name":"kit","phase":"Receiving objects","detail":"1.20 MiB, 600.00 KiB/s","error":"","auth":false,"project":"","thread":"",$extra}"""))!!

    @Test fun a_job_reads_its_progress_and_a_null_pct_is_an_indeterminate_bar() {
        val j = job(""""state":"running","pct":45,"signin":false,"notes":[]""")
        assertTrue(j.running); assertEquals(45, j.pct); assertEquals("~/Documents/kit", j.destShown)
        assertNull(job(""""state":"running","pct":null,"signin":false,"notes":[]""").pct)
        assertNull("no id: no job", CloneJob.of(JSONObject("{}")))
        assertTrue(job(""""state":"failed","pct":null,"signin":true,"notes":[]""").let { it.stopped && it.signin })
    }
    @Test fun done_with_no_notes_goes_on_at_once_to_its_thread_else_home() {
        assertEquals(CloneAfter.Thread("t9"), cloneAfter(job(""""state":"done","pct":100,"signin":false,"notes":[]""").copy(thread = "t9")))
        assertEquals(CloneAfter.Home, cloneAfter(job(""""state":"done","pct":100,"signin":false,"notes":[]""")))
        val noted = job(""""state":"done","pct":100,"signin":false,"notes":["Found a KiCad project: kit.kicad_pro."]""").copy(thread = "t9")
        assertEquals("notes are read first", CloneAfter.Stay, cloneAfter(noted)); assertEquals(CloneAfter.Thread("t9"), cloneLeads(noted))
        assertEquals(CloneAfter.Stay, cloneAfter(job(""""state":"running","pct":3,"signin":false,"notes":[]""")))
        assertEquals(CloneAfter.Stay, cloneAfter(job(""""state":"failed","pct":null,"signin":false,"notes":[]""")))
    }
    @Test fun repos_carry_rows_and_the_signed_out_hint() {
        val r = CloneRepos.of(JSONObject("""{"repos":[{"name":"o/kit","value":"o/kit","description":"A board","private":true,"account":"o"},{"name":"o/x","value":""}],
            "more":3,"state":"ok","loading":false,"hint":""}"""))
        assertEquals(listOf("o/kit", "o/x"), r.repos.map { it.value }); assertTrue(r.repos[0].private); assertFalse(r.showsHint); assertEquals(3, r.more)
        val s = CloneRepos.of(JSONObject("""{"repos":[],"more":0,"state":"signedout","loading":false,"hint":"Sign in to GitHub to see your repositories here."}"""))
        assertTrue(s.offersSignin && s.showsHint)
        val n = CloneRepos.of(JSONObject("""{"repos":[],"state":"nogh","hint":"Install the GitHub CLI (gh) on this machine to see your GitHub repositories here."}"""))
        assertFalse(n.offersSignin); assertTrue(n.showsHint)
    }
    @Test fun dest_params_send_only_what_the_person_changed_and_start_takes_the_agreed_folder() {
        assertEquals("""{"repo":"o/kit"}""", cloneDestParams("o/kit", "", null).toString())
        val p = cloneDestParams("o/kit", "/home/user/src", " board ")
        assertEquals("/home/user/src", p.getString("parent")); assertEquals("board", p.getString("name"))
        val d = CloneDest.of(JSONObject("""{"ok":true,"repo":"o/kit","name":"kit","dest":"/home/user/Documents/kit-2","destShown":"~/Documents/kit-2","parent":"/home/user/Documents","folder":"kit-2"}"""))
        assertTrue(d.known)
        val s = cloneStartParams("o/kit", d, false, " lay it out ", "a1-x")
        assertEquals(listOf("a1-x", "/home/user/Documents", "kit-2", false, "lay it out"), listOf(s.get("id"), s.get("parent"), s.get("name"), s.get("thread"), s.get("text")))
        assertFalse("no first message: no text", cloneStartParams("o/kit", d, true, "", "a1-x").has("text"))
        val busy = CloneDest.of(JSONObject("""{"ok":false,"error":"~/Documents/kit already exists and isn't empty. Pick another folder or name.","repo":"o/kit","dest":"/home/user/Documents/kit","folder":"kit"}"""))
        assertTrue(busy.known && !busy.ok)
        assertFalse("not an address", CloneDest.of(JSONObject("""{"ok":false,"error":"\"kit\" isn't a repository address"}""")).known)
        assertEquals("o/kit", cloneRepoArg(CloneRepo("o/kit", "o/kit", "", false, "o"), "ki")); assertEquals("https://x.org/o/r", cloneRepoArg(null, " https://x.org/o/r "))
    }
    @Test fun signin_is_open_until_it_ends() {
        assertTrue(CloneSignin.of(JSONObject("""{"state":"waiting","code":"ABCD-1234","url":"https://github.com/login/device","error":""}"""))!!.open)
        assertFalse(CloneSignin.of(JSONObject("""{"state":"done"}"""))!!.open)
        assertNull(CloneSignin.of(JSONObject("""{"state":""}""")))
    }
    // the review's race: a fast clone's done push lands before clone.start's reply (running); a reconnect's status may be older than a push
    @Test fun an_ended_job_is_never_taken_back_to_running_and_two_ended_go_by_ended() {
        val run = job(""""state":"running","pct":10,"signin":false,"notes":[],"ended":0""")
        val done = job(""""state":"done","pct":100,"signin":false,"notes":[],"ended":50""")
        assertEquals(50L, done.ended)
        assertEquals("the late reply loses", done, cloneMerge(done, run))
        assertEquals(done, cloneMerge(run, done))
        val later = run.copy(pct = 60); assertEquals("running after running: the newer", later, cloneMerge(run, later))
        val failed = done.copy(state = "failed", ended = 40)
        assertEquals("the later end wins", done, cloneMerge(done, failed)); assertEquals(done, cloneMerge(failed, done))
        assertEquals("a first snapshot is taken", run, cloneMerge(null, run))
        val other = run.copy(id = "c2"); assertEquals(other, cloneMerge(done, other))
    }
    @Test fun ids_are_named_here_in_the_hubs_alphabet() {
        val a = cloneNewId(1795214181848, kotlin.random.Random(1)); val b = cloneNewId(1795214181848, kotlin.random.Random(2))
        assertTrue(a, Regex("^a1795214181848-[a-z0-9]{10}$").matches(a)); assertTrue(a.length <= 64 && a != b)
    }
    @Test fun try_again_starts_the_same_clone_into_the_same_folder_under_a_new_id() {
        val f = job(""""state":"failed","pct":null,"signin":false,"notes":[]""")
        val p = cloneRetryParams(f, CloneSpec("https://github.com/o/kit.git", false, "lay it out"), "a2-y")
        assertEquals(listOf("a2-y", "https://github.com/o/kit.git", "/home/user/Documents/kit", false, "lay it out"), listOf(p.get("id"), p.get("repo"), p.get("dest"), p.get("thread"), p.get("text")))
        assertFalse("folder, not parent and name", p.has("parent") || p.has("name"))
        val q = cloneRetryParams(f, null, "a3-z"); assertEquals("o/kit", q.get("repo")); assertEquals(true, q.get("thread")); assertFalse(q.has("text"))
    }
}
