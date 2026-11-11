package dev.backplane.mobile

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

// Swarms on the phone (docs/SWARM.md, Client data): the board as `swarm.get` gives it, and what the screen decides from it.
class SwarmUiTest {
    // the shape native/host/src/bots/swarm.rs `board` writes
    private val got = JSONObject("""{"room":{"id":"r1","name":"debounce","kind":"swarm","project":"p1","goal":"a debounced encoder","state":"paused","reason":"circular","by":"t-boss","budget":{},"used":{}},
      "members":[{"member":"thread:t1","thread":"t1","name":"opus","color":"#e06c75","role":null,"provider":"claude","model":"opus","state":"working","title":"debounce · opus"},
                 {"member":"thread:t2","thread":"t2","name":"codex","color":"#61afef","role":"tester","provider":"codex","model":"","state":"removed","title":""}],
      "posts":[{"id":"m1","from":"opus","thread":"t1","color":"#e06c75","text":"I'll take it","tags":["claim"],"reply":"","mentions":[],"files":["src/a.rs"],"guard":false,"held":"","at":10},
               {"id":"m2","from":"codex","thread":"t2","color":"#61afef","text":"me too","tags":["claim"],"reply":"","mentions":[],"files":["src/a.rs"],"guard":false,"held":"","at":11},
               {"id":"m3","from":"hub","thread":"","color":"","text":"The room circles: paused.","tags":[],"reply":"","mentions":[],"files":[],"guard":true,"held":"","at":12},
               {"id":"m4","from":"codex","thread":"t2","color":"#61afef","text":"still","tags":["critique"],"reply":"","mentions":[],"files":[],"guard":false,"held":"paused","at":13}],
      "claims":[{"post":"m1","thread":"t1","files":["src/a.rs"],"state":"held"},{"post":"m2","thread":"t2","files":["src/a.rs"],"state":"waiting"}]}""")

    @Test fun theBoardReadsAsTheHubWritesIt() {
        val b = SwarmParse.board(got)
        assertEquals(listOf("paused", "circular", "a debounced encoder"), listOf(b.room.state, b.room.reason, b.room.goal))
        assertEquals(listOf("opus", "codex"), b.members.map { it.name })
        assertEquals("", b.members[0].role); assertEquals("tester", b.members[1].role)
        assertEquals(listOf("m1", "m2", "m3", "m4"), b.posts.map { it.id })
        assertEquals("seconds from the hub, ms on the phone", 10_000L, b.posts[0].at); assertEquals(1_794_385_718_000L, swarmMs(1_794_385_718_000L))
        assertTrue(b.posts[2].guard); assertEquals("paused", b.posts[3].held); assertEquals(listOf("src/a.rs"), b.posts[0].files)
    }
    @Test fun aClaimIsWaitingWhileAPeerHoldsItsFileAndPlainOnceDone() {
        val b = SwarmParse.board(got)
        assertEquals("held", SwarmUi.claim(b.posts[0], b.claims)); assertEquals("waiting", SwarmUi.claim(b.posts[1], b.claims))
        assertEquals("answered by a done: no open claim", "", SwarmUi.claim(b.posts[0], b.claims.drop(1)))
        assertEquals("not a claim", "", SwarmUi.claim(b.posts[3], b.claims))
    }
    @Test fun aRereadAppendsTheNewerPostsOncePerId() {
        val old = SwarmParse.board(got)
        val fresh = SwarmParse.board(JSONObject(got.toString()).put("posts", org.json.JSONArray().put(JSONObject().put("id", "m4").put("text", "edited")).put(JSONObject().put("id", "m5").put("text", "new"))))
        val m = SwarmUi.merge(old, fresh, SwarmUi.since(old))
        assertEquals("m4", SwarmUi.since(old))
        assertEquals(listOf("m1", "m2", "m3", "m4", "m5"), m.posts.map { it.id }); assertEquals("edited", m.posts[3].text)
        assertEquals("a whole read replaces", fresh, SwarmUi.merge(old, fresh, ""))
    }
    @Test fun theStateLineOffersWhatTheStateAllows() {
        assertEquals(listOf("pause", "stop"), SwarmUi.controls("active"))
        assertEquals(listOf("resume", "stop"), SwarmUi.controls("paused"))
        assertEquals(emptyList<String>(), SwarmUi.controls("done")); assertEquals(emptyList<String>(), SwarmUi.controls("stopped"))
        assertTrue(SwarmUi.ended("done") && SwarmUi.ended("stopped")); assertFalse(SwarmUi.ended("paused"))
    }
    @Test fun aProjectListsItsLiveSwarmsOnly() {
        val all = SwarmParse.list(JSONObject("""{"swarms":[{"id":"a","name":"A","project":"p1","state":"active","members":3},{"id":"b","name":"B","project":"p1","state":"done","members":2},
            {"id":"c","name":"C","project":"p2","state":"paused","members":2},{"id":"d","name":"D","project":"p1","state":"paused","reason":"idle","members":4}]}"""))
        assertEquals(listOf("a", "d"), SwarmUi.live(all, "p1").map { it.id }); assertEquals(3, all[0].members)
    }
    @Test fun aSynthesisIsSetApartAndAGuardHasNoLabels() {
        val b = SwarmParse.board(got)
        assertEquals(emptyList<String>(), SwarmUi.labels(b.posts[2]))
        assertFalse("a guard's post is grey, not dimmed", SwarmUi.dimmed(b.posts[2].copy(held = "guard"))); assertTrue(SwarmUi.dimmed(b.posts[3]))
        val syn = b.posts[0].copy(tags = listOf("synthesis", "done"))
        assertTrue(SwarmUi.synthesis(syn)); assertEquals(listOf("done"), SwarmUi.labels(syn))
    }
    @Test fun newSwarmNeedsATopicAndTwoToEightMembers() {
        val m = DSwarmPick("claude")
        assertFalse(SwarmUi.canCreate("", listOf(m, m))); assertFalse(SwarmUi.canCreate("t", listOf(m)))
        assertTrue(SwarmUi.canCreate("t", List(8) { m })); assertFalse(SwarmUi.canCreate("t", List(9) { m }))
        assertEquals("""{"provider":"codex"}""", DSwarmPick("codex", "", " ").json().toString())
        assertEquals(setOf("provider", "model", "role"), DSwarmPick("claude", "opus", "critic").json().keySet())
    }
    @Test fun aRemovedMembersChipOpensNothing() {
        val b = SwarmParse.board(got)
        assertTrue(SwarmUi.opens(b.members[0])); assertFalse(SwarmUi.opens(b.members[1]))
    }
    @Test fun aNewMemberRowTakesAProviderThatCanRun() {
        val ms = listOf("codex" to listOf(DModel("gpt", "gpt", false, "no CLI")), "claude" to listOf(DModel("opus", "Opus", true, "")))
        assertEquals("claude", SwarmUi.firstPick(ms).provider)
    }
}
