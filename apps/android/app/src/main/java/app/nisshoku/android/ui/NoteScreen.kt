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
 * A note, in the editor that suits it: the app's own text editor
 * (NativeEditor.kt), or the formatted one in a web view (Editor.kt) for
 * what needs it. The `editorMode` preference decides (`auto` by the note's
 * content, `native`, `rich`); each editor can hand the note to the other.
 */
@Composable
fun NoteScreen(noteId: String, dark: Boolean, model: AppModel, onClose: () -> Unit) {
    // The note shown (a link in the formatted editor may lead to another).
    var current by rememberSaveable(noteId) { mutableStateOf(noteId) }
    var mode by rememberSaveable(noteId) { mutableStateOf<String?>(null) }
    // The note opened last (where "on start, open the last note" goes back to).
    LaunchedEffect(current) {
        if (model.prefString("lastNoteId", "") != current) model.setPref("lastNoteId", kotlinx.serialization.json.JsonPrimitive(current))
    }
    LaunchedEffect(noteId) {
        if (mode != null) return@LaunchedEffect
        val note = runCatching { Api.note(noteId) }.getOrNull()
        mode = when {
            // A conflict copy: the formatted editor shows it beside the original.
            note?.isConflict == true -> "rich"
            model.prefString("editorMode", "auto") == "native" -> "native"
            model.prefString("editorMode", "auto") == "rich" -> "rich"
            needsRichEditor(note?.body ?: "") -> "rich"
            else -> "native"
        }
    }
    AnimatedContent(mode, transitionSpec = { fadeIn() togetherWith fadeOut() }, label = "editor") { shown ->
        when (shown) {
            "native" -> androidx.compose.runtime.key(current) {
                NativeEditorScreen(noteId = current, model = model, onClose = onClose, onRich = { mode = "rich" }, onOther = { current = it })
            }
            "rich" -> EditorScreen(noteId = current, dark = dark, onClose = onClose, onPlain = { mode = "native" }, onOpened = { current = it })
            else -> Box(Modifier.fillMaxSize())
        }
    }
}
