package app.nisshoku.android.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import app.nisshoku.android.engine.Api

/**
 * A note: in the app's own editor (NativeEditor.kt), or, for a work's plot,
 * on its board (Works.kt).
 */
@Composable
fun NoteScreen(noteId: String, dark: Boolean, model: AppModel, onClose: () -> Unit) {
    // The note shown (the diary moves to other days, a conflict to the note kept).
    var current by rememberSaveable(noteId) { mutableStateOf(noteId) }
    var mode by rememberSaveable(noteId) { mutableStateOf<String?>(null) }
    // The note opened last (where "on start, open the last note" goes back to).
    LaunchedEffect(current) {
        if (model.prefString("lastNoteId", "") != current) model.setPref("lastNoteId", kotlinx.serialization.json.JsonPrimitive(current))
    }
    LaunchedEffect(noteId) {
        if (mode != null) return@LaunchedEffect
        val note = runCatching { Api.note(noteId) }.getOrNull()
        mode = if (note?.kind == "plot") "plot" else "native"
    }
    AnimatedContent(mode, transitionSpec = { fadeIn() togetherWith fadeOut() }, label = "editor") { shown ->
        when (shown) {
            "native" -> androidx.compose.runtime.key(current) {
                NativeEditorScreen(noteId = current, model = model, onClose = onClose, onOther = { current = it })
            }
            "plot" -> PlotScreen(current, model, onClose)
            else -> Box(Modifier.fillMaxSize())
        }
    }
}
