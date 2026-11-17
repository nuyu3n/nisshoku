package app.nisshoku.android.ui

import android.content.Context
import android.content.Intent
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.AutoStories
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.LocalFireDepartment
import androidx.compose.material.icons.outlined.Tag
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.InputChip
import androidx.compose.material3.LinearWavyProgressIndicator
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
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.nisshoku.android.engine.Api
import app.nisshoku.android.engine.DiaryEntry
import app.nisshoku.android.engine.DiaryStats
import app.nisshoku.android.engine.Folder
import app.nisshoku.android.engine.LinkedNote
import app.nisshoku.android.engine.NoteSummary
import app.nisshoku.android.engine.Stats
import app.nisshoku.android.engine.Tag
import app.nisshoku.android.tr
import app.nisshoku.android.trNow
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.ChronoUnit

// ---------------------------------------------------------------------------
// Tasks

/** The groups of the task list, in order. */
enum class TaskGroup { Overdue, Today, Upcoming, Someday, Done }

@Composable
fun TaskGroup.label(): String = when (this) {
    TaskGroup.Overdue -> tr("期限切れ", "Overdue")
    TaskGroup.Today -> tr("今日", "Today")
    TaskGroup.Upcoming -> tr("これから", "Upcoming")
    TaskGroup.Someday -> tr("期限なし", "No due date")
    TaskGroup.Done -> tr("完了", "Done")
}

/** Tasks by group (as the web app's `groupTasks`). */
fun groupTasks(tasks: List<NoteSummary>): Map<TaskGroup, List<NoteSummary>> {
    val today = LocalDate.now()
    fun day(ms: Long) = Instant.ofEpochMilli(ms).atZone(ZoneId.systemDefault()).toLocalDate()
    val groups = tasks.groupBy { t ->
        when {
            t.todoCompleted -> TaskGroup.Done
            t.todoDue <= 0 -> TaskGroup.Someday
            overdue(t.todoDue, t.dueAllDay) -> TaskGroup.Overdue
            ChronoUnit.DAYS.between(today, day(t.todoDue)) == 0L -> TaskGroup.Today
            else -> TaskGroup.Upcoming
        }
    }
    val byDue = compareBy<NoteSummary> { it.todoDue }.thenBy { it.dueAllDay }
    return TaskGroup.entries.associateWith { g ->
        val list = groups[g].orEmpty()
        when (g) {
            TaskGroup.Overdue, TaskGroup.Upcoming -> list.sortedWith(byDue)
            TaskGroup.Today -> list.sortedWith(compareByDescending<NoteSummary> { it.dueAllDay }.thenBy { it.todoDue })
            TaskGroup.Someday -> list.sortedWith(compareByDescending<NoteSummary> { it.order }.thenByDescending { it.createdTime })
            TaskGroup.Done -> list.sortedByDescending { it.completedTime }
        }
    }.filterValues { it.isNotEmpty() }
}

// ---------------------------------------------------------------------------
// Today's writing (the drawer's head)

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun TodayCard(stats: Stats, goal: Int, onDiary: () -> Unit) {
    Surface(
        shape = RoundedCornerShape(24.dp),
        color = MaterialTheme.colorScheme.primaryContainer,
        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(24.dp)).clickable(onClick = onDiary),
    ) {
        Column(Modifier.padding(18.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(tr("今日の執筆", "Written today"), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onPrimaryContainer)
            Row(verticalAlignment = Alignment.Bottom) {
                Text("%,d".format(stats.today), style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.onPrimaryContainer)
                Spacer(Modifier.width(6.dp))
                Text(tr("字", "chars"), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onPrimaryContainer, modifier = Modifier.padding(bottom = 5.dp))
                Spacer(Modifier.weight(1f))
                if (stats.streak > 0) {
                    Icon(Icons.Outlined.LocalFireDepartment, null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(18.dp).padding(bottom = 2.dp))
                    Text(tr("${stats.streak}日連続", "${stats.streak}-day streak"), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(bottom = 5.dp))
                }
            }
            if (goal > 0) {
                LinearWavyProgressIndicator(
                    progress = { (stats.today.toFloat() / goal).coerceIn(0f, 1f) },
                    modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
                )
                Text(
                    if (stats.today >= goal) tr("目標を達成しました", "Goal reached") else tr("目標まであと ${"%,d".format(goal - stats.today)}字", "${"%,d".format(goal - stats.today)} to go"),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The diary's figures, and the same day in earlier years

@Composable
fun DiaryHeader(version: Int, onOpen: (String) -> Unit) {
    var stats by remember { mutableStateOf<DiaryStats?>(null) }
    var past by remember { mutableStateOf<List<DiaryEntry>>(emptyList()) }
    LaunchedEffect(version) {
        val today = LocalDate.now().toString()
        stats = runCatching { Api.diaryStats(today) }.getOrNull()
        past = runCatching { Api.onThisDay(today) }.getOrDefault(emptyList())
    }
    Column(Modifier.animateContentSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        stats?.takeIf { it.totalDays > 0 }?.let { s ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                StatTile(tr("連続", "Streak"), tr("${s.streak}日", "${s.streak}d"), Modifier.weight(1f))
                StatTile(tr("今月", "This month"), tr("${s.monthDays}日", "${s.monthDays}d"), Modifier.weight(1f))
                StatTile(tr("合計", "In all"), tr("${s.totalDays}日", "${s.totalDays}d"), Modifier.weight(1f))
            }
        }
        if (past.isNotEmpty()) {
            Text(tr("過去のこの日", "On this day"), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = 8.dp, top = 4.dp))
            past.forEachIndexed { i, e ->
                ListItem(
                    overlineContent = { Text(e.date) },
                    headlineContent = { Text(e.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = e.excerpt.takeIf { it.isNotBlank() }?.let { { Text(it, maxLines = 2, overflow = TextOverflow.Ellipsis) } },
                    leadingContent = { Icon(Icons.Outlined.AutoStories, null) },
                    colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
                    modifier = Modifier.clip(groupShape(i, past.size)).clickable { onOpen(e.noteId) },
                )
            }
        }
    }
}

@Composable
fun StatTile(label: String, value: String, modifier: Modifier = Modifier) {
    Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceContainerLow, modifier = modifier) {
        Column(Modifier.padding(horizontal = 14.dp, vertical = 12.dp)) {
            Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(value, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
        }
    }
}

// ---------------------------------------------------------------------------
// A notebook's sub-notebooks, above its notes

@Composable
fun SubNotebooks(children: List<Folder>, onOpen: (Folder) -> Unit) {
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        children.forEach { f ->
            AssistChip(
                onClick = { onOpen(f) },
                label = { Text(f.title) },
                leadingIcon = { FolderMark(f.icon, f.color, Icons.Outlined.Folder) },
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Sheets and dialogs

/** The note's tags: chips to take away, a field to add, the existing tags to pick. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TagsSheet(current: List<String>, all: List<Tag>, onDismiss: () -> Unit, onSave: (List<String>) -> Unit) {
    val tags = remember { mutableStateListOf(*current.toTypedArray()) }
    var text by remember { mutableStateOf("") }
    val focus = remember { FocusRequester() }
    fun add() {
        text.split(',', '、', '，').map { it.trim().removePrefix("#") }.filter { it.isNotEmpty() && it !in tags }.forEach { tags += it }
        text = ""
    }
    ModalBottomSheet(onDismissRequest = { add(); onSave(tags.toList()); onDismiss() }, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.padding(horizontal = 20.dp).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(tr("タグ", "Tags"), style = MaterialTheme.typography.titleLarge)
            if (tags.isNotEmpty()) {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    tags.forEach { t ->
                        InputChip(
                            selected = true,
                            onClick = { tags.remove(t) },
                            label = { Text(t) },
                            trailingIcon = { Icon(Icons.Outlined.Close, tr("外す", "Remove"), Modifier.size(16.dp)) },
                        )
                    }
                }
            }
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                placeholder = { Text(tr("タグを追加", "Add a tag")) },
                leadingIcon = { Icon(Icons.Outlined.Tag, null) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { add() }),
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
            )
            val others = all.filter { it.title !in tags && (text.isBlank() || it.title.contains(text.trim(), ignoreCase = true)) }
            if (others.isNotEmpty()) {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    others.take(20).forEach { t ->
                        FilterChip(selected = false, onClick = { tags += t.title; text = "" }, label = { Text(t.title) }, leadingIcon = { Icon(Icons.Outlined.Add, null, Modifier.size(16.dp)) })
                    }
                }
            }
            Button(onClick = { add(); onSave(tags.toList()); onDismiss() }, modifier = Modifier.fillMaxWidth().height(52.dp)) { Text(tr("保存", "Save")) }
        }
    }
}

/** The templates to start a note from. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TemplateSheet(onDismiss: () -> Unit, onPick: (LinkedNote) -> Unit) {
    var templates by remember { mutableStateOf<List<LinkedNote>?>(null) }
    LaunchedEffect(Unit) { templates = runCatching { Api.templates() }.getOrDefault(emptyList()) }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Text(tr("テンプレートから作成", "New from a template"), style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
        val list = templates
        when {
            list == null -> Spacer(Modifier.height(120.dp))
            list.isEmpty() -> Text(
                tr("テンプレートはまだありません。ノートを長押しして「テンプレートとして保存」で作れます。", "No templates yet. Long-press a note and choose \"Save as template\"."),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 24.dp).padding(bottom = 40.dp),
            )
            else -> LazyColumn(contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                itemsIndexed(list, key = { _, t -> t.id }) { i, t ->
                    ListItem(
                        headlineContent = { Text(t.title.ifBlank { tr("無題", "Untitled") }) },
                        leadingContent = { Icon(Icons.Outlined.Description, null) },
                        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
                        modifier = Modifier.clip(groupShape(i, list.size, outer = 20, inner = 4)).clickable { onPick(t) },
                    )
                }
            }
        }
    }
}

/** Asks for a web page to save as a note. */
@Composable
fun ClipDialog(onDismiss: () -> Unit, onClip: (String) -> Unit) {
    var url by remember { mutableStateOf("https://") }
    val focus = remember { FocusRequester() }
    val ok = url.trim().let { it.startsWith("http://") || it.startsWith("https://") } && url.trim().length > 10
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("Web ページを取り込む", "Clip a web page")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(tr("ページの本文をノートとして保存します。", "Saves the page's text as a note."), style = MaterialTheme.typography.bodyMedium)
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
                    keyboardActions = KeyboardActions(onGo = { if (ok) onClip(url) }),
                    modifier = Modifier.fillMaxWidth().focusRequester(focus),
                )
            }
        },
        confirmButton = { Button(onClick = { onClip(url) }, enabled = ok) { Text(tr("取り込む", "Clip")) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("キャンセル", "Cancel")) } },
    )
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
}

/** "Are you sure?" for what cannot be undone. */
@Composable
fun ConfirmDialog(title: String, text: String, confirm: String, onDismiss: () -> Unit, onConfirm: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { Text(text) },
        confirmButton = {
            Button(
                onClick = { onConfirm(); onDismiss() },
                colors = androidx.compose.material3.ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error, contentColor = MaterialTheme.colorScheme.onError),
            ) { Text(confirm) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("キャンセル", "Cancel")) } },
    )
}

/** Sends a note's text to another app (the system's share sheet). */
suspend fun shareNote(context: Context, id: String) {
    val note = runCatching { Api.note(id) }.getOrNull() ?: return
    val send = Intent(Intent.ACTION_SEND)
        .setType("text/plain")
        .putExtra(Intent.EXTRA_SUBJECT, note.title)
        .putExtra(Intent.EXTRA_TEXT, if (note.title.isBlank()) note.body else "${note.title}\n\n${note.body}")
    context.startActivity(Intent.createChooser(send, trNow("共有", "Share")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}
