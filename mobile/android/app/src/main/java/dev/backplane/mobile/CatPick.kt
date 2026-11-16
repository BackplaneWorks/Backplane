package dev.backplane.mobile

// The cat control's decisions (feature `cats`, docs/PHONE-DATA.md): how a colour typed is read, what a rig is cached by, and what the
// preview asks `cat.rig` while a name is typed. No Android here, so CatPickTest runs it on the JVM.
object CatPick {
    /** A colour as the hub keeps it: "#rrggbb" in lower case from "#RRGGBB" or "rrggbb" (spaces around are fine); null for anything else.
     *  (bots/cat.rs `parse_color`: the same rule, so a colour this accepts is never refused.) */
    fun hex(s: String): String? {
        val h = s.trim().removePrefix("#")
        if (h.length != 6 || !h.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) return null
        return "#" + h.lowercase()
    }
    /** A rig's cache key: the hub answers the same rig for the same look, colour and mood. A colour that does not parse is the coat's own, as the hub reads it. */
    fun key(look: Int, color: String, mood: String = "idle"): String = "$look|${hex(color) ?: ""}|${mood.ifEmpty { "idle" }}"
    /** Whether the bot's look was picked (Different cat) rather than following its name: the look its name gives (`cat.rig {name}`) differs
     *  from the one `bots.list` has. Not yet known: following the name. */
    fun picked(rowLook: Int, nameLook: Int?): Boolean = nameLook != null && nameLook != rowLook
    /** The preview's `cat.rig` parameters while `typed` is in the name field: a picked look stays whatever the name, so it is asked by look;
     *  else by the name typed (the hub hashes it). An empty field shows the bot's own cat. */
    fun ask(typed: String, color: String, rowLook: Int, picked: Boolean): Map<String, Any> {
        val n = typed.trim()
        return if (picked || n.isEmpty()) mapOf("look" to rowLook, "color" to color, "mood" to "idle") else mapOf("name" to n, "color" to color, "mood" to "idle")
    }
}
