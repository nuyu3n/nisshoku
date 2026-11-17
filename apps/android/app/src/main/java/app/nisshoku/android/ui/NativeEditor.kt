package app.nisshoku.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.OutputTransformation
import androidx.compose.foundation.text.input.TextFieldBuffer
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.then
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.Article
import androidx.compose.material.icons.automirrored.outlined.FormatListBulleted
import androidx.compose.material.icons.automirrored.outlined.Redo
import androidx.compose.material.icons.automirrored.outlined.Undo
import androidx.compose.material.icons.outlined.CheckBox
import androidx.compose.material.icons.outlined.FormatBold
import androidx.compose.material.icons.outlined.FormatItalic
import androidx.compose.material.icons.outlined.FormatListNumbered
import androidx.compose.material.icons.outlined.FormatQuote
import androidx.compose.material.icons.outlined.KeyboardHide
import androidx.compose.material.icons.outlined.Title
import androidx.compose.material.icons.outlined.TextRotateVertical
import androidx.compose.material.icons.outlined.Toc
import androidx.compose.material.icons.outlined.ChatBubbleOutline
import androidx.compose.material.icons.outlined.Spellcheck
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.Image
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.Tag
import androidx.compose.material.icons.outlined.PushPin
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import app.nisshoku.android.engine.Api
import app.nisshoku.android.engine.Nisshoku
import app.nisshoku.android.tr
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.JsonPrimitive
import kotlin.math.abs

// The app's editor: the note's Markdown in a native text field, so that the
// keyboard (Gboard's Japanese input, its composition, its gestures) talks to
// Android's text input directly, with no web page in between. Markdown is
// styled as it is typed (headings, emphasis, lists, check boxes, tables and
// math in a fixed-width face) and stays what is stored; pictures show above
// the text.

// ---------------------------------------------------------------------------
// Markdown, as typed

/** A list item's start: indent, bullet or number, and a check box if any. */
private val listMarker = Regex("""^(\s*)([-*+]|\d+[.)])[ \t]+(\[([ xX])\][ \t]+)?""")
private val headingMarker = Regex("""^(#{1,6})[ \t]+""")
private val rule = Regex("""^\s*([-*_])(\s*\1){2,}\s*$""")
private val inlineMarks = Regex(
    """(\*\*|__)(?=\S)(.+?)(?<=\S)\1""" + // 1, 2: bold
        """|(?<![*\w])([*_])(?=\S)(.+?)(?<=\S)\3(?![*\w])""" + // 3, 4: italic
        """|~~(?=\S)(.+?)(?<=\S)~~""" + // 5: struck
        """|`([^`\n]+)`""" + // 6: code
        """|\[([^\]\n]+)\]\(([^)\n]+)\)""", // 7, 8: link
)

private class MarkdownColors(val dim: Color, val accent: Color, val quote: Color, val codeBackground: Color)

/** Styles the Markdown in the buffer (the text itself is left as it is). */
private fun TextFieldBuffer.styleMarkdown(c: MarkdownColors) {
    val text = asCharSequence()
    val dim = SpanStyle(color = c.dim)
    val code = SpanStyle(fontFamily = FontFamily.Monospace, background = c.codeBackground, fontSize = 0.9.em)
    var start = 0
    var fenced = false
    while (start <= text.length) {
        var end = text.indexOf('\n', start)
        if (end < 0) end = text.length
        if (end > start) {
            val line = text.subSequence(start, end)
            val fence = line.trimStart().startsWith("```")
            when {
                fence -> {
                    fenced = !fenced
                    addStyle(dim, start, end)
                }
                fenced -> addStyle(code, start, end)
                else -> styleLine(line, start, c, dim, code)
            }
        }
        start = end + 1
    }
}

private fun TextFieldBuffer.styleLine(line: CharSequence, at: Int, c: MarkdownColors, dim: SpanStyle, code: SpanStyle) {
    val end = at + line.length
    headingMarker.find(line)?.let { m ->
        val level = m.groupValues[1].length
        val size = when (level) {
            1 -> 1.45.em
            2 -> 1.25.em
            3 -> 1.12.em
            else -> 1.em
        }
        addStyle(SpanStyle(fontWeight = FontWeight.Bold, fontSize = size), at, end)
        addStyle(dim, at, at + level)
    }
    if (rule.matches(line)) {
        addStyle(dim, at, end)
        return
    }
    // A table row, or a block of math: in a fixed-width face, so that they line up.
    val trimmed = line.trimStart()
    if (trimmed.startsWith("|") || trimmed.startsWith("$$")) {
        addStyle(SpanStyle(fontFamily = FontFamily.Monospace, fontSize = 0.88.em, color = c.quote), at, end)
        return
    }
    // Images and attachments: their links dimmed (the pictures show above the text).
    for (m in Regex("""!?\[[^\]\n]*]\(:/[0-9a-f]{32}\)""").findAll(line)) {
        addStyle(SpanStyle(color = c.accent, fontSize = 0.85.em), at + m.range.first, at + m.range.last + 1)
    }
    for (m in Regex("""(?<!\$)\$[^\s$][^$\n]*\$(?!\$)""").findAll(line)) {
        addStyle(SpanStyle(fontFamily = FontFamily.Monospace, color = c.accent, fontSize = 0.9.em), at + m.range.first, at + m.range.last + 1)
    }
    if (line.trimStart().startsWith(">")) {
        addStyle(SpanStyle(color = c.quote, fontStyle = FontStyle.Italic), at, end)
    }
    listMarker.find(line)?.let { m ->
        addStyle(SpanStyle(color = c.accent, fontWeight = FontWeight.SemiBold), at + m.groups[2]!!.range.first, at + m.range.last + 1)
        // Done: struck through.
        if (m.groupValues[4].equals("x", ignoreCase = true) && end > at + m.range.last + 1) {
            addStyle(SpanStyle(color = c.dim, textDecoration = TextDecoration.LineThrough), at + m.range.last + 1, end)
        }
    }
    for (m in inlineMarks.findAll(line)) {
        val s = at + m.range.first
        val e = at + m.range.last + 1
        when {
            m.groups[2] != null -> {
                addStyle(SpanStyle(fontWeight = FontWeight.Bold), s, e)
                addStyle(dim, s, s + 2)
                addStyle(dim, e - 2, e)
            }
            m.groups[4] != null -> {
                addStyle(SpanStyle(fontStyle = FontStyle.Italic), s, e)
                addStyle(dim, s, s + 1)
                addStyle(dim, e - 1, e)
            }
            m.groups[5] != null -> {
                addStyle(SpanStyle(textDecoration = TextDecoration.LineThrough), s + 2, e - 2)
                addStyle(dim, s, s + 2)
                addStyle(dim, e - 2, e)
            }
            m.groups[6] != null -> addStyle(code, s, e)
            m.groups[7] != null -> {
                val text = m.groups[7]!!.range
                addStyle(SpanStyle(color = c.accent), at + text.first, at + text.last + 1)
                addStyle(dim, s, s + 1)
                addStyle(dim, at + text.last + 1, e)
            }
        }
    }
}

/** The line around `offset`: its start, and its end (before the line break). */
private fun CharSequence.lineAt(offset: Int): Pair<Int, Int> {
    val at = offset.coerceIn(0, length)
    val start = if (at == 0) 0 else lastIndexOf('\n', at - 1) + 1
    var end = indexOf('\n', at)
    if (end < 0) end = length
    return start to end
}

/**
 * Enter in a list goes on with the list: the next bullet, number or empty
 * check box. Enter on an empty item ends the list instead.
 */
private val continueLists = InputTransformation {
    val before = originalText
    val cursor = selection.start
    if (length != before.length + 1 || !selection.collapsed || cursor < 1 || charAt(cursor - 1) != '\n') return@InputTransformation
    if (originalSelection.start != cursor - 1 || !originalSelection.collapsed) return@InputTransformation
    val text = asCharSequence()
    val (start, _) = text.lineAt(cursor - 1)
    val line = text.subSequence(start, cursor - 1)
    val quote = Regex("""^(\s*>[ \t]?)""").find(line)
    val m = listMarker.find(line)
    val prefix = when {
        m != null -> {
            val number = m.groupValues[2].dropLast(1).toIntOrNull()
            val bullet = if (number != null) "${number + 1}${m.groupValues[2].last()}" else m.groupValues[2]
            m.groupValues[1] + bullet + " " + (if (m.groups[3] != null) "[ ] " else "")
        }
        quote != null -> quote.value
        else -> return@InputTransformation
    }
    val used = (m?.value ?: quote!!.value).length
    if (line.length <= used) {
        // An empty item: the list ends here, with the blank line Markdown
        // wants between a list and the paragraph after it.
        val inList = m != null && start > 0
        replace(start, cursor, if (inList) LINE_BREAK.toString() else "")
        selection = TextRange(start + if (inList) 1 else 0)
    } else {
        replace(cursor, cursor, prefix)
        selection = TextRange(cursor + prefix.length)
    }
}

/** "..." becomes "…" and "--" "――" as they are typed (the `smartPunctuation` preference). */
private val smartPunctuation = InputTransformation {
    if (length != originalText.length + 1 || !selection.collapsed) return@InputTransformation
    val at = selection.start
    if (at < 2) return@InputTransformation
    val text = asCharSequence()
    when {
        at >= 3 && text[at - 1] == '.' && text[at - 2] == '.' && text[at - 3] == '.' -> {
            replace(at - 3, at, "…")
            selection = TextRange(at - 2)
        }
        text[at - 1] == '-' && text[at - 2] == '-' && (at < 3 || text[at - 3] != '-') -> {
            replace(at - 2, at, "――")
            selection = TextRange(at)
        }
    }
}

// ---------------------------------------------------------------------------
// Edits of the format bar

/** Puts `prefix` at the start of the cursor's line, or takes it away; other list prefixes give way. */
private fun TextFieldState.toggleLinePrefix(prefix: String) = edit {
    val (start, end) = asCharSequence().lineAt(selection.start)
    val line = asCharSequence().subSequence(start, end).toString()
    val old = (listMarker.find(line)?.value ?: Regex("""^\s*>[ \t]?""").find(line)?.value ?: "")
    val cursor = selection.start
    val new = if (old.trim() == prefix.trim()) "" else prefix
    replace(start, start + old.length, new)
    selection = TextRange((cursor + new.length - old.length).coerceIn(start + new.length, length))
}

/** No heading → `#` → `##` → `###` → none. */
private fun TextFieldState.cycleHeading() = edit {
    val (start, end) = asCharSequence().lineAt(selection.start)
    val line = asCharSequence().subSequence(start, end).toString()
    val old = headingMarker.find(line)?.value ?: ""
    val level = old.trim().length
    val new = if (level >= 3) "" else "#".repeat(level + 1) + " "
    val cursor = selection.start
    replace(start, start + old.length, new)
    selection = TextRange((cursor + new.length - old.length).coerceIn(start + new.length, length))
}

/** A check box on the cursor's line: added, or ticked / unticked if there is one. */
private fun TextFieldState.toggleCheckbox() {
    val (start, end) = text.lineAt(selection.start)
    val m = listMarker.find(text.subSequence(start, end))
    if (m?.groups?.get(3) != null) tickCheckbox(start + m.groups[3]!!.range.first + 1) else toggleLinePrefix("- [ ] ")
}

private fun TextFieldState.tickCheckbox(at: Int) = edit {
    val cursor = selection
    replace(at, at + 1, if (charAt(at) == ' ') "x" else " ")
    selection = cursor
}

/** Wraps the selection in `mark` (`**`, `*`), or puts the pair at the cursor, to type between. */
private fun TextFieldState.wrap(mark: String) = edit {
    val s = selection.min
    val e = selection.max
    if (s == e) {
        replace(s, s, mark + mark)
        selection = TextRange(s + mark.length)
    } else {
        replace(e, e, mark)
        replace(s, s, mark)
        selection = TextRange(s + mark.length, e + mark.length)
    }
}

/** The check box (if any) that a tap at `offset` is on: the place of its mark. */
private fun CharSequence.checkboxAt(offset: Int): Int? {
    val (start, end) = lineAt(offset)
    val m = listMarker.find(subSequence(start, end)) ?: return null
    val box = m.groups[3] ?: return null
    // The whole marker ("- [ ] ") is the target: the box alone is small.
    return if (offset <= start + m.range.last + 1) start + box.range.first + 1 else null
}

// ---------------------------------------------------------------------------
// The screen

private const val LINE_BREAK = '\n'

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun NativeEditorScreen(noteId: String, model: AppModel, onClose: () -> Unit, onOther: (String) -> Unit = {}) {
    var noteKind by remember { mutableStateOf<String?>(null) }
    var parentId by remember { mutableStateOf("") }
    var conflict by remember { mutableStateOf(false) }
    /** The tool open: outline, comments, proof, history, preview. */
    var tool by remember { mutableStateOf<String?>(null) }
    var images by remember { mutableStateOf<Map<String, app.nisshoku.android.engine.ResourceFile>>(emptyMap()) }
    /** The words last selected, and the text then (a tap on the menu may clear the selection before a comment). */
    var lastSelection by remember { mutableStateOf<Pair<IntRange, String>?>(null) }
    var diaryDate by remember { mutableStateOf<String?>(null) }
    var pinned by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var editingTags by remember { mutableStateOf(false) }
    val context = androidx.compose.ui.platform.LocalContext.current
    val prefs by model.prefs.collectAsState()
    val scope = rememberCoroutineScope()
    val keyboard = LocalSoftwareKeyboardController.current
    val haptics = LocalHapticFeedback.current
    val close by rememberUpdatedState(onClose)

    val title = rememberTextFieldState()
    val body = rememberTextFieldState()
    var loaded by remember { mutableStateOf(false) }
    var readOnly by remember { mutableStateOf(false) }
    var saving by remember { mutableStateOf(false) }

    // What the engine has (the base of the next save's merge).
    val session = remember(noteId) {
        object {
            var title = ""
            var body = ""
            val lock = Mutex()
        }
    }

    suspend fun save() {
        if (!loaded || readOnly) return
        session.lock.withLock {
            val t = title.text.toString()
            val b = body.text.toString()
            if (t == session.title && b == session.body) return
            saving = true
            try {
                Api.saveNote(noteId, t, b, session.title, session.body)
                session.title = t
                session.body = b
            } catch (e: Exception) {
                if (e is kotlinx.coroutines.CancellationException) throw e
                model.say(e.message ?: "")
            } finally {
                saving = false
            }
        }
    }

    /** Takes what the engine has now (after a sync), unless it is what is shown. */
    suspend fun reload() {
        val note = runCatching { Api.note(noteId) }.getOrNull() ?: return
        session.lock.withLock {
            if (note.body != body.text.toString()) {
                body.edit {
                    val cursor = selection.start
                    replace(0, length, note.body)
                    selection = TextRange(cursor.coerceAtMost(length))
                }
            }
            if (note.title != title.text.toString()) title.edit { replace(0, length, note.title) }
            session.title = note.title
            session.body = note.body
        }
    }

    val titleFocus = remember { FocusRequester() }
    // A picture from the gallery becomes an attachment, put in at the cursor.
    val context0 = androidx.compose.ui.platform.LocalContext.current
    val picker = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia()) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        scope.launch {
            val r = runCatching {
                val bytes = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { context0.contentResolver.openInputStream(uri)!!.use { it.readBytes() } }
                val mime = context0.contentResolver.getType(uri) ?: "image/jpeg"
                Api.saveResource(bytes, "image." + (android.webkit.MimeTypeMap.getSingleton().getExtensionFromMimeType(mime) ?: "jpg"), mime)
            }.onFailure { model.say(it.message ?: "") }.getOrNull() ?: return@launch
            images = images + (r.id to r)
            body.edit {
                val at = selection.max
                val before = if (at > 0 && asCharSequence()[at - 1] != '\n') "\n" else ""
                val text = "$before![${r.title.ifBlank { "image" }}](:/${r.id})\n"
                replace(at, at, text)
                selection = TextRange(at + text.length)
            }
        }
    }
    var startAtTitle by remember { mutableStateOf(false) }
    LaunchedEffect(noteId) {
        val note = try {
            Api.note(noteId)
        } catch (e: Exception) {
            if (e is kotlinx.coroutines.CancellationException) throw e
            model.say(e.message ?: "")
            close()
            return@LaunchedEffect
        }
        title.edit { replace(0, length, note.title) }
        body.edit {
            replace(0, length, note.body)
            selection = TextRange(0)
        }
        title.undoState.clearHistory()
        body.undoState.clearHistory()
        session.title = note.title
        session.body = note.body
        readOnly = note.deletedTime > 0
        pinned = note.pinned
        diaryDate = note.diaryDate
        noteKind = note.kind
        parentId = note.parentId
        conflict = note.isConflict
        images = runCatching { Api.resourceFiles() }.getOrDefault(emptyList()).associateBy { it.id }
        startAtTitle = note.title.isEmpty() && note.body.isEmpty() && !readOnly
        loaded = true
    }
    // A new note starts at its title, with the keyboard up (once the fields are there).
    LaunchedEffect(loaded) {
        if (loaded && startAtTitle) {
            delay(80)
            runCatching { titleFocus.requestFocus() }
            keyboard?.show()
        }
    }

    LaunchedEffect(loaded) {
        if (!loaded) return@LaunchedEffect
        snapshotFlow { body.selection to body.text.toString() }.collect { (sel, text) ->
            if (!sel.collapsed) lastSelection = (sel.min until sel.max) to text
            else if (lastSelection?.second != text) lastSelection = null
        }
    }
    // Saved a moment after the last keystroke, when the app leaves the
    // screen, and on the way out.
    LaunchedEffect(loaded) {
        if (!loaded) return@LaunchedEffect
        snapshotFlow { title.text.toString() to body.text.toString() }.drop(1).collectLatest {
            delay(700)
            save()
        }
    }
    LifecycleEventEffect(Lifecycle.Event.ON_STOP) { Nisshoku.scope.launch { save() } }
    DisposableEffect(noteId) { onDispose { Nisshoku.scope.launch { save() } } }
    // A sync changed the notes: what was typed is saved (merged), then shown as it is now.
    LaunchedEffect(loaded) {
        if (!loaded) return@LaunchedEffect
        Nisshoku.events.collect { e ->
            if (e.name == "data-changed") {
                save()
                reload()
            }
        }
    }

    fun leave(then: () -> Unit) {
        keyboard?.hide()
        scope.launch {
            save()
            then()
        }
    }
    BackHandler { leave { close() } }

    val scheme = MaterialTheme.colorScheme
    val colors = remember(scheme) {
        MarkdownColors(dim = scheme.outline, accent = scheme.primary, quote = scheme.onSurfaceVariant, codeBackground = scheme.surfaceContainerHigh)
    }
    val markdown = remember(colors) { OutputTransformation { styleMarkdown(colors) } }
    val family = if ((prefs["editorFont"] as? JsonPrimitive)?.content == "sans") FontFamily.SansSerif else FontFamily.Serif
    val size = model.prefInt("fontSize", 17).coerceIn(12, 30)
    val lineHeight = ((prefs["lineHeight"] as? JsonPrimitive)?.content?.toFloatOrNull() ?: 1.95f).coerceIn(1.2f, 2.6f)
    val bodyStyle = TextStyle(fontFamily = family, fontSize = size.sp, lineHeight = (size * lineHeight * 0.92f).sp, color = scheme.onSurface)

    var layout by remember { mutableStateOf<(() -> TextLayoutResult?)?>(null) }
    val bodyFocus = remember { FocusRequester() }
    // The title is one paragraph: Enter goes on to the text (pasted line breaks become spaces).
    var toBody by remember { mutableStateOf(0) }
    val titleInput = remember {
        InputTransformation {
            if (asCharSequence().indexOf(LINE_BREAK) < 0) return@InputTransformation
            if (length == originalText.length + 1) {
                revertAllChanges()
                toBody++
            } else {
                var i = asCharSequence().indexOf(LINE_BREAK)
                while (i >= 0) {
                    replace(i, i + 1, " ")
                    i = asCharSequence().indexOf(LINE_BREAK)
                }
            }
        }
    }
    LaunchedEffect(toBody) { if (toBody > 0) runCatching { bodyFocus.requestFocus() } }

    Column(Modifier.fillMaxSize().background(scheme.background).imePadding()) {
        TopAppBar(
            title = {
                val count = body.text.count { !it.isWhitespace() }
                Text(
                    tr("${count}字", "$count chars") + if (saving) " …" else "",
                    style = MaterialTheme.typography.labelLarge,
                    color = scheme.onSurfaceVariant,
                )
            },
            navigationIcon = {
                IconButton(onClick = { leave { close() } }) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, tr("戻る", "Back")) }
            },
            actions = {
                if (loaded) IconButton(onClick = { keyboard?.hide(); tool = "preview" }) {
                    Icon(Icons.Outlined.TextRotateVertical, tr("縦書きプレビュー", "Vertical preview"))
                }
                if (loaded && !readOnly) Box {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, tr("その他", "More")) }
                    androidx.compose.material3.DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, shape = androidx.compose.foundation.shape.RoundedCornerShape(16.dp)) {
                        androidx.compose.material3.DropdownMenuItem(
                            text = { Text(tr("見出し", "Outline")) },
                            leadingIcon = { Icon(Icons.Outlined.Toc, null) },
                            onClick = { menu = false; tool = "outline" },
                        )
                        androidx.compose.material3.DropdownMenuItem(
                            text = { Text(tr("コメント", "Comments")) },
                            leadingIcon = { Icon(Icons.Outlined.ChatBubbleOutline, null) },
                            onClick = { menu = false; tool = "comments" },
                        )
                        androidx.compose.material3.DropdownMenuItem(
                            text = { Text(tr("文章チェック", "Writing check")) },
                            leadingIcon = { Icon(Icons.Outlined.Spellcheck, null) },
                            onClick = { menu = false; tool = "proof" },
                        )
                        androidx.compose.material3.DropdownMenuItem(
                            text = { Text(tr("履歴", "History")) },
                            leadingIcon = { Icon(Icons.Outlined.History, null) },
                            onClick = { menu = false; scope.launch { save(); tool = "history" } },
                        )
                        androidx.compose.material3.HorizontalDivider()
                        androidx.compose.material3.DropdownMenuItem(
                            text = { Text(tr("タグ", "Tags")) },
                            leadingIcon = { Icon(Icons.Outlined.Tag, null) },
                            onClick = { menu = false; editingTags = true },
                        )
                        androidx.compose.material3.DropdownMenuItem(
                            text = { Text(if (pinned) tr("ピン留めを外す", "Unpin") else tr("ピン留め", "Pin")) },
                            leadingIcon = { Icon(Icons.Outlined.PushPin, null) },
                            onClick = {
                                menu = false
                                pinned = !pinned
                                scope.launch { runCatching { Api.setPinned(noteId, pinned) } }
                            },
                        )
                        androidx.compose.material3.DropdownMenuItem(
                            text = { Text(tr("共有", "Share")) },
                            leadingIcon = { Icon(Icons.Outlined.Share, null) },
                            onClick = { menu = false; scope.launch { save(); shareNote(context, noteId) } },
                        )
                        androidx.compose.material3.DropdownMenuItem(
                            text = { Text(tr("ゴミ箱へ移動", "Move to trash"), color = scheme.error) },
                            leadingIcon = { Icon(Icons.Outlined.Delete, null, tint = scheme.error) },
                            onClick = {
                                menu = false
                                leave {
                                    scope.launch {
                                        runCatching { Api.trashNote(noteId) }
                                        close()
                                    }
                                }
                            },
                        )
                    }
                }
            },
        )
        Box(Modifier.weight(1f).fillMaxWidth()) {
            if (!loaded) {
                LoadingIndicator(Modifier.align(Alignment.Center))
            } else {
                Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp)) {
                    if (conflict) {
                        ConflictBanner(
                            onKeepCopy = { scope.launch { save(); model.resolveConflict(noteId, "copy")?.let { onOther(it) } ?: close() } },
                            onKeepOriginal = { scope.launch { model.resolveConflict(noteId, "original")?.let { onOther(it) } ?: close() } },
                        )
                    }
                    diaryDate?.let { date ->
                        DiaryMetaBar(noteId, date) { day ->
                            leave {
                                scope.launch { model.diaryOf(day)?.let(onOther) }
                            }
                        }
                    }
                    // (A diary entry's title is its day, shown above.)
                    if (diaryDate == null) BasicTextField(
                        state = title,
                        readOnly = readOnly,
                        textStyle = TextStyle(fontFamily = family, fontSize = (size * 1.55f).sp, lineHeight = (size * 2.1f).sp, fontWeight = FontWeight.Bold, color = scheme.onSurface),
                        inputTransformation = titleInput,
                        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Next),
                        onKeyboardAction = { runCatching { bodyFocus.requestFocus() } },
                        cursorBrush = SolidColor(scheme.primary),
                        decorator = { inner ->
                            Box {
                                if (title.text.isEmpty()) {
                                    Text(tr("タイトル", "Title"), style = TextStyle(fontFamily = family, fontSize = (size * 1.55f).sp, lineHeight = (size * 2.1f).sp, fontWeight = FontWeight.Bold), color = scheme.outline.copy(alpha = 0.6f))
                                }
                                inner()
                            }
                        },
                        modifier = Modifier.fillMaxWidth().padding(top = 4.dp, bottom = 12.dp).focusRequester(titleFocus),
                    )
                    // A work's chapter or character: its sheet above the text.
                    val work = remember(parentId, model.home.collectAsState().value.folders) { workOf(model.home.value.folders, parentId) }
                    if (work != null && noteKind == null) ChapterBar(noteId, work.id)
                    if (noteKind == "character") CharacterFields(noteId)
                    AttachmentStrip(body.text.toString(), images)
                    BasicTextField(
                        state = body,
                        readOnly = readOnly,
                        textStyle = bodyStyle,
                        inputTransformation = if (model.prefBool("smartPunctuation", true)) continueLists.then(smartPunctuation) else continueLists,
                        outputTransformation = markdown,
                        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                        cursorBrush = SolidColor(scheme.primary),
                        onTextLayout = { layout = it },
                        decorator = { inner ->
                            Box {
                                if (body.text.isEmpty()) Text(if (diaryDate != null) diaryPrompt(diaryDate!!) else tr("書きはじめる…", "Start writing…"), style = bodyStyle, color = scheme.outline.copy(alpha = 0.6f))
                                inner()
                            }
                        },
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = 320.dp)
                            .focusRequester(bodyFocus)
                            // A tap on a check box ticks it (and is not a tap into the text).
                            .pointerInput(readOnly) {
                                if (readOnly) return@pointerInput
                                awaitEachGesture {
                                    val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                                    val up = waitForUpOrCancellation(PointerEventPass.Initial) ?: return@awaitEachGesture
                                    val moved = up.position - down.position
                                    if (abs(moved.x) > viewConfiguration.touchSlop || abs(moved.y) > viewConfiguration.touchSlop) return@awaitEachGesture
                                    if (up.uptimeMillis - down.uptimeMillis > viewConfiguration.longPressTimeoutMillis) return@awaitEachGesture
                                    val result = layout?.invoke() ?: return@awaitEachGesture
                                    val box = body.text.checkboxAt(result.getOffsetForPosition(up.position)) ?: return@awaitEachGesture
                                    // (Only when the tap is on the marker's side of the line, not beyond its end.)
                                    if (up.position.x > result.getHorizontalPosition(box + 2, true) + 24.dp.toPx()) return@awaitEachGesture
                                    up.consume()
                                    body.tickCheckbox(box)
                                    haptics.performHapticFeedback(HapticFeedbackType.ToggleOn)
                                }
                            },
                    )
                    Spacer(Modifier.height(160.dp))
                }
            }
        }
        if (editingTags) {
            val current = model.home.collectAsState().value.notes.find { it.id == noteId }?.tags ?: emptyList()
            TagsSheet(current = current, all = model.home.collectAsState().value.tags, onDismiss = { editingTags = false }, onSave = { model.setTags(noteId, it) })
        }
        if (loaded && !readOnly) {
            FormatBar(
                body = body,
                onHideKeyboard = { keyboard?.hide() },
                onImage = { picker.launch(androidx.activity.result.PickVisualMediaRequest(androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia.ImageOnly)) },
            )
        }
        fun show(r: IntRange) {
            val end = minOf(r.last + 1, body.text.length)
            body.edit { selection = TextRange(minOf(r.first, end), end) }
            runCatching { bodyFocus.requestFocus() }
        }
        when (tool) {
            "outline" -> OutlineSheet(body.text.toString(), onDismiss = { tool = null }) { at -> tool = null; show(at until at) }
            "comments" -> CommentsSheet(
                noteId,
                body.text.toString(),
                body.selection.takeIf { !it.collapsed }?.let { it.min until it.max }
                    ?: lastSelection?.takeIf { it.second == body.text.toString() }?.first,
                onDismiss = { tool = null },
                onShow = { show(it) },
            )
            "proof" -> ProofSheet(body.text.toString(), onDismiss = { tool = null }) { show(it) }
            "history" -> HistorySheet(noteId, onDismiss = { tool = null }, onRestored = { scope.launch { reload() } })
            "preview" -> GenkoPreview(
                title.text.toString(),
                body.text.toString(),
                model.prefInt("manuscriptCols", 20).coerceIn(10, 40),
                model.prefInt("manuscriptRows", 20).coerceIn(5, 40),
                model.prefString("genkoInk", "accent"),
                onDismiss = { tool = null },
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun FormatBar(body: TextFieldState, onHideKeyboard: () -> Unit, onImage: () -> Unit) {
    val haptics = LocalHapticFeedback.current
    Surface(color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
        Row(
            Modifier.navigationBarsPadding().horizontalScroll(rememberScrollState()).padding(horizontal = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            @Composable
            fun button(icon: ImageVector, label: String, enabled: Boolean = true, run: () -> Unit) {
                IconButton(
                    onClick = {
                        haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                        run()
                    },
                    enabled = enabled,
                    modifier = Modifier.size(52.dp),
                ) { Icon(icon, label) }
            }
            button(Icons.AutoMirrored.Outlined.Undo, tr("元に戻す", "Undo"), body.undoState.canUndo) { body.undoState.undo() }
            button(Icons.AutoMirrored.Outlined.Redo, tr("やり直す", "Redo"), body.undoState.canRedo) { body.undoState.redo() }
            button(Icons.Outlined.CheckBox, tr("チェックボックス", "Check box")) { body.toggleCheckbox() }
            button(Icons.AutoMirrored.Outlined.FormatListBulleted, tr("箇条書き", "Bullet list")) { body.toggleLinePrefix("- ") }
            button(Icons.Outlined.FormatListNumbered, tr("番号付きリスト", "Numbered list")) { body.toggleLinePrefix("1. ") }
            button(Icons.Outlined.Title, tr("見出し", "Heading")) { body.cycleHeading() }
            button(Icons.Outlined.FormatBold, tr("太字", "Bold")) { body.wrap("**") }
            button(Icons.Outlined.FormatItalic, tr("斜体", "Italic")) { body.wrap("*") }
            button(Icons.Outlined.FormatQuote, tr("引用", "Quote")) { body.toggleLinePrefix("> ") }
            button(Icons.Outlined.Image, tr("画像を挿入", "Insert a picture")) { onImage() }
            button(Icons.Outlined.KeyboardHide, tr("キーボードを閉じる", "Hide the keyboard")) { onHideKeyboard() }
        }
    }
}

/** A question to start the day's entry with (one a day, as in the web app). */
@Composable
private fun diaryPrompt(date: String): String {
    val ja = listOf(
        "今日いちばん心が動いたことは？", "今日、誰とどんな話をした？", "今日の自分に点数をつけるなら？ その理由は？",
        "最近気になっていることは？", "今日食べたもので印象に残ったものは？", "うまくいかなかったこと、次に試すことは？",
        "今日、ありがたいと思ったことは？", "明日の自分に伝えたいことは？",
    )
    val en = listOf(
        "What moved you most today?", "Who did you talk with today, and about what?", "How would you score today, and why?",
        "What has been on your mind lately?", "What did you eat today that stayed with you?", "What didn't go well, and what will you try next?",
        "What are you grateful for today?", "What would you tell yourself tomorrow?",
    )
    val i = Math.floorMod(date.hashCode(), ja.size)
    return tr(ja[i], en[i])
}

/** The pictures and files the note has, as thumbnails above its text (a tap opens one). */
@Composable
private fun AttachmentStrip(body: String, files: Map<String, app.nisshoku.android.engine.ResourceFile>) {
    val ids = remember(body) { Regex(""":/([0-9a-f]{32})""").findAll(body).map { it.groupValues[1] }.distinct().toList() }
    val shown = ids.mapNotNull { files[it] }
    if (shown.isEmpty()) return
    val context = androidx.compose.ui.platform.LocalContext.current
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(bottom = 12.dp), horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp)) {
        for (f in shown) {
            val open = {
                runCatching {
                    val file = java.io.File(f.path)
                    val uri = androidx.core.content.FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
                    context.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW).setDataAndType(uri, f.mime).addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION))
                }
            }
            if (f.mime.startsWith("image/")) {
                val bitmap by androidx.compose.runtime.produceState<androidx.compose.ui.graphics.ImageBitmap?>(null, f.path) {
                    value = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
                        runCatching {
                            val o = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
                            android.graphics.BitmapFactory.decodeFile(f.path, o)
                            var sample = 1
                            while (o.outHeight / sample > 480) sample *= 2
                            android.graphics.BitmapFactory.decodeFile(f.path, android.graphics.BitmapFactory.Options().apply { inSampleSize = sample })?.asImageBitmap()
                        }.getOrNull()
                    }
                }
                Box(
                    Modifier.size(140.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(16.dp))
                        .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                        .clickable { open() },
                    contentAlignment = Alignment.Center,
                ) {
                    bitmap?.let { androidx.compose.foundation.Image(it, f.title, contentScale = androidx.compose.ui.layout.ContentScale.Crop, modifier = Modifier.fillMaxSize()) }
                        ?: Text(tr("同期待ち", "Not synced yet"), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
                }
            } else {
                androidx.compose.material3.AssistChip(onClick = { open() }, label = { Text(f.title.ifBlank { tr("添付ファイル", "Attachment") }, maxLines = 1) })
            }
        }
    }
}
