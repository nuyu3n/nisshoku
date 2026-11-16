package app.nisshoku.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
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

// The app's own editor: the note's Markdown in a native text field, so that
// the keyboard (Gboard's Japanese input, its composition, its gestures)
// talks to Android's text input directly, with no web page in between.
// Markdown is styled as it is typed (headings, emphasis, lists, check
// boxes), and stays what is stored. What needs the formatted view (math,
// tables, images) opens in the web editor (Editor.kt), one tap away.

/** Whether a note is better shown in the formatted (web) editor: it has math, a table, an image, an attachment or HTML. */
fun needsRichEditor(body: String): Boolean =
    body.contains("$$") ||
        Regex("""\$[^\s$][^$\n]*\$""").containsMatchIn(body) ||
        body.contains("![") ||
        body.contains("](:/") ||
        Regex("""(?m)^\s*\|.*\|\s*$""").containsMatchIn(body) ||
        Regex("""<[a-zA-Z][^>\n]*>""").containsMatchIn(body)

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
fun NativeEditorScreen(noteId: String, model: AppModel, onClose: () -> Unit, onRich: () -> Unit) {
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
    var startAtTitle by remember { mutableStateOf(false) }
    LaunchedEffect(noteId) {
        val note = try {
            Api.note(noteId)
        } catch (e: Exception) {
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
                // Math, tables, images, the diary's mood: the formatted view.
                IconButton(onClick = { leave(onRich) }) {
                    Icon(Icons.AutoMirrored.Outlined.Article, tr("リッチ表示で開く", "Open the formatted view"))
                }
            },
        )
        Box(Modifier.weight(1f).fillMaxWidth()) {
            if (!loaded) {
                LoadingIndicator(Modifier.align(Alignment.Center))
            } else {
                Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp)) {
                    BasicTextField(
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
                    BasicTextField(
                        state = body,
                        readOnly = readOnly,
                        textStyle = bodyStyle,
                        inputTransformation = continueLists,
                        outputTransformation = markdown,
                        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                        cursorBrush = SolidColor(scheme.primary),
                        onTextLayout = { layout = it },
                        decorator = { inner ->
                            Box {
                                if (body.text.isEmpty()) Text(tr("書きはじめる…", "Start writing…"), style = bodyStyle, color = scheme.outline.copy(alpha = 0.6f))
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
        if (loaded && !readOnly) {
            FormatBar(
                body = body,
                onHideKeyboard = { keyboard?.hide() },
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun FormatBar(body: TextFieldState, onHideKeyboard: () -> Unit) {
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
            button(Icons.Outlined.KeyboardHide, tr("キーボードを閉じる", "Hide the keyboard")) { onHideKeyboard() }
        }
    }
}
