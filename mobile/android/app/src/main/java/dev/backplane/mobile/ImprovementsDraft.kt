package dev.backplane.mobile

import java.net.URI
import java.net.URLDecoder

// Local state only. No screen, hub, path, thread or account input belongs here.
data class ImprovementsDraft(
    val target: String = PUBLIC,
    val text: String = "",
    val screenshotConsent: Boolean = false,
    val opened: Boolean = false,
    val report: String = "",
    val evidenceDetails: String = "",
    val evidenceReviewed: Boolean = false,
) {
    fun edited(text: String) = copy(text = text, opened = false, evidenceReviewed = false)
    fun canOpen(preview: ImprovementsPreview, hasImage: Boolean) = !opened && evidenceReviewed &&
        (hasImage || evidenceDetails.isNotBlank()) && report.isNotBlank() && preview.body == report && preview.valid()
    companion object {
        const val PUBLIC = "issue"
        const val INTERNAL = "internal"
        // Missing/unknown target in an existing record stays internal, never public.
        fun restored(target: String?, text: String, consent: Boolean, opened: Boolean) =
            ImprovementsDraft(if (target == PUBLIC) PUBLIC else INTERNAL, text, consent, opened)
    }
}

data class ImprovementsPreview(val body: String, val url: String) {
    fun valid(): Boolean = runCatching {
        val uri = URI(url)
        val items = uri.rawQuery.orEmpty().split('&').map { part ->
            val pair = part.split('=', limit = 2)
            URLDecoder.decode(pair[0], "UTF-8") to URLDecoder.decode(pair.getOrElse(1) { "" }, "UTF-8")
        }
        uri.scheme == "https" && uri.host == "github.com" && uri.port == -1 && uri.rawUserInfo == null &&
            uri.rawFragment == null && uri.path == "/BackplaneWorks/Backplane/issues/new" &&
            url.length <= 8000 && items.all { it.first == "body" || it.first == "title" } &&
            items.count { it.first == "title" } == 1 && items.first { it.first == "title" }.second == "Backplane improvement" &&
            items.count { it.first == "body" } == 1 && items.first { it.first == "body" }.second == body
    }.getOrDefault(false)
}
