package dev.backplane.mobile

import android.app.Application

class BackplaneApp : Application() {
}

// how often a composable runs (debug builds only: "Backplane render <name>"
// in logcat), to see what one key recomposes
object RenderCount {
    @Volatile var on = false
    fun tick(name: String) { if (on) android.util.Log.d("BackplaneRender", name) }
}
