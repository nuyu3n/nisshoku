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
import androidx.compose.material.icons.outlined.TaskAlt
import androidx.compose.material.icons.outlined.Today
import androidx.compose.material.icons.outlined.WarningAmber
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
    is Scope.Search -> scope.query
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun HomeScreen(
    model: AppModel,
    snackbar: SnackbarHostState,
    onOpen: (String) -> Unit,
    onSearch: () -> Unit,
    onSettings: () -> Unit,
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
    var folderEdit by remember { mutableStateOf<FolderEdit?>(null) }

    BackHandler(drawer.isOpen) { scope.launch { drawer.close() } }
    BackHandler(fabOpen) { fabOpen = false }

    ModalNavigationDrawer(
        drawerState = drawer,
        drawerContent = {
            Drawer(
                state = state,
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
                        Text(if (state.loading) " " else tr("$count 件", if (count == 1) "1 note" else "$count notes"))
                    },
                    navigationIcon = {
                        IconButton(onClick = { scope.launch { drawer.open() } }) {
                            Icon(Icons.Outlined.Menu, tr("メニュー", "Menu"))
                        }
                    },
                    actions = {
                        IconButton(onClick = onSearch) { Icon(Icons.Outlined.Search, tr("検索", "Search")) }
                        SyncButton(sync.status.state, sync.target.kind != "none", onSync = model::syncNow, onSetUp = onSettings)
                    },
                    scrollBehavior = bar,
                )
            },
            floatingActionButton = {
                if (state.scope != Scope.Trash) {
                    FabMenu(
                        open = fabOpen,
                        onOpenChange = {
                            haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                            fabOpen = it
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
                    onEmptyTrash = model::emptyTrash,
                )
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
            onDelete = { model.deleteForever(note) },
        )
    }
    movingNote?.let { note ->
        MoveSheet(
            folders = state.folders.filter { !it.isDiary },
            onDismiss = { movingNote = null },
            onPick = {
                model.move(note, it.id)
                movingNote = null
            },
        )
    }
    if (newTask) {
        TextPrompt(
            title = tr("新しいタスク", "New task"),
            label = tr("やること", "To do"),
            confirm = tr("追加", "Add"),
            onDismiss = { newTask = false },
            onDone = { title ->
                scope.launch { if (model.newTask(title)) snackbar.showSnackbar(trNow("タスクを追加しました", "Task added")) }
                newTask = false
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
private fun FabMenu(open: Boolean, onOpenChange: (Boolean) -> Unit, onNote: () -> Unit, onTask: () -> Unit, onDiary: () -> Unit) {
    FloatingActionButtonMenu(
        expanded = open,
        button = {
            ToggleFloatingActionButton(
                checked = open,
                onCheckedChange = onOpenChange,
                modifier = Modifier.semantics { contentDescription = trNow("新規作成", "Create") },
            ) {
                val icon by remember { derivedStateOf { if (checkedProgress > 0.5f) Icons.Outlined.Close else Icons.Outlined.Edit } }
                Icon(
                    painter = rememberVectorPainter(icon),
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
) {
    val list = rememberLazyListState()
    if (state.notes.isEmpty()) {
        AnimatedVisibility(!state.loading, enter = fadeIn(), exit = fadeOut()) {
            // Scrollable, so that pulling still syncs.
            Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
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
    val pinned = state.notes.filter { it.pinned && state.scope != Scope.Trash }
    val rest = state.notes.filter { !(it.pinned && state.scope != Scope.Trash) }
    LazyColumn(
        state = list,
        contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = 4.dp, bottom = 120.dp),
        verticalArrangement = Arrangement.spacedBy(3.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
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
                modifier = Modifier.padding(start = 16.dp, top = 24.dp, bottom = 16.dp),
            )
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
            HorizontalDivider(Modifier.padding(horizontal = 16.dp, vertical = 12.dp))
            DrawerItem(tr("競合", "Conflicts"), Icons.Outlined.WarningAmber, item(Scope.Conflicts)) { onScope(Scope.Conflicts) }
            DrawerItem(tr("ゴミ箱", "Trash"), Icons.Outlined.Delete, item(Scope.Trash)) { onScope(Scope.Trash) }
            DrawerItem(tr("設定", "Settings"), Icons.Outlined.Settings, false, onSettings)
            Spacer(Modifier.height(24.dp).navigationBarsPadding())
        }
    }
}

@Composable
private fun DrawerItem(label: String, icon: ImageVector, selected: Boolean, onClick: () -> Unit) {
    NavigationDrawerItem(
        label = { Text(label) },
        icon = { Icon(icon, null) },
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
        Column(Modifier.padding(horizontal = 12.dp).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            val actions = if (inTrash) listOf(
                Triple(tr("元に戻す", "Restore"), Icons.Outlined.Restore, onRestore),
                Triple(tr("完全に削除", "Delete forever"), Icons.Outlined.DeleteForever, onDelete),
            ) else listOf(
                Triple(if (note.pinned) tr("ピン留めを外す", "Unpin") else tr("ピン留め", "Pin"), Icons.Outlined.PushPin, onPin),
                Triple(tr("ノートブックへ移動", "Move to notebook"), Icons.Outlined.DriveFileMove, onMove),
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
private fun MoveSheet(folders: List<Folder>, onDismiss: () -> Unit, onPick: (Folder) -> Unit) {
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Text(tr("移動先", "Move to"), style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
        LazyColumn(contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
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
private fun FolderDialog(edit: FolderEdit, onDismiss: () -> Unit, onSave: (String, String, String) -> Unit, onDelete: (() -> Unit)?) {
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
