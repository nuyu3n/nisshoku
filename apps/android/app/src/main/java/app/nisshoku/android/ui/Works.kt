package app.nisshoku.android.ui

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.Notes
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.Inventory2
import androidx.compose.material.icons.outlined.People
import androidx.compose.material.icons.outlined.RadioButtonUnchecked
import androidx.compose.material.icons.outlined.Timeline
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeFlexibleTopAppBar
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.nisshoku.android.engine.Api
import app.nisshoku.android.engine.Character
import app.nisshoku.android.engine.ChapterInfo
import app.nisshoku.android.engine.Folder
import app.nisshoku.android.engine.Nisshoku
import app.nisshoku.android.engine.Plot
import app.nisshoku.android.engine.PlotCard
import app.nisshoku.android.engine.WorkMeta
import app.nisshoku.android.engine.WorkOverview
import app.nisshoku.android.tr
import app.nisshoku.android.trNow
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch

// Works (a novel…): a notebook marked as a work, its notes a manuscript
// (chapters), characters, a plot board and materials.

/** The work a notebook belongs to (itself or the nearest notebook above marked as one). */
fun workOf(folders: List<Folder>, folderId: String): Folder? {
    var f = folders.find { it.id == folderId }
    val seen = mutableSetOf<String>()
    while (f != null && seen.add(f.id)) {
        if (f.isWork) return f
        f = folders.find { it.id == f!!.parentId }
    }
    return null
}

/** Statuses of a chapter: label and colour. */
@Composable
fun statusLabel(status: String): Pair<String, Color> = when (status) {
    "idea" -> tr("構想", "Idea") to Color(0xFF9B958B)
    "draft" -> tr("下書き", "Draft") to Color(0xFFE2A33C)
    "revise" -> tr("推敲", "Revising") to Color(0xFF6F94B8)
    "done" -> tr("完成", "Done") to Color(0xFF5E8C4A)
    else -> tr("未設定", "No status") to MaterialTheme.colorScheme.outline
}

val STATUSES = listOf("", "idea", "draft", "revise", "done")

@Composable
fun roleLabel(role: String): String = when (role) {
    "protagonist" -> tr("主人公", "Protagonist")
    "antagonist" -> tr("敵役", "Antagonist")
    "main" -> tr("主要人物", "Main")
    "supporting" -> tr("脇役", "Supporting")
    "minor" -> tr("端役", "Minor")
    else -> tr("未設定", "No role")
}

val ROLES = listOf("protagonist", "antagonist", "main", "supporting", "minor", "")

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun WorkScreen(workId: String, model: AppModel, onOpen: (String) -> Unit, onBack: () -> Unit) {
    val scope = rememberCoroutineScope()
    val home by model.home.collectAsState()
    var work by remember { mutableStateOf<WorkOverview?>(null) }
    var tab by rememberSaveable { mutableStateOf(0) }
    var editing by remember { mutableStateOf(false) }
    var asking by remember { mutableStateOf<String?>(null) }
    suspend fun load() {
        work = runCatching { Api.workOverview(workId) }.getOrNull()
    }
    LaunchedEffect(workId, home.version) { load() }
    val bar = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    Scaffold(
        modifier = Modifier.nestedScroll(bar.nestedScrollConnection),
        topBar = {
            Column {
                LargeFlexibleTopAppBar(
                    title = { Text(work?.title ?: "", maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    subtitle = { work?.meta?.logline?.takeIf { it.isNotBlank() }?.let { Text(it, maxLines = 1, overflow = TextOverflow.Ellipsis) } },
                    navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, tr("戻る", "Back")) } },
                    actions = { IconButton(onClick = { editing = true }) { Icon(Icons.Outlined.Edit, tr("作品の情報", "About the work")) } },
                    scrollBehavior = bar,
                )
                PrimaryTabRow(selectedTabIndex = tab) {
                    listOf(
                        tr("原稿", "Manuscript") to Icons.AutoMirrored.Outlined.Notes,
                        tr("人物", "Characters") to Icons.Outlined.People,
                        tr("プロット", "Plot") to Icons.Outlined.Timeline,
                        tr("資料", "Materials") to Icons.Outlined.Inventory2,
                    ).forEachIndexed { i, (label, icon) ->
                        Tab(selected = tab == i, onClick = { tab = i }, text = { Text(label) }, icon = { Icon(icon, null) })
                    }
                }
            }
        },
        floatingActionButton = {
            if (tab != 2) {
                FloatingActionButton(onClick = { asking = listOf("chapter", "character", "", "material")[tab] }) {
                    Icon(Icons.Outlined.Add, tr("追加", "Add"))
                }
            }
        },
    ) { padding ->
        val w = work
        if (w == null) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) { LoadingIndicator() }
            return@Scaffold
        }
        Box(Modifier.fillMaxSize().padding(padding)) {
            when (tab) {
                0 -> Manuscript(w, onOpen) { id, status -> scope.launch { setStatus(id, status); load() } }
                1 -> Characters(w, onOpen)
                2 -> PlotBoard(workId, w)
                else -> Materials(w, onOpen)
            }
        }
    }
    if (editing) {
        work?.let { w ->
            WorkInfoDialog(w.meta, onDismiss = { editing = false }) { meta ->
                editing = false
                scope.launch {
                    runCatching { Api.setWork(workId, meta) }
                    load()
                }
            }
        }
    }
    asking?.let { kind ->
        TextPrompt(
            title = when (kind) {
                "chapter" -> tr("新しい章", "New chapter")
                "character" -> tr("新しい人物", "New character")
                else -> tr("新しい資料", "New material")
            },
            label = if (kind == "character") tr("名前", "Name") else tr("タイトル", "Title"),
            confirm = tr("作成", "Create"),
            onDismiss = { asking = null },
            onDone = { title ->
                asking = null
                scope.launch {
                    val id = runCatching {
                        if (kind == "chapter") {
                            // A chapter goes in the last section (the work's own notebook if it has none).
                            val folder = work?.sections?.lastOrNull()?.folderId ?: workId
                            Api.createNote(folder, title, "").id
                        } else Api.createWorkNote(workId, kind, title).id
                    }.getOrNull()
                    model.reload()
                    load()
                    id?.let(onOpen)
                }
            },
        )
    }
}

private suspend fun setStatus(noteId: String, status: String) {
    // The chapter's synopsis and point of view are kept.
    val w = runCatching { Api.note(noteId) }.getOrNull() ?: return
    val info = runCatching { Api.workOverview(w.parentId) }.getOrNull()?.sections?.flatMap { it.chapters }?.find { it.id == noteId }
    runCatching { Api.setChapterMeta(noteId, status, info?.synopsis ?: "", info?.pov) }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Manuscript(work: WorkOverview, onOpen: (String) -> Unit, onStatus: (String, String) -> Unit) {
    LazyColumn(contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = 8.dp, bottom = 120.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
        item {
            Surface(shape = RoundedCornerShape(24.dp), color = MaterialTheme.colorScheme.primaryContainer, modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp)) {
                Column(Modifier.padding(18.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Row(verticalAlignment = Alignment.Bottom) {
                        Text("%,d".format(work.totalChars), style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold)
                        Text(tr(" 字・${work.chapterCount} 章・原稿用紙 ${(work.totalChars + 399) / 400} 枚", " chars · ${work.chapterCount} chapters · ${(work.totalChars + 399) / 400} pages"), modifier = Modifier.padding(bottom = 5.dp))
                    }
                    work.meta.targetChars?.takeIf { it > 0 }?.let { target ->
                        LinearWavyProgressIndicator(progress = { (work.totalChars.toFloat() / target).coerceIn(0f, 1f) }, modifier = Modifier.fillMaxWidth())
                        Text(tr("目標 ${"%,d".format(target)}字", "Goal ${"%,d".format(target)}") + (work.meta.deadline?.let { tr("・締切 $it", " · due $it") } ?: ""), style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
        }
        for (section in work.sections) {
            if (work.sections.size > 1 || section.folderId != work.id) {
                item(key = "s-" + section.folderId) { SectionLabel(section.title, Modifier.padding(start = (section.depth * 12).dp)) }
            }
            if (section.chapters.isEmpty()) {
                item(key = "e-" + section.folderId) {
                    Text(tr("章はまだありません。＋ から書き始めましょう。", "No chapters yet. Start one with +."), color = MaterialTheme.colorScheme.outline, modifier = Modifier.padding(16.dp))
                }
            }
            itemsIndexed(section.chapters, key = { _, c -> c.id }) { i, c ->
                ChapterRow(i + 1, c, groupShape(i, section.chapters.size), onOpen, onStatus)
            }
        }
    }
}

@Composable
private fun ChapterRow(n: Int, c: ChapterInfo, shape: androidx.compose.ui.graphics.Shape, onOpen: (String) -> Unit, onStatus: (String, String) -> Unit) {
    var menu by remember { mutableStateOf(false) }
    val (label, color) = statusLabel(c.status)
    ListItem(
        overlineContent = { Text(tr("第${n}章", "Chapter $n")) },
        headlineContent = { Text(c.title.ifBlank { tr("無題", "Untitled") }, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.titleMedium.copy(fontFamily = androidx.compose.ui.text.font.FontFamily.Serif)) },
        supportingContent = {
            Text((c.synopsis.ifBlank { c.excerpt }).take(80), maxLines = 2, overflow = TextOverflow.Ellipsis)
        },
        trailingContent = {
            Column(horizontalAlignment = Alignment.End) {
                Box {
                    AssistChip(
                        onClick = { menu = true },
                        label = { Text(label) },
                        leadingIcon = { Box(Modifier.size(8.dp).clip(CircleShape).background(color)) },
                    )
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        STATUSES.forEach { s ->
                            val (l, col) = statusLabel(s)
                            DropdownMenuItem(text = { Text(l) }, leadingIcon = { Box(Modifier.size(10.dp).clip(CircleShape).background(col)) }, onClick = { menu = false; onStatus(c.id, s) })
                        }
                    }
                }
                Text(tr("${c.charCount}字", "${c.charCount}"), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
            }
        },
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
        modifier = Modifier.clip(shape).clickable { onOpen(c.id) },
    )
}

@Composable
private fun Characters(work: WorkOverview, onOpen: (String) -> Unit) {
    LazyColumn(contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = 8.dp, bottom = 120.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
        if (work.characters.isEmpty()) {
            item { EmptyState(Icons.Outlined.People, tr("人物はまだいません", "No characters yet"), tr("＋ から登場人物を追加できます", "Add one with +")) }
        }
        itemsIndexed(work.characters, key = { _, c -> c.id }) { i, c ->
            ListItem(
                headlineContent = { Text(c.name.ifBlank { tr("無名", "Unnamed") } + if (c.ruby.isNotBlank()) "（${c.ruby}）" else "") },
                supportingContent = { Text(listOfNotNull(roleLabel(c.role), c.excerpt.takeIf { it.isNotBlank() }).joinToString("・"), maxLines = 2, overflow = TextOverflow.Ellipsis) },
                leadingContent = {
                    Box(Modifier.size(40.dp).clip(CircleShape).background(parseColor(c.color) ?: MaterialTheme.colorScheme.secondaryContainer), contentAlignment = Alignment.Center) {
                        Text(c.name.take(1), color = Color.White, fontWeight = FontWeight.Bold)
                    }
                },
                trailingContent = { if (c.mentions > 0) Text(tr("${c.mentions}回", "×${c.mentions}"), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.outline) },
                colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
                modifier = Modifier.clip(groupShape(i, work.characters.size)).clickable { onOpen(c.id) },
            )
        }
    }
}

@Composable
private fun Materials(work: WorkOverview, onOpen: (String) -> Unit) {
    LazyColumn(contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = 8.dp, bottom = 120.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
        if (work.materials.isEmpty()) {
            item { EmptyState(Icons.Outlined.Inventory2, tr("資料はまだありません", "No materials yet"), tr("設定やメモを ＋ から追加できます", "Add notes on the world with +")) }
        }
        itemsIndexed(work.materials, key = { _, n -> n.id }) { i, n ->
            NoteCard(n, groupShape(i, work.materials.size), onClick = { onOpen(n.id) }, onLongClick = {}, onToggleDone = null)
        }
    }
}

@Composable
private fun WorkInfoDialog(meta: WorkMeta, onDismiss: () -> Unit, onSave: (WorkMeta) -> Unit) {
    var logline by remember { mutableStateOf(meta.logline) }
    var synopsis by remember { mutableStateOf(meta.synopsis) }
    var genre by remember { mutableStateOf(meta.genre) }
    var target by remember { mutableStateOf(meta.targetChars?.toString() ?: "") }
    var deadline by remember { mutableStateOf(meta.deadline ?: "") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("作品の情報", "About the work")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                OutlinedTextField(logline, { logline = it }, label = { Text(tr("ひとことで言うと", "Logline")) }, modifier = Modifier.fillMaxWidth())
                OutlinedTextField(genre, { genre = it }, label = { Text(tr("ジャンル", "Genre")) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                OutlinedTextField(synopsis, { synopsis = it }, label = { Text(tr("あらすじ", "Synopsis")) }, minLines = 3, modifier = Modifier.fillMaxWidth())
                OutlinedTextField(target, { target = it.filter(Char::isDigit) }, label = { Text(tr("目標の文字数", "Target length (chars)")) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                OutlinedTextField(deadline, { deadline = it }, label = { Text(tr("締切（YYYY-MM-DD）", "Deadline (YYYY-MM-DD)")) }, singleLine = true, modifier = Modifier.fillMaxWidth())
            }
        },
        confirmButton = {
            Button(onClick = {
                onSave(WorkMeta(logline.trim(), synopsis.trim(), genre.trim(), target.toLongOrNull()?.takeIf { it > 0 }, deadline.trim().takeIf { Regex("""\d{4}-\d{2}-\d{2}""").matches(it) }))
            }) { Text(tr("保存", "Save")) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("キャンセル", "Cancel")) } },
    )
}

// ---------------------------------------------------------------------------
// The plot board

private val TEMPLATES = listOf("kishotenketsu", "threeAct", "heroJourney", "free")

@Composable
private fun templateName(t: String) = when (t) {
    "kishotenketsu" -> tr("起承転結", "Kishōtenketsu")
    "threeAct" -> tr("三幕構成", "Three acts")
    "heroJourney" -> tr("ヒーローズ・ジャーニー", "Hero's journey")
    else -> tr("自由", "Free")
}

/** Columns of cards (scenes, events); each card can be ticked off and tied to a chapter. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PlotBoard(workId: String, work: WorkOverview) {
    var noteId by remember { mutableStateOf<String?>(null) }
    var plot by remember { mutableStateOf<Plot?>(null) }
    var editing by remember { mutableStateOf<PlotCard?>(null) }
    // A work without a board yet starts one from a template.
    var template by remember { mutableStateOf(if (work.plotId != null) "" else null) }
    LaunchedEffect(workId, template) {
        val t = template ?: return@LaunchedEffect
        val doc = runCatching { Api.openPlot(workId, t.ifEmpty { null }) }.getOrNull()
        noteId = doc?.noteId
        plot = doc?.plot
    }
    if (template == null) {
        Column(Modifier.fillMaxSize().padding(24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(tr("プロットの型を選んでください", "Pick a structure for the plot"), style = MaterialTheme.typography.titleMedium)
            TEMPLATES.forEach { t ->
                ListItem(
                    headlineContent = { Text(templateName(t)) },
                    leadingContent = { Icon(Icons.Outlined.Timeline, null) },
                    colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
                    modifier = Modifier.clip(RoundedCornerShape(20.dp)).clickable { template = t },
                )
            }
        }
        return
    }
    // Saved a moment after each change.
    LaunchedEffect(noteId) {
        val id = noteId ?: return@LaunchedEffect
        snapshotFlow { plot }.drop(1).collectLatest { p ->
            delay(500)
            if (p != null) runCatching { Api.setPlot(id, p) }
        }
    }
    val p = plot
    if (p == null) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicator() }
        return
    }
    val chapters = work.sections.flatMap { it.chapters }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp).weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            p.columns.forEach { col ->
                Surface(shape = RoundedCornerShape(24.dp), color = MaterialTheme.colorScheme.surfaceContainerLow, modifier = Modifier.width(260.dp).fillMaxHeightCompat()) {
                    Column(Modifier.padding(10.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(col.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(6.dp))
                        p.cards.filter { it.column == col.id }.forEach { card ->
                            Surface(
                                shape = RoundedCornerShape(16.dp),
                                color = MaterialTheme.colorScheme.surfaceContainerHighest,
                                modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).clickable { editing = card },
                            ) {
                                Row(Modifier.padding(start = 4.dp, end = 12.dp, top = 4.dp, bottom = 8.dp)) {
                                    IconButton(onClick = { plot = p.copy(cards = p.cards.map { if (it.id == card.id) it.copy(done = !it.done) else it }) }) {
                                        Icon(if (card.done) Icons.Outlined.CheckCircle else Icons.Outlined.RadioButtonUnchecked, null, tint = if (card.done) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline)
                                    }
                                    Column(Modifier.padding(top = 10.dp)) {
                                        Text(card.title.ifBlank { tr("無題", "Untitled") }, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium, color = if (card.done) MaterialTheme.colorScheme.outline else MaterialTheme.colorScheme.onSurface)
                                        if (card.text.isNotBlank()) Text(card.text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 4, overflow = TextOverflow.Ellipsis)
                                        card.chapter?.let { ch -> chapters.find { it.id == ch }?.let { Text("→ " + it.title, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary) } }
                                    }
                                }
                            }
                        }
                        TextButton(onClick = {
                            val card = PlotCard(newId(), col.id, "", "", null, false)
                            plot = p.copy(cards = p.cards + card)
                            editing = card
                        }) {
                            Icon(Icons.Outlined.Add, null)
                            Text(tr("カードを追加", "Add a card"))
                        }
                    }
                }
            }
        }
    }
    editing?.let { card ->
        CardDialog(card, p.columns.map { it.id to it.title }, chapters.map { it.id to it.title }, onDismiss = {
            // A new card left empty goes.
            if (card.title.isBlank() && card.text.isBlank()) plot = p.copy(cards = p.cards.filter { it.id != card.id })
            editing = null
        }, onDelete = {
            plot = p.copy(cards = p.cards.filter { it.id != card.id })
            editing = null
        }) { changed ->
            plot = p.copy(cards = p.cards.map { if (it.id == card.id) changed else it })
            editing = null
        }
    }
}

private fun Modifier.fillMaxHeightCompat(): Modifier = this.then(Modifier.padding(bottom = 16.dp))

private fun newId(): String = java.util.UUID.randomUUID().toString().replace("-", "")

@Composable
private fun CardDialog(card: PlotCard, columns: List<Pair<String, String>>, chapters: List<Pair<String, String>>, onDismiss: () -> Unit, onDelete: () -> Unit, onSave: (PlotCard) -> Unit) {
    var title by remember { mutableStateOf(card.title) }
    var text by remember { mutableStateOf(card.text) }
    var column by remember { mutableStateOf(card.column) }
    var chapter by remember { mutableStateOf(card.chapter) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("カード", "Card")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()).imePadding(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                OutlinedTextField(title, { title = it }, label = { Text(tr("見出し", "Title")) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                OutlinedTextField(text, { text = it }, label = { Text(tr("内容", "Text")) }, minLines = 3, modifier = Modifier.fillMaxWidth())
                Text(tr("列", "Column"), style = MaterialTheme.typography.labelLarge)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    columns.forEach { (id, t) -> FilterChip(selected = column == id, onClick = { column = id }, label = { Text(t) }) }
                }
                if (chapters.isNotEmpty()) {
                    Text(tr("章", "Chapter"), style = MaterialTheme.typography.labelLarge)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        FilterChip(selected = chapter == null, onClick = { chapter = null }, label = { Text(tr("なし", "None")) })
                        chapters.forEach { (id, t) -> FilterChip(selected = chapter == id, onClick = { chapter = id }, label = { Text(t.ifBlank { tr("無題", "Untitled") }) }) }
                    }
                }
            }
        },
        confirmButton = { Button(onClick = { onSave(card.copy(title = title.trim(), text = text.trim(), column = column, chapter = chapter)) }) { Text(tr("保存", "Save")) } },
        dismissButton = {
            Row {
                IconButton(onClick = onDelete) { Icon(Icons.Outlined.Delete, tr("削除", "Delete"), tint = MaterialTheme.colorScheme.error) }
                TextButton(onClick = onDismiss) { Text(tr("キャンセル", "Cancel")) }
            }
        },
    )
}

// ---------------------------------------------------------------------------
// Chapters and characters in the editor

/** A chapter's status, synopsis and point of view, above its text. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ChapterBar(noteId: String, workId: String) {
    var info by remember { mutableStateOf<ChapterInfo?>(null) }
    var characters by remember { mutableStateOf<List<app.nisshoku.android.engine.CharacterInfo>>(emptyList()) }
    var synopsis by remember { mutableStateOf("") }
    var status by remember { mutableStateOf("") }
    var pov by remember { mutableStateOf<String?>(null) }
    var open by remember { mutableStateOf(false) }
    var loaded by remember { mutableStateOf(false) }
    LaunchedEffect(noteId) {
        val w = runCatching { Api.workOverview(workId) }.getOrNull()
        info = w?.sections?.flatMap { it.chapters }?.find { it.id == noteId }
        characters = w?.characters.orEmpty()
        synopsis = info?.synopsis ?: ""
        status = info?.status ?: ""
        pov = info?.pov
        loaded = true
    }
    LaunchedEffect(loaded) {
        if (!loaded) return@LaunchedEffect
        snapshotFlow { Triple(status, synopsis, pov) }.drop(1).collectLatest { (s, syn, p) ->
            delay(500)
            runCatching { Api.setChapterMeta(noteId, s, syn, p) }
        }
    }
    if (info == null) return
    Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceContainerLow, modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp).animateContentSize()) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                STATUSES.drop(1).forEach { s ->
                    val (l, c) = statusLabel(s)
                    FilterChip(selected = status == s, onClick = { status = if (status == s) "" else s }, label = { Text(l) }, leadingIcon = { Box(Modifier.size(8.dp).clip(CircleShape).background(c)) })
                }
                TextButton(onClick = { open = !open }) { Text(if (open) tr("閉じる", "Less") else tr("あらすじ・視点", "Synopsis, POV")) }
            }
            if (open) {
                OutlinedTextField(synopsis, { synopsis = it }, label = { Text(tr("この章のあらすじ", "This chapter's synopsis")) }, minLines = 2, modifier = Modifier.fillMaxWidth())
                if (characters.isNotEmpty()) {
                    Text(tr("視点人物", "Point of view"), style = MaterialTheme.typography.labelLarge)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        characters.forEach { c -> FilterChip(selected = pov == c.id, onClick = { pov = if (pov == c.id) null else c.id }, label = { Text(c.name) }) }
                    }
                }
            }
        }
    }
}

/** A character's sheet, above the free notes about them. */
@Composable
fun CharacterFields(noteId: String) {
    var c by remember { mutableStateOf<Character?>(null) }
    LaunchedEffect(noteId) { c = runCatching { Api.character(noteId) }.getOrNull() ?: Character() }
    LaunchedEffect(noteId, c != null) {
        if (c == null) return@LaunchedEffect
        snapshotFlow { c }.drop(1).collectLatest { v ->
            delay(500)
            if (v != null) runCatching { Api.setCharacter(noteId, v) }
        }
    }
    androidx.compose.runtime.DisposableEffect(noteId) {
        onDispose { c?.let { v -> Nisshoku.scope.launch { runCatching { Api.setCharacter(noteId, v) } } } }
    }
    val ch = c ?: return
    Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceContainerLow, modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp)) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                ROLES.dropLast(1).forEach { r -> FilterChip(selected = ch.role == r, onClick = { c = ch.copy(role = if (ch.role == r) "" else r) }, label = { Text(roleLabel(r)) }) }
            }
            @Composable
            fun field(label: String, value: String, lines: Int = 1, set: (String) -> Character) {
                OutlinedTextField(value, { c = set(it) }, label = { Text(label) }, singleLine = lines == 1, minLines = lines, modifier = Modifier.fillMaxWidth())
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.weight(1f)) { field(tr("よみがな", "Reading"), ch.ruby) { ch.copy(ruby = it) } }
                Box(Modifier.weight(1f)) { field(tr("年齢", "Age"), ch.age) { ch.copy(age = it) } }
                Box(Modifier.weight(1f)) { field(tr("性別", "Gender"), ch.gender) { ch.copy(gender = it) } }
            }
            field(tr("別名（、区切り）", "Other names (comma-separated)"), ch.aliases.joinToString("、")) { v -> ch.copy(aliases = v.split('、', ',', '，').map { it.trim() }.filter { it.isNotEmpty() }) }
            field(tr("外見", "Appearance"), ch.appearance, 2) { ch.copy(appearance = it) }
            field(tr("性格", "Personality"), ch.personality, 2) { ch.copy(personality = it) }
            field(tr("経歴", "Background"), ch.background, 2) { ch.copy(background = it) }
            field(tr("動機・目的", "Motivation"), ch.motivation, 2) { ch.copy(motivation = it) }
            Text(tr("色", "Colour"), style = MaterialTheme.typography.labelLarge)
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                listOf("", "#e0773b", "#c2412d", "#c9a227", "#5e8c4a", "#3f7cac", "#3f51a6", "#8a5cb8").forEach { col ->
                    Box(
                        Modifier.size(32.dp).clip(CircleShape)
                            .background(parseColor(col) ?: MaterialTheme.colorScheme.surfaceVariant)
                            .clickable { c = ch.copy(color = col) },
                        contentAlignment = Alignment.Center,
                    ) { if (ch.color == col) Icon(Icons.Outlined.CheckCircle, null, tint = Color.White, modifier = Modifier.size(18.dp)) }
                }
            }
            Spacer(Modifier.height(4.dp))
            Text(tr("メモ", "Notes"), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.outline)
        }
    }
}

/** Marks a notebook as a work (or no longer). */
suspend fun makeWork(folderId: String, on: Boolean) {
    runCatching { Api.setWork(folderId, if (on) WorkMeta() else null) }
        .onFailure { throw IllegalStateException(trNow("作品にできませんでした", "Could not make it a work") + ": " + it.message) }
}

/** A work's plot note, on its board. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PlotScreen(noteId: String, model: AppModel, onClose: () -> Unit) {
    var work by remember { mutableStateOf<WorkOverview?>(null) }
    LaunchedEffect(noteId) {
        val note = runCatching { Api.note(noteId) }.getOrNull()
        val w = note?.let { workOf(model.home.value.folders, it.parentId) }
        work = w?.let { runCatching { Api.workOverview(it.id) }.getOrNull() }
    }
    androidx.activity.compose.BackHandler(onBack = onClose)
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(tr("プロット", "Plot") + (work?.title?.let { " · $it" } ?: "")) },
                navigationIcon = { IconButton(onClick = onClose) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, tr("戻る", "Back")) } },
            )
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding)) {
            work?.let { PlotBoard(it.id, it) } ?: Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicator() }
        }
    }
}
