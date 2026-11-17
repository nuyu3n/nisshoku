package app.nisshoku.android.ui

import android.text.format.DateUtils
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.Restore
import androidx.compose.material.icons.outlined.Spellcheck
import androidx.compose.material.icons.outlined.Undo
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.nisshoku.android.engine.Api
import app.nisshoku.android.engine.Comment
import app.nisshoku.android.engine.Revision
import app.nisshoku.android.tr
import app.nisshoku.android.trNow
import kotlinx.coroutines.launch

// The tools of a note in the app's own editor: the headings, the history,
// comments, the writing check. (The vertical preview: Genko.kt.)

// ---------------------------------------------------------------------------
// Headings

data class Heading(val level: Int, val text: String, val offset: Int, val chars: Int)

/** The note's headings (`#` lines outside code), with the characters under each. */
fun headings(body: String): List<Heading> {
    val out = mutableListOf<Heading>()
    var offset = 0
    var fenced = false
    val lines = body.split('\n')
    val counts = IntArray(lines.size)
    for ((i, line) in lines.withIndex()) {
        if (line.trimStart().startsWith("```")) fenced = !fenced
        val m = if (fenced) null else Regex("""^(#{1,6})[ \t]+(.*)""").find(line)
        if (m != null) out += Heading(m.groupValues[1].length, m.groupValues[2].trim(), offset, 0)
        counts[i] = line.count { !it.isWhitespace() }
        offset += line.length + 1
    }
    // Characters up to the next heading of the same level or above.
    return out.mapIndexed { i, h ->
        val end = out.drop(i + 1).firstOrNull { it.level <= h.level }?.offset ?: body.length
        h.copy(chars = body.substring(h.offset, end).count { !it.isWhitespace() })
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OutlineSheet(body: String, onDismiss: () -> Unit, onJump: (Int) -> Unit) {
    val items = remember(body) { headings(body) }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Text(tr("見出し", "Outline"), style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
        if (items.isEmpty()) {
            Text(
                tr("見出しはまだありません。行頭に「# 」を書くと見出しになります。", "No headings yet. Start a line with \"# \" for one."),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 24.dp).padding(bottom = 40.dp),
            )
        }
        LazyColumn(contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            itemsIndexed(items) { i, h ->
                ListItem(
                    headlineContent = { Text(h.text.ifBlank { tr("（無題）", "(untitled)") }, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    trailingContent = { Text(tr("${h.chars}字", "${h.chars}"), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.outline) },
                    colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
                    modifier = Modifier
                        .padding(start = ((h.level - 1) * 16).dp)
                        .clip(groupShape(i, items.size, outer = 20, inner = 4))
                        .clickable { onJump(h.offset) },
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// History

/**
 * The note's earlier versions: the list, one version's text, and going back
 * to it (the version shown now is kept in the history first).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HistorySheet(noteId: String, onDismiss: () -> Unit, onRestored: () -> Unit) {
    val scope = rememberCoroutineScope()
    var list by remember { mutableStateOf<List<Revision>?>(null) }
    var shown by remember { mutableStateOf<Pair<Revision, String>?>(null) }
    LaunchedEffect(noteId) {
        runCatching { Api.saveSnapshot(noteId) }
        list = runCatching { Api.revisions(noteId) }.getOrDefault(emptyList())
    }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        val s = shown
        if (s == null) {
            Text(tr("履歴", "History"), style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            val l = list
            if (l != null && l.isEmpty()) {
                Text(
                    tr("まだ履歴はありません。書き進めると、10分ほどごとに自動で残ります。", "No history yet. Versions are kept every ten minutes or so as you write."),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 24.dp).padding(bottom = 40.dp),
                )
            }
            LazyColumn(contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                itemsIndexed(l.orEmpty()) { i, r ->
                    ListItem(
                        headlineContent = { Text(DateUtils.formatDateTime(app.nisshoku.android.engine.Nisshoku.context, r.time, DateUtils.FORMAT_SHOW_DATE or DateUtils.FORMAT_SHOW_TIME or DateUtils.FORMAT_ABBREV_MONTH)) },
                        supportingContent = { Text(DateUtils.getRelativeTimeSpanString(r.time, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS).toString()) },
                        leadingContent = { Icon(Icons.Outlined.History, null) },
                        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
                        modifier = Modifier.clip(groupShape(i, l!!.size, outer = 20, inner = 4)).clickable {
                            scope.launch {
                                val c = runCatching { Api.revisionContent(noteId, r.id) }.getOrNull() ?: return@launch
                                shown = r to (if (c.title.isNotBlank()) "${c.title}\n\n${c.body}" else c.body)
                            }
                        },
                    )
                }
            }
        } else {
            Row(Modifier.padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                TextButton(onClick = { shown = null }) { Text(tr("‹ 一覧", "‹ List")) }
                Spacer(Modifier.weight(1f))
                Button(onClick = {
                    scope.launch {
                        runCatching { Api.restoreRevision(noteId, s.first.id) }
                            .onSuccess { onRestored(); onDismiss() }
                    }
                }) {
                    Icon(Icons.Outlined.Restore, null)
                    Spacer(Modifier.width(6.dp))
                    Text(tr("この版に戻す", "Restore this version"))
                }
            }
            Text(
                DateUtils.formatDateTime(app.nisshoku.android.engine.Nisshoku.context, s.first.time, DateUtils.FORMAT_SHOW_DATE or DateUtils.FORMAT_SHOW_TIME),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp),
            )
            Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceContainerHigh, modifier = Modifier.padding(horizontal = 16.dp).padding(bottom = 24.dp)) {
                Text(
                    s.second,
                    style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Serif),
                    modifier = Modifier.heightIn(max = 520.dp).verticalScroll(rememberScrollState()).padding(16.dp).fillMaxWidth(),
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Comments

/** Where a comment's quote is in the text now: by its quote, the closest match of the text before it. */
fun commentRange(text: String, c: Comment): IntRange? {
    if (c.quote.isEmpty()) return null
    var best: IntRange? = null
    var bestScore = -1
    var i = text.indexOf(c.quote)
    while (i >= 0) {
        var score = 0
        while (score < c.before.length && i - 1 - score >= 0 && text[i - 1 - score] == c.before[c.before.length - 1 - score]) score++
        if (score > bestScore) {
            bestScore = score
            best = i until i + c.quote.length
        }
        if (score == c.before.length) break
        i = text.indexOf(c.quote, i + 1)
    }
    return best
}

/**
 * The note's comments: each with the words it is on (a tap shows them in
 * the text), done or deleted; and a new one on the words selected.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CommentsSheet(
    noteId: String,
    text: String,
    selection: IntRange?,
    onDismiss: () -> Unit,
    onShow: (IntRange) -> Unit,
) {
    val scope = rememberCoroutineScope()
    var list by remember { mutableStateOf<List<Comment>>(emptyList()) }
    var draft by remember { mutableStateOf("") }
    var showDone by remember { mutableStateOf(false) }
    suspend fun load() {
        list = runCatching { Api.comments(noteId) }.getOrDefault(emptyList())
    }
    LaunchedEffect(noteId) { load() }
    val quote = selection?.let { text.substring(it.first, it.last + 1) }?.takeIf { it.isNotBlank() }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(tr("コメント", "Comments"), style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(start = 8.dp))
            if (quote != null) {
                Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.primaryContainer) {
                    Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text("“${quote.take(120)}”", style = MaterialTheme.typography.bodyMedium, fontStyle = FontStyle.Italic, maxLines = 3, overflow = TextOverflow.Ellipsis)
                        OutlinedTextField(value = draft, onValueChange = { draft = it }, placeholder = { Text(tr("コメントを書く", "Write a comment")) }, minLines = 2, modifier = Modifier.fillMaxWidth())
                        Button(
                            enabled = draft.isNotBlank(),
                            onClick = {
                                val sel = selection!!
                                scope.launch {
                                    runCatching { Api.addComment(noteId, quote, text.substring(maxOf(0, sel.first - 16), sel.first), draft) }
                                    draft = ""
                                    load()
                                }
                            },
                            modifier = Modifier.align(Alignment.End),
                        ) { Text(tr("追加", "Add")) }
                    }
                }
            } else {
                Text(
                    tr("文字を選んでからここを開くと、その部分にコメントを付けられます。", "Select some text first to comment on it."),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 8.dp),
                )
            }
            val open = list.filter { !it.resolved }
            val done = list.filter { it.resolved }
            if (list.isEmpty()) {
                Text(tr("コメントはまだありません", "No comments yet"), color = MaterialTheme.colorScheme.outline, modifier = Modifier.padding(8.dp))
            }
            Column(Modifier.heightIn(max = 460.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                (open + if (showDone) done else emptyList()).forEachIndexed { i, c ->
                    val range = commentRange(text, c)
                    Surface(
                        shape = groupShape(i, open.size + if (showDone) done.size else 0, outer = 20, inner = 4),
                        color = MaterialTheme.colorScheme.surfaceContainerHigh,
                        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(20.dp)).clickable(enabled = range != null) { range?.let(onShow); onDismiss() },
                    ) {
                        Column(Modifier.padding(start = 16.dp, top = 12.dp, bottom = 4.dp, end = 4.dp)) {
                            Text(
                                "“${c.quote.take(80)}”" + if (range == null) tr("（本文にありません）", " (no longer in the text)") else "",
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.primary,
                                maxLines = 2,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(c.text, style = MaterialTheme.typography.bodyMedium, color = if (c.resolved) MaterialTheme.colorScheme.outline else MaterialTheme.colorScheme.onSurface)
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text(
                                    DateUtils.getRelativeTimeSpanString(c.createdTime, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS).toString(),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.outline,
                                    modifier = Modifier.weight(1f),
                                )
                                IconButton(onClick = { scope.launch { runCatching { Api.updateComment(noteId, c.id, null, !c.resolved) }; load() } }) {
                                    Icon(if (c.resolved) Icons.Outlined.Undo else Icons.Outlined.CheckCircle, if (c.resolved) tr("未解決に戻す", "Reopen") else tr("解決", "Resolve"))
                                }
                                IconButton(onClick = { scope.launch { runCatching { Api.deleteComment(noteId, c.id) }; load() } }) {
                                    Icon(Icons.Outlined.Delete, tr("削除", "Delete"))
                                }
                            }
                        }
                    }
                }
            }
            if (done.isNotEmpty()) {
                TextButton(onClick = { showDone = !showDone }) {
                    Text(if (showDone) tr("解決済みを隠す", "Hide resolved") else tr("解決済み ${done.size} 件を表示", "Show ${done.size} resolved"))
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The writing check (lib/proof.ts of the web app)

data class Issue(val message: String, val excerpt: String, val from: Int, val to: Int)

data class Measures(val sentences: Int, val average: Int, val longest: Int, val kanjiRatio: Int, val paragraphs: Int)

private val VARIANTS = listOf(
    "出来る" to "できる", "下さい" to "ください", "全て" to "すべて", "沢山" to "たくさん", "様々" to "さまざま",
    "子供" to "子ども", "私達" to "私たち", "一つ" to "ひとつ", "既に" to "すでに", "更に" to "さらに",
    "殆ど" to "ほとんど", "何故" to "なぜ", "頂く" to "いただく", "事が出来" to "ことができ", "如何" to "いかが", "又は" to "または",
)
private val RANUKI = Regex("(見|来|寝|着|居|出|起き|食べ|考え|決め|忘れ|信じ|生き|借り|降り)れ(る|ない|ます|た|て|れば)")
private val REPEATED = Regex("""([\p{IsHan}\p{IsKatakana}ー]{2,}|[ぁ-ん]{3,})\1""")
private val PARTICLES = Regex("([がをにはへでとのも])\\1(?![ぁ-ん])")

/** Long sentences, runs of one ending, ら抜き, many commas, doubled words, mixed spellings. */
fun checkWriting(body: String, maxSentence: Int = 80): Pair<List<Issue>, Measures> {
    val issues = mutableListOf<Issue>()
    var count = 0
    var total = 0
    var longest = 0
    var kanji = 0
    var letters = 0
    var paragraphs = 0
    data class End(val ending: String, val from: Int, val to: Int, val text: String)
    val endings = mutableListOf<End>()
    var lineStart = 0
    for (line in body.split('\n')) {
        // The text of the line without its Markdown marks (offsets stay the line's).
        val skip = Regex("""^(\s*(#{1,6}|[-*+]|\d+[.)]|>)[ \t]+(\[[ xX]\][ \t]+)?)""").find(line)?.value?.length ?: 0
        val text = line.substring(skip)
        val base = lineStart + skip
        if (text.isNotBlank()) paragraphs++
        for (ch in text) {
            if (Character.UnicodeScript.of(ch.code) == Character.UnicodeScript.HAN) kanji++
            if (ch.isLetter()) letters++
        }
        var start = 0
        var i = 0
        val sentences = mutableListOf<Pair<String, Int>>()
        while (i < text.length) {
            if (text[i] in "。！？!?") {
                var end = i + 1
                while (end < text.length && text[end] in "」』）)") end++
                sentences += text.substring(start, end) to start
                start = end
                i = end
                continue
            }
            i++
        }
        if (start < text.length && text.substring(start).isNotBlank()) sentences += text.substring(start) to start
        for ((s, at) in sentences) {
            val body2 = s.trim()
            if (body2.isEmpty()) continue
            val len = body2.codePointCount(0, body2.length)
            count++
            total += len
            longest = maxOf(longest, len)
            if (len > maxSentence) issues += Issue(trNow("文が長めです（${len}字）", "Long sentence ($len characters)"), body2, base + at, base + at + s.length)
            val commas = body2.count { it in "、，," }
            if (commas >= 5) issues += Issue(trNow("読点が多めです（${commas}個）", "Many commas ($commas)"), body2, base + at, base + at + s.length)
            val ending = body2.trimEnd { it in "。！？!?」』）) \t" }.takeLast(1)
            if (ending.isNotEmpty()) endings += End(ending, base + at, base + at + s.length, body2)
        }
        for (m in RANUKI.findAll(text)) {
            issues += Issue(trNow("ら抜き言葉かもしれません（「${m.value}」→「${m.groupValues[1]}られ${m.groupValues[2]}」）", "Possible ら-dropping: ${m.value}"), m.value, base + m.range.first, base + m.range.last + 1)
        }
        for (m in REPEATED.findAll(text)) {
            issues += Issue(trNow("同じ言葉が続いています（「${m.groupValues[1]}」）", "Repeated word: ${m.groupValues[1]}"), m.value, base + m.range.first, base + m.range.last + 1)
        }
        for (m in PARTICLES.findAll(text)) {
            issues += Issue(trNow("助詞が重なっています（「${m.value}」）", "Doubled particle: ${m.value}"), m.value, base + m.range.first, base + m.range.last + 1)
        }
        lineStart += line.length + 1
    }
    var run = mutableListOf<End>()
    fun flush() {
        if (run.size >= 3) {
            val f = run.first()
            issues += Issue(trNow("文末が「${f.ending}」で${run.size}回続いています", "${run.size} sentences in a row end with “${f.ending}”"), run.joinToString(" ") { it.text }, f.from, f.to)
        }
        run = mutableListOf()
    }
    for (e in endings) {
        if (run.isNotEmpty() && run.last().ending != e.ending) flush()
        run += e
    }
    flush()
    for ((a, b) in VARIANTS) {
        if (body.contains(a) && body.contains(b)) {
            val at = body.indexOf(a)
            issues += Issue(trNow("表記ゆれ：「$a」と「$b」が混在しています", "Inconsistent spelling: $a / $b"), a, at, at + a.length)
        }
    }
    issues.sortBy { it.from }
    return issues to Measures(count, if (count > 0) total / count else 0, longest, if (letters > 0) kanji * 100 / letters else 0, paragraphs)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProofSheet(body: String, onDismiss: () -> Unit, onShow: (IntRange) -> Unit) {
    val (issues, m) = remember(body) { checkWriting(body) }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(tr("文章チェック", "Writing check"), style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(start = 8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                StatTile(tr("文", "Sentences"), "${m.sentences}", Modifier.weight(1f))
                StatTile(tr("平均", "Average"), tr("${m.average}字", "${m.average}"), Modifier.weight(1f))
                StatTile(tr("最長", "Longest"), tr("${m.longest}字", "${m.longest}"), Modifier.weight(1f))
                StatTile(tr("漢字", "Kanji"), "${m.kanjiRatio}%", Modifier.weight(1f))
            }
            if (issues.isEmpty()) {
                Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Outlined.Spellcheck, null, tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(10.dp))
                    Text(tr("気になるところは見つかりませんでした", "Nothing to point out"))
                }
            }
            Column(Modifier.heightIn(max = 480.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                issues.forEachIndexed { i, issue ->
                    ListItem(
                        headlineContent = { Text(issue.message) },
                        supportingContent = { Text(issue.excerpt, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
                        modifier = Modifier.clip(groupShape(i, issues.size, outer = 20, inner = 4)).clickable {
                            onShow(issue.from until issue.to)
                            onDismiss()
                        },
                    )
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Conflicts

/**
 * A conflict copy: the version this device had when another device changed
 * the note. Keep it (it replaces the note) or the note as it is.
 */
@Composable
fun ConflictBanner(onKeepCopy: () -> Unit, onKeepOriginal: () -> Unit) {
    var confirm by remember { mutableStateOf<String?>(null) }
    Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.errorContainer, modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(
                tr("これは競合コピーです。ほかの端末とこの端末で同時に書き換えたときに、この端末の版を残したものです。", "This is a conflict copy: this device's version, kept when the note was changed on two devices at once."),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onErrorContainer,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilledTonalButton(onClick = { confirm = "copy" }) { Text(tr("こちらを残す", "Keep this one")) }
                TextButton(onClick = { confirm = "original" }) { Text(tr("元のノートを残す", "Keep the original")) }
            }
        }
    }
    confirm?.let { keep ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            title = { Text(if (keep == "copy") tr("この版を残しますか？", "Keep this version?") else tr("元のノートを残しますか？", "Keep the original?")) },
            text = {
                Text(
                    if (keep == "copy") tr("元のノートの内容がこの版で置き換わり、競合コピーは消えます（元の内容は履歴に残ります）。", "The note takes this version's text and the conflict copy goes (the old text stays in the history).")
                    else tr("この競合コピーをゴミ箱に移します。", "This conflict copy goes to the trash."),
                )
            },
            confirmButton = { Button(onClick = { confirm = null; if (keep == "copy") onKeepCopy() else onKeepOriginal() }) { Text(tr("OK", "OK")) } },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text(tr("キャンセル", "Cancel")) } },
        )
    }
}
