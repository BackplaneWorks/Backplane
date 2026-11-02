package dev.backplane.mobile

import java.net.URLEncoder

fun main() {
    val source = "  Report + & ? # / 日本語 👩🏽‍💻\r\n\nkeep trailing spaces  \n"
    val body = source + "\n\n---\nOS: Android\nApp version: 1.2.3\n"
    val url = "https://github.com/BackplaneWorks/Backplane/issues/new?title=Backplane%20improvement&body=" +
        URLEncoder.encode(body, "UTF-8").replace("+", "%20")
    check(ImprovementsPreview(body, url).valid())
    check(!ImprovementsPreview(body + " ", url).valid())
    for (bad in listOf(url.replace("github.com", "github.com.evil.test"), url.replace("https:", "http:"),
        "$url&body=other", "$url&token=private", "$url#fragment", url.replace("BackplaneWorks", "another-owner"),
        url.replace("github.com", "name@github.com"))) check(!ImprovementsPreview(body, bad).valid())
    check(ImprovementsDraft().target == ImprovementsDraft.PUBLIC)
    check(!ImprovementsDraft().screenshotConsent)
    check(ImprovementsDraft.restored(null, source, false, false).target == ImprovementsDraft.INTERNAL)
    check(ImprovementsDraft.restored("unknown", source, false, false).target == ImprovementsDraft.INTERNAL)
    check(ImprovementsDraft.restored("internal", source, false, true).opened)
    check(ImprovementsDraft.restored("issue", source, false, false).target == ImprovementsDraft.PUBLIC)
    val saved = ImprovementsDraft(text = source, report = body, opened = true, evidenceReviewed = true)
    check(saved.text == source && saved.report == body && saved.copy().opened)
    val edited = saved.edited(source + "edit")
    check(!edited.opened && !edited.evidenceReviewed)
    check(!ImprovementsPreview(body, "").valid())
    val preview = ImprovementsPreview(body, url)
    val ready = saved.copy(opened = false, evidenceDetails = "Steps, expected and actual behavior")
    check(ready.canOpen(preview, false))
    check(!ready.copy(opened = true).canOpen(preview, false))
    check(!ready.copy(evidenceReviewed = false).canOpen(preview, true))
    check(!ready.copy(evidenceDetails = "").canOpen(preview, false))
    check(ready.copy(evidenceDetails = "").canOpen(preview, true))
    check(!ready.canOpen(preview.copy(body = "$body stale"), true))
    println("Android Improvements model checks passed; no browser or GitHub request executed")
}
