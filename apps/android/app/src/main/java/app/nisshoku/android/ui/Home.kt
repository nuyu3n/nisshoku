package app.nisshoku.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.Notes
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.AutoStories
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.CloudDone
import androidx.compose.material.icons.outlined.CloudOff
import androidx.compose.material.icons.outlined.CloudSync
import androidx.compose.material.icons.outlined.CreateNewFolder
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.DeleteForever
import androidx.compose.material.icons.outlined.DriveFileMove
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.ErrorOutline
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.Inbox
import androidx.compose.material.icons.outlined.Menu
import androidx.compose.material.icons.outlined.PushPin
import androidx.compose.material.icons.outlined.Restore
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Tag
import androidx.compose.material.icons.outlined.Event
import androidx.compose.material.icons.outlined.EventBusy
import androidx.compose.material.icons.outlined.RadioButtonUnchecked
import androidx.compose.material.icons.outlined.TaskAlt
import androidx.compose.material.icons.outlined.Today
import androidx.compose.material.icons.outlined.WarningAmber
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.automirrored.outlined.KeyboardArrowRight
import androidx.compose.material3.Surface
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.DriveFileRenameOutline
import androidx.compose.material.icons.outlined.Language
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material.icons.outlined.VerticalAlignTop
import androidx.compose.material3.AssistChip
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ContainedLoadingIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FloatingActionButtonMenu
import androidx.compose.material3.FloatingActionButtonMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeFlexibleTopAppBar
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.NavigationDrawerItemDefaults
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ToggleFloatingActionButton
import androidx.compose.material3.ToggleFloatingActionButtonDefaults.animateIcon
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.PullToRefreshDefaults
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.nisshoku.android.engine.Folder
import app.nisshoku.android.engine.NoteSummary
import app.nisshoku.android.engine.Scope
import app.nisshoku.android.tr
import app.nisshoku.android.trNow
import kotlinx.coroutines.launch

@Composable
fun scopeTitle(scope: Scope, folders: List<Folder>): String = when (scope) {
    Scope.All -> tr("すべてのノート", "All notes")
    Scope.Diary -> tr("日記", "Diary")
    Scope.Todo -> tr("タスク", "Tasks")
    Scope.Pinned -> tr("ピン留め", "Pinned")
    Scope.Trash -> tr("ゴミ箱", "Trash")
    Scope.Conflicts -> tr("競合", "Conflicts")
    is Scope.InFolder -> folders.find { it.id == scope.id }?.title ?: tr("ノートブック", "Notebook")
    is Scope.WithTag -> "#" + (LocalTags.current.find { it.id == scope.id }?.title ?: "")
    is Scope.Search -> scope.query
}

/** The tags, for the title of a tag's list. */
private val LocalTags = androidx.compose.runtime.compositionLocalOf<List<app.nisshoku.android.engine.Tag>> { emptyList() }

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun HomeScreen(
    model: AppModel,
    snackbar: SnackbarHostState,
    onOpen: (String) -> Unit,
    onSearch: () -> Unit,
    onSettings: () -> Unit,
    onWork: (String) -> Unit = {},
) {
    val state by model.home.collectAsState()
    val sync by model.sync.collectAsState()
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val haptics = LocalHapticFeedback.current
    var fabOpen by rememberSaveable { mutableStateOf(false) }
    var actionsFor by remember { mutableStateOf<NoteSummary?>(null) }
    var movingNote by remember { mutableStateOf<NoteSummary?>(null) }
    var newTask by remember { mutableStateOf(false) }
    var dueFor by remember { mutableStateOf<NoteSummary?>(null) }
    var folderEdit by remember { mutableStateOf<FolderEdit?>(null) }
    var tagsFor by remember { mutableStateOf<NoteSummary?>(null) }
    var renameFor by remember { mutableStateOf<NoteSummary?>(null) }
    var deleteFor by remember { mutableStateOf<NoteSummary?>(null) }
    var tagEdit by remember { mutableStateOf<app.nisshoku.android.engine.Tag?>(null) }
    var folderMove by remember { mutableStateOf<Folder?>(null) }
    var subOf by remember { mutableStateOf<String?>(null) }
    var templates by remember { mutableStateOf(false) }
    var clipping by remember { mutableStateOf(false) }
    var confirmEmpty by remember { mutableStateOf(false) }
    var overflow by remember { mutableStateOf(false) }
    val stats by model.stats.collectAsState()
    val prefs by model.prefs.collectAsState()
    val context = androidx.compose.ui.platform.LocalContext.current
    val look = ListLook(
        excerpt = (prefs["showExcerpt"] as? kotlinx.serialization.json.JsonPrimitive)?.content != "false",
        compact = (prefs["compactList"] as? kotlinx.serialization.json.JsonPrimitive)?.content == "true",
    )

    // "New task" asked for from outside (a widget, a launcher shortcut).
    val newTaskRequests by model.newTaskRequests.collectAsState()
    var seenRequests by rememberSaveable { mutableStateOf(0) }
    LaunchedEffect(newTaskRequests) {
        if (newTaskRequests > seenRequests) {
            seenRequests = newTaskRequests
            newTask = true
        }
    }

    BackHandler(drawer.isOpen) { scope.launch { drawer.close() } }
    BackHandler(fabOpen) { fabOpen = false }
    // Back from a notebook, the tasks, the diary…: to all notes first, as
    // other apps' lists do, rather than out of the app.
    BackHandler(!drawer.isOpen && !fabOpen && state.scope != Scope.All) { model.setScope(Scope.All) }

    androidx.compose.runtime.CompositionLocalProvider(LocalTags provides state.tags, LocalListLook provides look) {
    ModalNavigationDrawer(
        drawerState = drawer,
        drawerContent = {
            Drawer(
                state = state,
                today = {
                    TodayCard(stats, model.prefInt("dailyGoal", 0)) {
                        scope.launch { drawer.close() }
                        scope.launch { model.todaysDiary()?.let(onOpen) }
                    }
                },
                onEditTag = { tagEdit = it },
                onScope = {
                    model.setScope(it)
                    scope.launch { drawer.close() }
                },
                onSettings = {
                    scope.launch { drawer.close() }
                    onSettings()
                },
                onNewFolder = { folderEdit = FolderEdit(null, "") },
                onEditFolder = { folderEdit = FolderEdit(it.id, it.title, it.icon, it.color) },
            )
        },
    ) {
        val bar = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
        Scaffold(
            modifier = Modifier.nestedScroll(bar.nestedScrollConnection),
            snackbarHost = { SnackbarHost(snackbar) },
            topBar = {
                LargeFlexibleTopAppBar(
                    title = {
                        AnimatedContent(scopeTitle(state.scope, state.folders), label = "title") { Text(it, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                    },
                    subtitle = {
                        val count = state.notes.size
                        val unit = if (state.scope == Scope.Todo) (if (count == 1) "task" else "tasks") else (if (count == 1) "note" else "notes")
                        Text(if (state.loading) " " else tr("$count 件", "$count $unit"))
                    },
                    navigationIcon = {
                        IconButton(onClick = { scope.launch { drawer.open() } }) {
                            Icon(Icons.Outlined.Menu, tr("メニュー", "Menu"))
                        }
                    },
                    actions = {
                        IconButton(onClick = onSearch) { Icon(Icons.Outlined.Search, tr("検索", "Search")) }
                        SyncButton(sync.status.state, sync.target.kind != "none", onSync = model::syncNow, onSetUp = onSettings)
                        Box {
                            IconButton(onClick = { overflow = true }) { Icon(Icons.Outlined.MoreVert, tr("その他", "More")) }
                            DropdownMenu(expanded = overflow, onDismissRequest = { overflow = false }, shape = RoundedCornerShape(16.dp)) {
                                DropdownMenuItem(
                                    text = { Text(tr("テンプレートから作成", "New from a template")) },
                                    leadingIcon = { Icon(Icons.Outlined.Description, null) },
                                    onClick = { overflow = false; templates = true },
                                )
                                DropdownMenuItem(
                                    text = { Text(tr("Web ページを取り込む", "Clip a web page")) },
                                    leadingIcon = { Icon(Icons.Outlined.Language, null) },
                                    onClick = { overflow = false; clipping = true },
                                )
                                if (state.scope is Scope.InFolder) {
                                    DropdownMenuItem(
                                        text = { Text(tr("ノートブックを編集", "Edit the notebook")) },
                                        leadingIcon = { Icon(Icons.Outlined.Edit, null) },
                                        onClick = {
                                            overflow = false
                                            state.folders.find { it.id == (state.scope as Scope.InFolder).id }?.let { folderEdit = FolderEdit(it.id, it.title, it.icon, it.color) }
                                        },
                                    )
                                }
                                DropdownMenuItem(
                                    text = { Text(tr("設定", "Settings")) },
                                    leadingIcon = { Icon(Icons.Outlined.Settings, null) },
                                    onClick = { overflow = false; onSettings() },
                                )
                            }
                        }
                    },
                    scrollBehavior = bar,
                )
            },
            floatingActionButton = {
                if (state.scope != Scope.Trash) {
                    // In the tasks and the diary, + makes what is shown there at once.
                    val direct = when (state.scope) {
                        Scope.Todo -> ({ newTask = true })
                        Scope.Diary -> ({ scope.launch { model.todaysDiary()?.let(onOpen) }; Unit })
                        else -> null
                    }
                    FabMenu(
                        open = fabOpen,
                        icon = if (direct != null) Icons.Outlined.Add else Icons.Outlined.Edit,
                        onOpenChange = {
                            haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                            if (it && direct != null) direct() else fabOpen = it
                        },
                        onNote = {
                            fabOpen = false
                            scope.launch { model.newNote()?.let(onOpen) }
                        },
                        onTask = {
                            fabOpen = false
                            newTask = true
                        },
                        onDiary = {
                            fabOpen = false
                            scope.launch { model.todaysDiary()?.let(onOpen) }
                        },
                    )
                }
            },
        ) { padding ->
            val pull = rememberPullToRefreshState()
            val syncing = sync.status.state == "syncing"
            PullToRefreshBox(
                isRefreshing = syncing,
                onRefresh = { if (sync.target.kind != "none") model.syncNow() else onSettings() },
                state = pull,
                modifier = Modifier.fillMaxSize().padding(padding),
                indicator = {
                    PullToRefreshDefaults.LoadingIndicator(state = pull, isRefreshing = syncing, modifier = Modifier.align(Alignment.TopCenter))
                },
            ) {
                NoteList(
                    state = state,
                    onOpen = onOpen,
                    onActions = { actionsFor = it },
                    onTrash = model::trash,
                    onPin = model::togglePinned,
                    onDone = model::toggleDone,
                    onEmptyTrash = { confirmEmpty = true },
                    header = when {
                        state.scope == Scope.Diary -> ({
                            DiaryCalendar(
                                version = state.version,
                                weekStartsMonday = model.prefString("weekStart", "sun") == "mon",
                                onOpen = { date -> scope.launch { model.diaryOf(date)?.let(onOpen) } },
                            )
                            Spacer(Modifier.height(8.dp))
                            DiaryHeader(state.version, onOpen)
                        })
                        state.scope is Scope.InFolder -> ({
                            val id = (state.scope as Scope.InFolder).id
                            workOf(state.folders, id)?.let { w -> WorkCard(w.title) { onWork(w.id) } }
                            val children = state.folders.filter { it.parentId == id }
                            if (children.isNotEmpty()) SubNotebooks(children) { model.setScope(Scope.InFolder(it.id)) }
                        })
                        else -> null
                    },
                )
            }
        }
    }
    }

    actionsFor?.let { note ->
        NoteActions(
            note = note,
            inTrash = state.scope == Scope.Trash,
            onDismiss = { actionsFor = null },
            onPin = { model.togglePinned(note) },
            onMove = { movingNote = note },
            onTrash = { model.trash(note) },
            onRestore = { model.restore(note) },
            onDelete = { deleteFor = note },
            onRename = { renameFor = note },
            onTags = { tagsFor = note },
            onDuplicate = { scope.launch { model.duplicate(note.id)?.let(onOpen) } },
            onTemplate = { model.saveAsTemplate(note.id) },
            onShare = { scope.launch { shareNote(context, note.id) } },
            onDue = { dueFor = note },
            onClearDue = { model.setDue(note, 0, false) },
            onToggleTodo = { model.toggleTodo(note) },
        )
    }
    tagsFor?.let { note ->
        TagsSheet(current = note.tags, all = state.tags, onDismiss = { tagsFor = null }, onSave = { model.setTags(note.id, it) })
    }
    renameFor?.let { note ->
        TextPrompt(
            title = tr("名前を変更", "Rename"),
            label = tr("タイトル", "Title"),
            confirm = tr("変更", "Rename"),
            initial = note.title,
            onDismiss = { renameFor = null },
            onDone = {
                model.renameNote(note.id, it)
                renameFor = null
            },
        )
    }
    deleteFor?.let { note ->
        ConfirmDialog(
            title = tr("完全に削除しますか？", "Delete forever?"),
            text = tr("「${note.title.ifBlank { "無題" }}」を完全に削除します。元に戻せません。", "\"${note.title.ifBlank { "Untitled" }}\" will be deleted for good. This cannot be undone."),
            confirm = tr("削除", "Delete"),
            onDismiss = { deleteFor = null },
            onConfirm = { model.deleteForever(note) },
        )
    }
    if (confirmEmpty) {
        ConfirmDialog(
            title = tr("ゴミ箱を空にしますか？", "Empty the trash?"),
            text = tr("ゴミ箱の ${state.notes.size} 件を完全に削除します。元に戻せません。", "The ${state.notes.size} notes in the trash will be deleted for good. This cannot be undone."),
            confirm = tr("空にする", "Empty"),
            onDismiss = { confirmEmpty = false },
            onConfirm = model::emptyTrash,
        )
    }
    if (templates) {
        TemplateSheet(onDismiss = { templates = false }) { t ->
            templates = false
            scope.launch { model.fromTemplate(t.id)?.let(onOpen) }
        }
    }
    if (clipping) {
        ClipDialog(onDismiss = { clipping = false }) { url ->
            clipping = false
            scope.launch { model.clip(url)?.let(onOpen) }
        }
    }
    tagEdit?.let { tag ->
        TagDialog(
            tag = tag,
            onDismiss = { tagEdit = null },
            onRename = { model.renameTag(tag.id, it); tagEdit = null },
            onDelete = { model.deleteTag(tag.id); tagEdit = null },
        )
    }
    folderMove?.let { folder ->
        // A notebook goes under another (not under itself or its own), or to the top.
        val inside = descendants(state.folders, folder.id) + folder.id
        MoveSheet(
            folders = state.folders.filter { !it.isDiary && it.id !in inside },
            top = true,
            onDismiss = { folderMove = null },
            onPick = {
                model.moveFolder(folder.id, it?.id ?: "")
                folderMove = null
            },
        )
    }
    subOf?.let { parent ->
        TextPrompt(
            title = tr("新しいサブノートブック", "New sub-notebook"),
            label = tr("名前", "Name"),
            confirm = tr("作成", "Create"),
            onDismiss = { subOf = null },
            onDone = {
                model.createFolder(it, parent)
                subOf = null
            },
        )
    }
    dueFor?.let { note ->
        DuePicker(
            initial = note.todoDue,
            onDismiss = { dueFor = null },
            onPick = { date ->
                model.setDue(note, allDayDue(date), true)
                dueFor = null
            },
        )
    }
    movingNote?.let { note ->
        MoveSheet(
            folders = state.folders.filter { !it.isDiary },
            onDismiss = { movingNote = null },
            onPick = { folder ->
                folder?.let { model.move(note, it.id) }
                movingNote = null
            },
        )
    }
    if (newTask) {
        NewTaskSheet(
            onDismiss = { newTask = false },
            onAdd = { title, due ->
                newTask = false
                scope.launch {
                    if (model.newTask(title, due, due != null)) model.say(trNow("タスクを追加しました", "Task added"))
                }
            },
        )
    }
    folderEdit?.let { edit ->
        FolderDialog(
            edit = edit,
            onDismiss = { folderEdit = null },
            onSave = { title, icon, color ->
                if (edit.id == null) model.createFolder(title)
                else {
                    if (title != edit.title) model.renameFolder(edit.id, title)
                    if (icon != edit.icon) model.setFolderIcon(edit.id, icon.ifEmpty { null })
                    if (color != edit.color) model.setFolderColor(edit.id, color.ifEmpty { null })
                }
                folderEdit = null
            },
            onDelete = edit.id?.let { id -> { model.trashFolder(id); folderEdit = null } },
            onMove = edit.id?.let { id -> { folderEdit = null; folderMove = state.folders.find { it.id == id } } },
            isWork = state.folders.find { it.id == edit.id }?.isWork,
            onWork = edit.id?.let { id ->
                { on: Boolean ->
                    folderEdit = null
                    scope.launch {
                        runCatching { makeWork(id, on) }.onFailure { model.say(it.message ?: "") }
                        model.reload()
                        if (on) onWork(id)
                    }
                }
            },
            onNewSub = edit.id?.let { id -> { folderEdit = null; subOf = id } },
        )
    }
}

/** Sync: its state, and a tap to sync now (or to set it up). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun SyncButton(state: String, configured: Boolean, onSync: () -> Unit, onSetUp: () -> Unit) {
    IconButton(onClick = if (configured) onSync else onSetUp, enabled = state != "syncing") {
        AnimatedContent(
            targetState = if (!configured) "off" else state,
            transitionSpec = { (fadeIn() + scaleIn(initialScale = 0.6f)) togetherWith (fadeOut() + scaleOut(targetScale = 0.6f)) },
            label = "sync",
        ) { s ->
            when (s) {
                "syncing" -> ContainedLoadingIndicator(Modifier.size(28.dp))
                "error" -> Icon(Icons.Outlined.ErrorOutline, tr("同期エラー", "Sync error"), tint = MaterialTheme.colorScheme.error)
                "off" -> Icon(Icons.Outlined.CloudOff, tr("同期を設定", "Set up sync"))
                "idle" -> Icon(Icons.Outlined.CloudDone, tr("同期する", "Sync now"))
                else -> Icon(Icons.Outlined.CloudSync, tr("同期する", "Sync now"))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun FabMenu(open: Boolean, icon: ImageVector, onOpenChange: (Boolean) -> Unit, onNote: () -> Unit, onTask: () -> Unit, onDiary: () -> Unit) {
    FloatingActionButtonMenu(
        expanded = open,
        button = {
            ToggleFloatingActionButton(
                checked = open,
                onCheckedChange = onOpenChange,
                modifier = Modifier.semantics { contentDescription = trNow("新規作成", "Create") },
            ) {
                val shown by remember(icon) { derivedStateOf { if (checkedProgress > 0.5f) Icons.Outlined.Close else icon } }
                Icon(
                    painter = rememberVectorPainter(shown),
                    contentDescription = null,
                    modifier = Modifier.animateIcon({ checkedProgress }).scale(1f),
                )
            }
        },
    ) {
        FloatingActionButtonMenuItem(onClick = onDiary, icon = { Icon(Icons.Outlined.Today, null) }, text = { Text(tr("今日の日記", "Today's diary")) })
        FloatingActionButtonMenuItem(onClick = onTask, icon = { Icon(Icons.Outlined.TaskAlt, null) }, text = { Text(tr("タスク", "Task")) })
        FloatingActionButtonMenuItem(onClick = onNote, icon = { Icon(Icons.AutoMirrored.Outlined.Notes, null) }, text = { Text(tr("ノート", "Note")) })
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun NoteList(
    state: HomeState,
    onOpen: (String) -> Unit,
    onActions: (NoteSummary) -> Unit,
    onTrash: (NoteSummary) -> Unit,
    onPin: (NoteSummary) -> Unit,
    onDone: (NoteSummary) -> Unit,
    onEmptyTrash: () -> Unit,
    header: (@Composable () -> Unit)?,
) {
    val list = rememberLazyListState()
    if (state.notes.isEmpty() && state.scope != Scope.Diary) {
        AnimatedVisibility(!state.loading, enter = fadeIn(), exit = fadeOut()) {
            // Scrollable, so that pulling still syncs.
            Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 12.dp)) {
                header?.let { Column(Modifier.padding(top = 4.dp)) { it() } }
                when (state.scope) {
                    Scope.Trash -> EmptyState(Icons.Outlined.Delete, tr("ゴミ箱は空です", "The trash is empty"), tr("削除したノートはここに入ります", "Deleted notes come here"))
                    Scope.Todo -> EmptyState(Icons.Outlined.TaskAlt, tr("タスクはありません", "No tasks"), tr("＋ からタスクを追加できます", "Add one with +"))
                    Scope.Diary -> EmptyState(Icons.Outlined.AutoStories, tr("日記はまだありません", "No diary entries yet"), tr("＋ から今日の日記を書きましょう", "Write today's with +"))
                    else -> EmptyState(Icons.Outlined.Inbox, tr("ノートはまだありません", "No notes yet"), tr("＋ から書き始めましょう", "Start writing with +"))
                }
            }
        }
        return
    }
    if (state.scope == Scope.Todo) {
        val groups = groupTasks(state.notes)
        LazyColumn(
            state = list,
            contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = 4.dp, bottom = 120.dp),
            verticalArrangement = Arrangement.spacedBy(3.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            for ((group, tasks) in groups) {
                item(key = "g-$group") {
                    SectionLabel("${group.label()}  ${tasks.size}", Modifier.animateItem())
                }
                itemsIndexed(tasks, key = { _, n -> n.id }) { i, note ->
                    SwipeNote(note, groupShape(i, tasks.size), state.scope, onOpen, onActions, onTrash, onPin, onDone, Modifier.animateItem())
                }
            }
        }
        return
    }
    val pinned = state.notes.filter { it.pinned && state.scope != Scope.Trash }
    val rest = state.notes.filter { !(it.pinned && state.scope != Scope.Trash) }
    LazyColumn(
        state = list,
        contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = 4.dp, bottom = 120.dp),
        verticalArrangement = Arrangement.spacedBy(3.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        if (header != null) {
            item(key = "header") {
                Column(Modifier.padding(bottom = 12.dp)) { header() }
            }
        }
        if (state.scope == Scope.Trash) {
            item(key = "empty-trash") {
                Row(Modifier.fillMaxWidth().padding(bottom = 8.dp), horizontalArrangement = Arrangement.End) {
                    TextButton(onClick = onEmptyTrash) {
                        Icon(Icons.Outlined.DeleteForever, null)
                        Spacer(Modifier.width(6.dp))
                        Text(tr("ゴミ箱を空にする", "Empty the trash"))
                    }
                }
            }
        }
        if (pinned.isNotEmpty()) {
            item(key = "pinned-label") { SectionLabel(tr("ピン留め", "Pinned"), Modifier.animateItem()) }
            itemsIndexed(pinned, key = { _, n -> "p-" + n.id }) { i, note ->
                SwipeNote(note, groupShape(i, pinned.size), state.scope, onOpen, onActions, onTrash, onPin, onDone, Modifier.animateItem())
            }
            item(key = "rest-label") { SectionLabel(tr("ノート", "Notes"), Modifier.animateItem()) }
        }
        itemsIndexed(rest, key = { _, n -> n.id }) { i, note ->
            SwipeNote(note, groupShape(i, rest.size), state.scope, onOpen, onActions, onTrash, onPin, onDone, Modifier.animateItem())
        }
    }
}

/** A note that swipes: to the left into the trash, to the right pinned (or done, for a task). */
@Composable
private fun SwipeNote(
    note: NoteSummary,
    shape: androidx.compose.ui.graphics.Shape,
    scope: Scope,
    onOpen: (String) -> Unit,
    onActions: (NoteSummary) -> Unit,
    onTrash: (NoteSummary) -> Unit,
    onPin: (NoteSummary) -> Unit,
    onDone: (NoteSummary) -> Unit,
    modifier: Modifier,
) {
    val haptics = LocalHapticFeedback.current
    val inTrash = scope == Scope.Trash
    val swipe = rememberSwipeToDismissBoxState()
    val coroutines = rememberCoroutineScope()
    SwipeToDismissBox(
        state = swipe,
        modifier = modifier,
        enableDismissFromStartToEnd = !inTrash,
        enableDismissFromEndToStart = !inTrash,
        onDismiss = { value ->
            haptics.performHapticFeedback(HapticFeedbackType.Confirm)
            when (value) {
                SwipeToDismissBoxValue.EndToStart -> onTrash(note)
                SwipeToDismissBoxValue.StartToEnd -> if (note.isTodo) onDone(note) else onPin(note)
                else -> {}
            }
            coroutines.launch { swipe.reset() }
        },
        backgroundContent = {
            val direction = swipe.dismissDirection
            val progress by animateFloatAsState(if (swipe.progress > 0.35f) 1.15f else 1f, label = "swipe")
            val (color, icon, align) = when (direction) {
                SwipeToDismissBoxValue.EndToStart -> Triple(MaterialTheme.colorScheme.errorContainer, Icons.Outlined.Delete, Alignment.CenterEnd)
                SwipeToDismissBoxValue.StartToEnd -> Triple(
                    MaterialTheme.colorScheme.primaryContainer,
                    if (note.isTodo) Icons.Outlined.CheckCircle else Icons.Outlined.PushPin,
                    Alignment.CenterStart,
                )
                else -> Triple(MaterialTheme.colorScheme.surface, null, Alignment.Center)
            }
            Box(Modifier.fillMaxSize().clip(shape).background(color).padding(horizontal = 24.dp), contentAlignment = align) {
                icon?.let { Icon(it, null, modifier = Modifier.scale(progress)) }
            }
        },
    ) {
        NoteCard(
            note = note,
            shape = shape,
            onClick = { onOpen(note.id) },
            onLongClick = { onActions(note) },
            onToggleDone = if (note.isTodo && !inTrash) ({ onDone(note) }) else null,
        )
    }
}

@Composable
private fun Drawer(
    state: HomeState,
    today: @Composable () -> Unit,
    onEditTag: (app.nisshoku.android.engine.Tag) -> Unit,
    onScope: (Scope) -> Unit,
    onSettings: () -> Unit,
    onNewFolder: () -> Unit,
    onEditFolder: (Folder) -> Unit,
) {
    ModalDrawerSheet(drawerShape = RoundedCornerShape(topEnd = 28.dp, bottomEnd = 28.dp)) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 12.dp)) {
            Text(
                tr("日色", "Nisshoku"),
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(start = 16.dp, top = 24.dp, bottom = 12.dp),
            )
            today()
            Spacer(Modifier.height(12.dp))
            fun item(scope: Scope): Boolean = state.scope == scope
            DrawerItem(tr("すべてのノート", "All notes"), Icons.AutoMirrored.Outlined.Notes, item(Scope.All)) { onScope(Scope.All) }
            DrawerItem(tr("ピン留め", "Pinned"), Icons.Outlined.PushPin, item(Scope.Pinned)) { onScope(Scope.Pinned) }
            DrawerItem(tr("日記", "Diary"), Icons.Outlined.AutoStories, item(Scope.Diary)) { onScope(Scope.Diary) }
            DrawerItem(tr("タスク", "Tasks"), Icons.Outlined.TaskAlt, item(Scope.Todo)) { onScope(Scope.Todo) }
            HorizontalDivider(Modifier.padding(horizontal = 16.dp, vertical = 12.dp))
            Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(tr("ノートブック", "Notebooks"), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
                IconButton(onClick = onNewFolder) { Icon(Icons.Outlined.CreateNewFolder, tr("新しいノートブック", "New notebook")) }
            }
            for ((folder, depth) in tree(state.folders.filter { !it.isDiary })) {
                val selected = (state.scope as? Scope.InFolder)?.id == folder.id
                NavigationDrawerItem(
                    label = { Text(folder.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    icon = { FolderMark(folder.icon, folder.color, Icons.Outlined.Folder) },
                    badge = {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text("${folder.noteCount}", style = MaterialTheme.typography.labelMedium)
                            if (selected) {
                                IconButton(onClick = { onEditFolder(folder) }, modifier = Modifier.size(36.dp)) {
                                    Icon(Icons.Outlined.Edit, tr("編集", "Edit"), modifier = Modifier.size(18.dp))
                                }
                            }
                        }
                    },
                    selected = selected,
                    onClick = { onScope(Scope.InFolder(folder.id)) },
                    modifier = Modifier.padding(start = (depth * 16).dp).height(52.dp),
                    colors = NavigationDrawerItemDefaults.colors(),
                )
            }
            if (state.tags.isNotEmpty()) {
                HorizontalDivider(Modifier.padding(horizontal = 16.dp, vertical = 12.dp))
                Text(tr("タグ", "Tags"), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 16.dp, bottom = 8.dp))
                for (tag in state.tags) {
                    NavigationDrawerItem(
                        label = { Text(tag.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        icon = { Icon(Icons.Outlined.Tag, null) },
                        badge = {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text("${tag.noteCount}", style = MaterialTheme.typography.labelMedium)
                                if ((state.scope as? Scope.WithTag)?.id == tag.id) {
                                    IconButton(onClick = { onEditTag(tag) }, modifier = Modifier.size(36.dp)) {
                                        Icon(Icons.Outlined.Edit, tr("編集", "Edit"), modifier = Modifier.size(18.dp))
                                    }
                                }
                            }
                        },
                        selected = (state.scope as? Scope.WithTag)?.id == tag.id,
                        onClick = { onScope(Scope.WithTag(tag.id)) },
                        modifier = Modifier.height(52.dp),
                    )
                }
            }
            HorizontalDivider(Modifier.padding(horizontal = 16.dp, vertical = 12.dp))
            if (state.conflicts > 0 || item(Scope.Conflicts)) {
                DrawerItem(tr("競合", "Conflicts"), Icons.Outlined.WarningAmber, item(Scope.Conflicts), badge = state.conflicts.takeIf { it > 0 }) { onScope(Scope.Conflicts) }
            }
            DrawerItem(tr("ゴミ箱", "Trash"), Icons.Outlined.Delete, item(Scope.Trash)) { onScope(Scope.Trash) }
            DrawerItem(tr("設定", "Settings"), Icons.Outlined.Settings, false, onClick = onSettings)
            Spacer(Modifier.height(24.dp).navigationBarsPadding())
        }
    }
}

@Composable
private fun DrawerItem(label: String, icon: ImageVector, selected: Boolean, badge: Int? = null, onClick: () -> Unit) {
    NavigationDrawerItem(
        label = { Text(label) },
        icon = { Icon(icon, null) },
        badge = badge?.let { { Text("$it", style = MaterialTheme.typography.labelMedium) } },
        selected = selected,
        onClick = onClick,
        modifier = Modifier.height(52.dp),
    )
}

/** Notebooks in tree order, with their depth. */
private fun tree(folders: List<Folder>): List<Pair<Folder, Int>> {
    val byParent = folders.groupBy { it.parentId }
    val ids = folders.map { it.id }.toSet()
    val out = mutableListOf<Pair<Folder, Int>>()
    fun walk(parent: String, depth: Int) {
        for (f in byParent[parent].orEmpty().sortedBy { it.title.lowercase() }) {
            out += f to depth
            walk(f.id, depth + 1)
        }
    }
    // Roots: no parent, or a parent not shown.
    folders.filter { it.parentId.isEmpty() || it.parentId !in ids }.map { it.parentId }.distinct().forEach { walk(it, 0) }
    return out
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun NoteActions(
    note: NoteSummary,
    inTrash: Boolean,
    onDismiss: () -> Unit,
    onPin: () -> Unit,
    onMove: () -> Unit,
    onTrash: () -> Unit,
    onRestore: () -> Unit,
    onDelete: () -> Unit,
    onRename: () -> Unit,
    onTags: () -> Unit,
    onDuplicate: () -> Unit,
    onTemplate: () -> Unit,
    onShare: () -> Unit,
    onDue: () -> Unit,
    onClearDue: () -> Unit,
    onToggleTodo: () -> Unit,
) {
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = rememberCoroutineScope()
    fun act(f: () -> Unit) {
        scope.launch { sheet.hide() }.invokeOnCompletion {
            onDismiss()
            f()
        }
    }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheet) {
        Text(
            note.title.ifBlank { tr("無題", "Untitled") },
            style = MaterialTheme.typography.titleLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
        )
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 12.dp).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            val actions = if (inTrash) listOf(
                Triple(tr("元に戻す", "Restore"), Icons.Outlined.Restore, onRestore),
                Triple(tr("完全に削除", "Delete forever"), Icons.Outlined.DeleteForever, onDelete),
            ) else listOfNotNull(
                Triple(tr("名前を変更", "Rename"), Icons.Outlined.DriveFileRenameOutline, onRename),
                Triple(if (note.pinned) tr("ピン留めを外す", "Unpin") else tr("ピン留め", "Pin"), Icons.Outlined.PushPin, onPin),
                Triple(tr("タグ", "Tags") + if (note.tags.isNotEmpty()) "（${note.tags.joinToString("、")}）" else "", Icons.Outlined.Tag, onTags),
                Triple(if (note.todoDue > 0) tr("期限を変更", "Change the due date") else tr("期限を設定", "Set a due date"), Icons.Outlined.Event, onDue),
                if (note.todoDue > 0) Triple(tr("期限を消す", "Remove the due date"), Icons.Outlined.EventBusy, onClearDue) else null,
                Triple(
                    if (note.isTodo) tr("タスクをやめてノートにする", "Make it a plain note") else tr("タスクにする", "Make it a task"),
                    if (note.isTodo) Icons.AutoMirrored.Outlined.Notes else Icons.Outlined.RadioButtonUnchecked,
                    onToggleTodo,
                ),
                Triple(tr("ノートブックへ移動", "Move to notebook"), Icons.Outlined.DriveFileMove, onMove),
                Triple(tr("複製", "Duplicate"), Icons.Outlined.ContentCopy, onDuplicate),
                Triple(tr("テンプレートとして保存", "Save as template"), Icons.Outlined.Description, onTemplate),
                Triple(tr("共有", "Share"), Icons.Outlined.Share, onShare),
                Triple(tr("ゴミ箱へ移動", "Move to trash"), Icons.Outlined.Delete, onTrash),
            )
            actions.forEachIndexed { i, (label, icon, run) ->
                ListItem(
                    headlineContent = { Text(label) },
                    leadingContent = { Icon(icon, null) },
                    colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
                    modifier = Modifier
                        .clip(groupShape(i, actions.size, outer = 20, inner = 4))
                        .clickable { act(run) },
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MoveSheet(folders: List<Folder>, onDismiss: () -> Unit, top: Boolean = false, onPick: (Folder?) -> Unit) {
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Text(tr("移動先", "Move to"), style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
        LazyColumn(contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            if (top) {
                item(key = "top") {
                    ListItem(
                        headlineContent = { Text(tr("いちばん上", "The top level")) },
                        leadingContent = { Icon(Icons.Outlined.VerticalAlignTop, null) },
                        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.secondaryContainer),
                        modifier = Modifier.padding(bottom = 6.dp).clip(RoundedCornerShape(20.dp)).clickable { onPick(null) },
                    )
                }
            }
            val items = tree(folders)
            itemsIndexed(items, key = { _, it -> it.first.id }) { i, (folder, depth) ->
                ListItem(
                    headlineContent = { Text(folder.title) },
                    leadingContent = { FolderMark(folder.icon, folder.color, Icons.Outlined.Folder) },
                    colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
                    modifier = Modifier
                        .padding(start = (depth * 16).dp)
                        .clip(groupShape(i, items.size, outer = 20, inner = 4))
                        .clickable { onPick(folder) },
                )
            }
        }
    }
}

/** A dialog asking for one line of text, with the keyboard up at once. */
@Composable
fun TextPrompt(
    title: String,
    label: String,
    confirm: String,
    initial: String = "",
    onDismiss: () -> Unit,
    onDone: (String) -> Unit,
) {
    var text by remember { mutableStateOf(initial) }
    val focus = remember { FocusRequester() }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                label = { Text(label) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (text.isNotBlank()) onDone(text.trim()) }),
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
            )
        },
        confirmButton = { Button(onClick = { onDone(text.trim()) }, enabled = text.isNotBlank()) { Text(confirm) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("キャンセル", "Cancel")) } },
        modifier = Modifier.imePadding(),
    )
    LaunchedEffect(Unit) { focus.requestFocus() }
}

data class FolderEdit(val id: String?, val title: String, val icon: String = "", val color: String = "")

private val folderColors = listOf("", "#d9622b", "#c2412d", "#c9a227", "#5e8c4a", "#3f7cac", "#3f51a6", "#8a5cb8", "#9b958b")
private val folderIcons = listOf("", "📓", "📝", "📚", "💡", "✏️", "🌙", "🌸", "⭐", "🎵", "🏠", "💼")

@Composable
private fun FolderDialog(
    edit: FolderEdit,
    onDismiss: () -> Unit,
    onSave: (String, String, String) -> Unit,
    onDelete: (() -> Unit)?,
    onMove: (() -> Unit)? = null,
    onNewSub: (() -> Unit)? = null,
    isWork: Boolean? = null,
    onWork: ((Boolean) -> Unit)? = null,
) {
    var title by remember { mutableStateOf(edit.title) }
    var icon by remember { mutableStateOf(edit.icon) }
    var color by remember { mutableStateOf(edit.color) }
    val focus = remember { FocusRequester() }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (edit.id == null) tr("新しいノートブック", "New notebook") else tr("ノートブック", "Notebook")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                OutlinedTextField(
                    value = title,
                    onValueChange = { title = it },
                    label = { Text(tr("名前", "Name")) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().focusRequester(focus),
                )
                if (edit.id != null) {
                    Text(tr("アイコン", "Icon"), style = MaterialTheme.typography.labelLarge)
                    ChoiceRow(folderIcons, icon, { icon = it }) { value, selected ->
                        Text(value.ifEmpty { "—" }, style = MaterialTheme.typography.titleMedium, color = if (selected) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurface)
                    }
                    Text(tr("色", "Colour"), style = MaterialTheme.typography.labelLarge)
                    ChoiceRow(folderColors, color, { color = it }) { value, _ ->
                        val c = parseColor(value)
                        Box(Modifier.size(22.dp).clip(RoundedCornerShape(50)).background(c ?: MaterialTheme.colorScheme.surfaceVariant))
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        onNewSub?.let { AssistChip(onClick = it, label = { Text(tr("サブノートブック", "Sub-notebook")) }, leadingIcon = { Icon(Icons.Outlined.CreateNewFolder, null, Modifier.size(18.dp)) }) }
                        onMove?.let { AssistChip(onClick = it, label = { Text(tr("移動", "Move")) }, leadingIcon = { Icon(Icons.Outlined.DriveFileMove, null, Modifier.size(18.dp)) }) }
                    }
                    if (onWork != null && isWork != null) {
                        AssistChip(
                            onClick = { onWork(!isWork) },
                            label = { Text(if (isWork) tr("作品をやめる", "No longer a work") else tr("作品にする（小説など）", "Make it a work (a novel…)")) },
                            leadingIcon = { Icon(Icons.Outlined.AutoStories, null, Modifier.size(18.dp)) },
                        )
                    }
                }
            }
        },
        confirmButton = { Button(onClick = { onSave(title.trim(), icon, color) }, enabled = title.isNotBlank()) { Text(tr("保存", "Save")) } },
        dismissButton = {
            Row {
                if (onDelete != null) TextButton(onClick = onDelete) { Text(tr("削除", "Delete"), color = MaterialTheme.colorScheme.error) }
                TextButton(onClick = onDismiss) { Text(tr("キャンセル", "Cancel")) }
            }
        },
    )
    LaunchedEffect(Unit) { if (edit.id == null) focus.requestFocus() }
}

/** A wrapping row of choices (emoji, colours), the chosen one marked. */
@Composable
private fun ChoiceRow(values: List<String>, selected: String, onPick: (String) -> Unit, content: @Composable (String, Boolean) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        values.forEach { v ->
            val on = v == selected
            Box(
                Modifier
                    .size(40.dp)
                    .clip(RoundedCornerShape(if (on) 12.dp else 20.dp))
                    .background(if (on) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh)
                    .clickable { onPick(v) },
                contentAlignment = Alignment.Center,
            ) { content(v, on) }
        }
    }
}

/** The ids of a notebook's sub-notebooks, at every depth. */
private fun descendants(folders: List<Folder>, id: String): Set<String> {
    val out = mutableSetOf<String>()
    fun walk(p: String) {
        for (f in folders) if (f.parentId == p && out.add(f.id)) walk(f.id)
    }
    walk(id)
    return out
}

/** A tag: renamed, or deleted (from every note). */
@Composable
private fun TagDialog(tag: app.nisshoku.android.engine.Tag, onDismiss: () -> Unit, onRename: (String) -> Unit, onDelete: () -> Unit) {
    var title by remember { mutableStateOf(tag.title) }
    var confirm by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("タグ", "Tag")) },
        text = {
            OutlinedTextField(value = title, onValueChange = { title = it }, label = { Text(tr("名前", "Name")) }, singleLine = true, modifier = Modifier.fillMaxWidth())
        },
        confirmButton = { Button(onClick = { onRename(title.trim()) }, enabled = title.isNotBlank()) { Text(tr("保存", "Save")) } },
        dismissButton = {
            Row {
                TextButton(onClick = { confirm = true }) { Text(tr("削除", "Delete"), color = MaterialTheme.colorScheme.error) }
                TextButton(onClick = onDismiss) { Text(tr("キャンセル", "Cancel")) }
            }
        },
    )
    if (confirm) {
        ConfirmDialog(
            title = tr("タグを削除しますか？", "Delete the tag?"),
            text = tr("「${tag.title}」をすべてのノートから外します。ノートは消えません。", "\"${tag.title}\" is taken off every note. The notes stay."),
            confirm = tr("削除", "Delete"),
            onDismiss = { confirm = false },
            onConfirm = onDelete,
        )
    }
}

/** A work's notebook: the way to its overview (manuscript, characters, plot, materials). */
@Composable
private fun WorkCard(title: String, onOpen: () -> Unit) {
    Surface(
        shape = RoundedCornerShape(24.dp),
        color = MaterialTheme.colorScheme.tertiaryContainer,
        modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp).clip(RoundedCornerShape(24.dp)).clickable(onClick = onOpen),
    ) {
        Row(Modifier.padding(18.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Outlined.AutoStories, null, tint = MaterialTheme.colorScheme.onTertiaryContainer)
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onTertiaryContainer)
                Text(tr("原稿・人物・プロット・資料", "Manuscript, characters, plot, materials"), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onTertiaryContainer)
            }
            Icon(Icons.AutoMirrored.Outlined.KeyboardArrowRight, null, tint = MaterialTheme.colorScheme.onTertiaryContainer)
        }
    }
}
