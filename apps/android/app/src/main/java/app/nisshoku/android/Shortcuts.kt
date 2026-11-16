package app.nisshoku.android

import android.content.Context
import android.content.Intent
import android.content.pm.ShortcutInfo
import android.content.pm.ShortcutManager
import android.graphics.drawable.Icon
import android.net.Uri
import android.util.Log

/**
 * Long-press shortcuts on the launcher icon: today's diary, a new note, a
 * new task. They open the same `nisshoku://` links as the widgets, in the
 * app's language.
 */
object Shortcuts {
    fun update(context: Context) {
        try {
            val manager = context.getSystemService(ShortcutManager::class.java) ?: return
            fun shortcut(id: String, label: String, long: String, icon: Int, link: String) =
                ShortcutInfo.Builder(context, id)
                    .setShortLabel(label)
                    .setLongLabel(long)
                    .setIcon(Icon.createWithResource(context, icon))
                    .setIntent(Intent(Intent.ACTION_VIEW, Uri.parse(link), context, MainActivity::class.java))
                    .build()
            manager.dynamicShortcuts = listOf(
                shortcut("new-task", trNow("新しいタスク", "New task"), trNow("新しいタスク", "New task"), R.drawable.shortcut_task_icon, "nisshoku://new-task"),
                shortcut("new-note", trNow("新しいノート", "New note"), trNow("新しいノート", "New note"), R.drawable.shortcut_note_icon, "nisshoku://new-note"),
                shortcut("diary", trNow("日記", "Diary"), trNow("今日の日記", "Today's diary"), R.drawable.shortcut_diary_icon, "nisshoku://diary"),
            )
        } catch (e: Exception) {
            Log.w("Shortcuts", "could not set the shortcuts: $e")
        }
    }
}
