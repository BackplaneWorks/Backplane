package dev.backplane.mobile

// A hub without views is COMPATIBILITY_REQUIRED: it needs an update. The
// whole-log compatibility sync is retired: COMPATIBILITY_CHOSEN is never set
// (the app loads the recent window and pages back, never all history).
enum class StartupPolicy { DISCOVERING, THIN, COMPATIBILITY_REQUIRED, COMPATIBILITY_CHOSEN }

// Owner identity alone cannot distinguish two tabs of the same bot.
class ThinPresentationFence {
    private var epoch: Long? = null
    private var revision = 0L
    private var active = false

    fun observe(update: SyncUpdate, epoch: Long) = update.thinPresenting && begin(epoch, update.requestRevision)

    fun begin(epoch: Long, revision: Long): Boolean {
        if (revision !in 1..4294967295L || (this.epoch == epoch && revision <= this.revision)) return false
        this.epoch = epoch
        this.revision = revision
        active = true
        return true
    }

    fun accepts(epoch: Long, revision: Long) = active && this.epoch == epoch && this.revision == revision

    fun invalidate() { active = false }
}
