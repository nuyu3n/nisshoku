package app.nisshoku.android.ui

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import app.nisshoku.android.Lang
import app.nisshoku.android.engine.Api
import app.nisshoku.android.BuildConfig
import app.nisshoku.android.Shortcuts
import app.nisshoku.android.engine.Folder
import app.nisshoku.android.engine.Release
import app.nisshoku.android.engine.Tag
import app.nisshoku.desktop.Widgets
import kotlinx.coroutines.delay
import app.nisshoku.android.engine.Nisshoku
import app.nisshoku.android.engine.NoteSummary
import app.nisshoku.android.engine.Scope
import app.nisshoku.android.engine.SyncProgress
import app.nisshoku.android.engine.SyncSettings
import app.nisshoku.android.engine.SyncStatus
import app.nisshoku.android.sync.SyncService
import app.nisshoku.android.sync.SyncWorker
import app.nisshoku.android.trNow
import app.nisshoku.android.ui.theme.ThemeSettings
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonPrimitive
import java.time.LocalDate

/** A message for the snackbar, with an optional action. */
data class Message(val text: String, val action: String? = null, val run: (suspend () -> Unit)? = null)

data class HomeState(
    val ready: Boolean = false,
    val folders: List<Folder> = emptyList(),
    val tags: List<Tag> = emptyList(),
    /** Counts up with every reload (what shows other data reloads with it). */
    val version: Int = 0,
    val scope: Scope = Scope.All,
    val notes: List<NoteSummary> = emptyList(),
    val loading: Boolean = true,
)

/** The app's state over the engine, for all screens. */
class AppModel(app: Application) : AndroidViewModel(app) {
    private val _home = MutableStateFlow(HomeState())
    val home: StateFlow<HomeState> = _home.asStateFlow()

    private val _prefs = MutableStateFlow(JsonObject(emptyMap()))
    val prefs: StateFlow<JsonObject> = _prefs.asStateFlow()

    private val _sync = MutableStateFlow(SyncSettings())
    val sync: StateFlow<SyncSettings> = _sync.asStateFlow()

    private val _progress = MutableStateFlow<SyncProgress?>(null)
    val progress: StateFlow<SyncProgress?> = _progress.asStateFlow()

    private val _messages = Channel<Message>(Channel.BUFFERED)
    val messages = _messages.receiveAsFlow()

    private var loadJob: Job? = null
    private var widgetJob: Job? = null

    /** Asks the home screen for its "new task" sheet (a count, so that each ask is seen). */
    private val _newTaskRequests = MutableStateFlow(0)
    val newTaskRequests: StateFlow<Int> = _newTaskRequests.asStateFlow()

    fun requestNewTask() {
        _newTaskRequests.update { it + 1 }
    }

    /** What changed in the versions since the one last opened, to show once. */
    private val _whatsNew = MutableStateFlow<List<ReleaseNotes>?>(null)
    val whatsNew: StateFlow<List<ReleaseNotes>?> = _whatsNew.asStateFlow()

    /** A newer release with a package to install. */
    private val _update = MutableStateFlow<Release?>(null)
    val update: StateFlow<Release?> = _update.asStateFlow()

    val version: String = BuildConfig.VERSION_NAME.removeSuffix("-dev")

    /** Notes were written since the last sync (sent in the background on leaving). */
    private var unsent = false

    init {
        viewModelScope.launch { start() }
        viewModelScope.launch {
            Nisshoku.events.collect { e ->
                when (e.name) {
                    "sync-status" -> runCatching { Nisshoku.json.decodeFromString<SyncStatus>(e.payload) }.getOrNull()?.let { onStatus(it) }
                    "sync-progress" -> {
                        val p = runCatching { Nisshoku.json.decodeFromString<SyncProgress>(e.payload) }.getOrNull()
                        _progress.value = p?.takeIf { it.state == "running" }
                    }
                    "data-changed" -> reload()
                }
            }
        }
        viewModelScope.launch { Nisshoku.widgetActions.collect { applyWidgetActions() } }
    }

    private suspend fun start() {
        try {
            loadPrefs()
            _sync.value = Api.syncSettings()
            runCatching { Api.applyWidgetActions() }
            reload()
            _home.update { it.copy(ready = true) }
            Shortcuts.update(getApplication())
            showWhatsNew()
            if (prefBool("autoUpdateCheck", true)) viewModelScope.launch { checkForUpdate(quiet = true) }
            if (_sync.value.target.kind != "none") {
                SyncWorker.schedule(getApplication(), pref("backgroundSyncMinutes")?.jsonPrimitive?.intOrNull ?: 60)
            }
        } catch (e: Exception) {
            _home.update { it.copy(ready = true, loading = false) }
            say(e.message ?: "")
        }
    }

    private fun onStatus(status: SyncStatus) {
        val before = _sync.value.status
        _sync.update { it.copy(status = status) }
        if (status.state == "syncing" && before.state != "syncing") SyncService.start(getApplication())
        if (status.state == "idle" && status.lastSync != before.lastSync) unsent = false
        if (status.state == "error" && before.state != "error" && !status.message.isNullOrEmpty()) say(status.message)
    }

    fun say(text: String, action: String? = null, run: (suspend () -> Unit)? = null) {
        _messages.trySend(Message(text, action, run))
    }

    // ---------------------------------------------------------------------
    // Versions

    private fun showWhatsNew() {
        val seen = prefString("lastSeenVersion", "")
        if (seen == version) return
        setPref("lastSeenVersion", JsonPrimitive(version))
        val all = ReleaseNotes.bundled(getApplication())
        // A new install shows nothing; an update, what came since.
        val shown = if (seen.isEmpty()) emptyList() else all.filter { isNewer(it.version, seen) && !isNewer(it.version, version) }
        if (shown.any { it.items.isNotEmpty() }) _whatsNew.value = shown
    }

    fun showAllReleases() {
        _whatsNew.value = ReleaseNotes.bundled(getApplication())
    }

    fun closeWhatsNew() {
        _whatsNew.value = null
    }

    /** Looks for a newer release; `quiet` says nothing when there is none (or no network). */
    suspend fun checkForUpdate(quiet: Boolean) {
        val release = runCatching { Api.latestRelease() }.getOrElse {
            if (!quiet) say(it.message ?: "")
            return
        }
        if (isNewer(release.version, version) && release.apkUrl != null) _update.value = release
        else if (!quiet) say(trNow("最新のバージョンです", "You have the latest version"))
    }

    fun dismissUpdate() {
        _update.value = null
    }

    // ---------------------------------------------------------------------
    // Widgets

    /** Rewrites what the home-screen widgets show, a moment after the last change. */
    private fun refreshWidgets() {
        widgetJob?.cancel()
        widgetJob = Nisshoku.scope.launch {
            delay(800)
            runCatching { Api.refreshWidgets() }
            runCatching { Widgets.refreshAll(getApplication()) }
        }
    }

    private suspend fun applyWidgetActions() {
        val changed = runCatching { Api.applyWidgetActions() }.getOrDefault(0)
        if (changed > 0) {
            unsent = true
            reload()
        }
    }

    // ---------------------------------------------------------------------
    // Preferences

    fun pref(key: String): JsonElement? = _prefs.value[key]

    fun prefString(key: String, default: String): String =
        (_prefs.value[key] as? JsonPrimitive)?.takeIf { it.isString }?.content ?: default

    fun prefBool(key: String, default: Boolean): Boolean = (_prefs.value[key] as? JsonPrimitive)?.booleanOrNull ?: default

    fun prefInt(key: String, default: Int): Int = (_prefs.value[key] as? JsonPrimitive)?.intOrNull ?: default

    private suspend fun loadPrefs() {
        val prefs = Api.preferences()
        _prefs.value = prefs
        (prefs["language"] as? JsonPrimitive)?.content?.let { Lang.english = it == "en" }
    }

    fun setPref(key: String, value: JsonElement) {
        val next = JsonObject(_prefs.value + (key to value))
        _prefs.value = next
        if (key == "language") {
            Lang.english = (value as? JsonPrimitive)?.content == "en"
            Shortcuts.update(getApplication())
        }
        viewModelScope.launch {
            try {
                Api.setPreferences(next)
                if (key == "backgroundSyncMinutes") SyncWorker.schedule(getApplication(), prefInt(key, 60))
                if (key in setOf("language", "theme", "accent", "dailyGoal")) refreshWidgets()
            } catch (e: Exception) {
                say(e.message ?: "")
            }
        }
    }

    fun themeSettings(prefs: JsonObject): ThemeSettings = ThemeSettings(
        theme = (prefs["theme"] as? JsonPrimitive)?.content ?: "system",
        accent = (prefs["accent"] as? JsonPrimitive)?.content ?: "orange",
        dynamicColor = (prefs["materialYou"] as? JsonPrimitive)?.booleanOrNull ?: true,
    )

    // ---------------------------------------------------------------------
    // Notebooks and notes

    fun setScope(scope: Scope) {
        if (scope == _home.value.scope) return
        _home.update { it.copy(scope = scope, notes = emptyList(), loading = true) }
        reload()
    }

    fun reload() {
        loadJob?.cancel()
        loadJob = viewModelScope.launch {
            try {
                val scope = _home.value.scope
                val folders = Api.folders()
                val tags = Api.tags()
                val notes = Api.notes(scope)
                if (scope == _home.value.scope) {
                    _home.update { it.copy(folders = folders, tags = tags, notes = notes, loading = false, version = it.version + 1) }
                }
                refreshWidgets()
            } catch (e: Exception) {
                _home.update { it.copy(loading = false) }
                say(e.message ?: "")
            }
        }
    }

    /** A new note in the notebook shown (or the default one); its id. */
    suspend fun newNote(title: String? = null, body: String? = null): String? = try {
        val folder = (_home.value.scope as? Scope.InFolder)?.id
        Api.createNote(folder, title, body).id.also { unsent = true; reload() }
    } catch (e: Exception) {
        say(e.message ?: ""); null
    }

    suspend fun newTask(title: String, due: Long?, allDay: Boolean): Boolean = try {
        Api.createTask(title, (_home.value.scope as? Scope.InFolder)?.id, due, allDay)
        unsent = true
        reload()
        true
    } catch (e: Exception) {
        say(e.message ?: ""); false
    }

    /** Today's diary entry (made if needed); its id. */
    suspend fun todaysDiary(): String? = diaryOf(LocalDate.now().toString())

    /** The diary entry of a day (`YYYY-MM-DD`, made if needed); its id. */
    suspend fun diaryOf(date: String): String? = try {
        Api.openDiary(date).id.also { reload() }
    } catch (e: Exception) {
        say(e.message ?: ""); null
    }

    fun setDue(note: NoteSummary, due: Long, allDay: Boolean) = act { Api.setTodoDue(note.id, due, allDay) }
    fun toggleTodo(note: NoteSummary) = act { Api.setNoteTodo(note.id, !note.isTodo) }

    fun trash(note: NoteSummary) {
        viewModelScope.launch {
            try {
                Api.trashNote(note.id)
                unsent = true
                _home.update { s -> s.copy(notes = s.notes.filter { it.id != note.id }) }
                say(trNow("ゴミ箱に移動しました", "Moved to the trash"), trNow("元に戻す", "Undo")) {
                    Api.restoreNote(note.id)
                    reload()
                }
                reload()
            } catch (e: Exception) {
                say(e.message ?: "")
            }
        }
    }

    fun restore(note: NoteSummary) = act { Api.restoreNote(note.id) }
    fun deleteForever(note: NoteSummary) = act { Api.deleteForever(note.id) }
    fun emptyTrash() = act { Api.emptyTrash() }
    fun togglePinned(note: NoteSummary) = act { Api.setPinned(note.id, !note.pinned) }
    fun toggleDone(note: NoteSummary) = act { Api.setTodoCompleted(note.id, !note.todoCompleted) }
    fun move(note: NoteSummary, folderId: String) = act { Api.moveNote(note.id, folderId) }
    fun createFolder(title: String, parentId: String? = null) = act { Api.createFolder(title, parentId) }
    fun renameFolder(id: String, title: String) = act { Api.renameFolder(id, title) }
    fun setFolderIcon(id: String, emoji: String?) = act { Api.setFolderIcon(id, emoji) }
    fun setFolderColor(id: String, color: String?) = act { Api.setFolderColor(id, color) }
    fun trashFolder(id: String) = act {
        Api.trashFolder(id)
        if ((_home.value.scope as? Scope.InFolder)?.id == id) _home.update { it.copy(scope = Scope.All) }
    }

    private fun act(block: suspend () -> Unit) {
        viewModelScope.launch {
            try {
                block()
                unsent = true
                reload()
            } catch (e: Exception) {
                say(e.message ?: "")
            }
        }
    }

    /** The editor closed: what it wrote is sent by the next sync. */
    fun editorClosed() {
        unsent = true
        reload()
        // (The editor keeps preferences of its own: the last note, its panels.)
        viewModelScope.launch { runCatching { loadPrefs() } }
    }

    // ---------------------------------------------------------------------
    // Sync

    fun syncNow() {
        viewModelScope.launch {
            try {
                onStatus(Api.syncNow())
            } catch (e: Exception) {
                say(e.message ?: "")
            }
        }
    }

    suspend fun refreshSyncSettings() {
        runCatching { Api.syncSettings() }.getOrNull()?.let { _sync.value = it }
    }

    /** The app came to the front or left it. */
    fun setForeground(visible: Boolean) {
        Nisshoku.scope.launch { runCatching { Api.setForeground(visible) } }
        if (!visible && unsent && _sync.value.target.kind != "none" && !_sync.value.status.needsPassword) {
            SyncWorker.runOnce(getApplication())
        }
        if (visible) {
            viewModelScope.launch {
                runCatching { loadPrefs() }
                if (_home.value.ready) {
                    applyWidgetActions()
                    // (A background sync may have changed the notes meanwhile.)
                    reload()
                }
            }
        } else {
            refreshWidgets()
        }
    }
}
