package dev.backplane.mobile

import androidx.compose.foundation.lazy.LazyListState
import org.json.JSONObject

internal data class TimelineReading(val id: String, val offset: Double, val first: String, val last: String) {
    fun json() = JSONObject().put("id", id).put("offset", offset).put("first", first).put("last", last).toString()
    val span get() = listOf(id, first, last)
}

internal fun timelineReading(list: LazyListState, thread: ThreadView): TimelineReading? {
    val ids = thread.entries.mapTo(mutableSetOf()) { it.id }
    val layout = list.layoutInfo
    val visible = layout.visibleItemsInfo.filter {
        it.key.toString() in ids && it.offset + it.size > layout.viewportStartOffset && it.offset < layout.viewportEndOffset
    }
    val anchor = visible.firstOrNull { it.index == list.firstVisibleItemIndex } ?: visible.firstOrNull() ?: return null
    return TimelineReading(anchor.key.toString(), anchor.offset.toDouble(), visible.first().key.toString(), visible.last().key.toString())
}
