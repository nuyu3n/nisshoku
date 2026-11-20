package app.nisshoku.android.engine

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.put

// The engine's data, as its commands give it (crates/nisshoku-engine; the
// web app's api.ts has the same shapes).

@Serializable
data class Folder(
    val id: String,
    val title: String,
    val parentId: String = "",
    val icon: String = "",
    val color: String = "",
    val isDiary: Boolean = false,
    val isWork: Boolean = false,
    val noteCount: Int = 0,
)

@Serializable
data class NoteSummary(
    val id: String,
    val parentId: String = "",
    val title: String = "",
    val excerpt: String = "",
    val charCount: Int = 0,
    val createdTime: Long = 0,
    val updatedTime: Long = 0,
    val isConflict: Boolean = false,
    val deletedTime: Long = 0,
    val diaryDate: String? = null,
    val pinned: Boolean = false,
    val isTodo: Boolean = false,
    val todoCompleted: Boolean = false,
    val todoDue: Long = 0,
    val dueAllDay: Boolean = false,
    val completedTime: Long = 0,
    val order: Double = 0.0,
    val goal: Long? = null,
    val kind: String? = null,
    val tags: List<String> = emptyList(),
    /** `save_note`: the note had changed elsewhere, and that version was kept as this copy. */
    val conflictId: String? = null,
)

@Serializable
data class Tag(val id: String, val title: String = "", val noteCount: Int = 0)

/** A day of the diary that has an entry. */
@Serializable
data class DiaryDay(val date: String, val noteId: String = "", val charCount: Int = 0, val mood: String? = null)

@Serializable
data class Note(
    val id: String,
    val parentId: String = "",
    val title: String = "",
    val body: String = "",
    val createdTime: Long = 0,
    val updatedTime: Long = 0,
    val deletedTime: Long = 0,
    val isConflict: Boolean = false,
    val diaryDate: String? = null,
    val pinned: Boolean = false,
    val isTodo: Boolean = false,
    val todoCompleted: Boolean = false,
    val todoDue: Long = 0,
    val dueAllDay: Boolean = false,
    val kind: String? = null,
)

@Serializable
data class LinkedNote(val id: String, val title: String = "")

@Serializable
data class Revision(val id: String, val time: Long = 0)

@Serializable
data class RevisionContent(val title: String = "", val body: String = "")

@Serializable
data class Comment(
    val id: String,
    val quote: String = "",
    val before: String = "",
    val text: String = "",
    val createdTime: Long = 0,
    val updatedTime: Long = 0,
    val resolved: Boolean = false,
)

@Serializable
data class DiaryEvent(val time: String = "", val text: String = "")

@Serializable
data class WorkMeta(
    val logline: String = "",
    val synopsis: String = "",
    val genre: String = "",
    val targetChars: Long? = null,
    val deadline: String? = null,
)

@Serializable
data class ChapterInfo(
    val id: String,
    val title: String = "",
    val excerpt: String = "",
    val charCount: Int = 0,
    val updatedTime: Long = 0,
    val status: String = "",
    val synopsis: String = "",
    val pov: String? = null,
)

@Serializable
data class WorkSection(val folderId: String, val title: String = "", val depth: Int = 0, val chapters: List<ChapterInfo> = emptyList())

@Serializable
data class CharacterInfo(
    val id: String,
    val name: String = "",
    val ruby: String = "",
    val role: String = "",
    val color: String = "",
    val aliases: List<String> = emptyList(),
    val excerpt: String = "",
    val mentions: Int = 0,
    val chapters: List<String> = emptyList(),
)

@Serializable
data class WorkOverview(
    val id: String,
    val title: String = "",
    val meta: WorkMeta = WorkMeta(),
    val sections: List<WorkSection> = emptyList(),
    val characters: List<CharacterInfo> = emptyList(),
    val plotId: String? = null,
    val materials: List<NoteSummary> = emptyList(),
    val totalChars: Long = 0,
    val chapterCount: Int = 0,
)

@Serializable
data class Relation(val target: String = "", val label: String = "")

@Serializable
data class Character(
    val ruby: String = "",
    val role: String = "",
    val age: String = "",
    val gender: String = "",
    val appearance: String = "",
    val personality: String = "",
    val background: String = "",
    val motivation: String = "",
    val aliases: List<String> = emptyList(),
    val color: String = "",
    val relations: List<Relation> = emptyList(),
)

@Serializable
data class PlotColumn(val id: String, val title: String = "")

@Serializable
data class PlotCard(
    val id: String,
    val column: String = "",
    val title: String = "",
    val text: String = "",
    val chapter: String? = null,
    val done: Boolean = false,
)

@Serializable
data class Plot(val template: String = "", val columns: List<PlotColumn> = emptyList(), val cards: List<PlotCard> = emptyList())

@Serializable
data class PlotDoc(val noteId: String, val plot: Plot = Plot())

/** Writing figures: today's characters, the streak, the last 30 days (`[date, chars]`). */
@Serializable
data class Stats(
    val today: Long = 0,
    val streak: Int = 0,
    val days: List<kotlinx.serialization.json.JsonArray> = emptyList(),
    val history: List<kotlinx.serialization.json.JsonArray> = emptyList(),
    val totalNotes: Int = 0,
    val totalChars: Long = 0,
)

@Serializable
data class DiaryStats(
    val streak: Int = 0,
    val longestStreak: Int = 0,
    val monthDays: Int = 0,
    val totalDays: Int = 0,
    val totalChars: Long = 0,
)

@Serializable
data class DiaryEntry(
    val noteId: String,
    val date: String = "",
    val title: String = "",
    val excerpt: String = "",
    val charCount: Int = 0,
    val mood: String? = null,
    val weather: String? = null,
    val events: List<DiaryEvent> = emptyList(),
)

@Serializable
data class ResourceFile(val id: String, val path: String = "", val mime: String = "", val title: String = "", val size: Long = 0)

@Serializable
data class StorageInfo(
    val databaseBytes: Long = 0,
    val resourceBytes: Long = 0,
    val resourceCount: Int = 0,
    val unusedCount: Int = 0,
    val unusedBytes: Long = 0,
)

@Serializable
data class SyncReport(
    val created_remote: Int = 0,
    val updated_remote: Int = 0,
    val deleted_remote: Int = 0,
    val created_local: Int = 0,
    val updated_local: Int = 0,
    val deleted_local: Int = 0,
    val conflicts: Int = 0,
    val errors: List<String> = emptyList(),
)

@Serializable
data class SyncStatus(
    /** `off`, `idle`, `syncing` or `error`. */
    val state: String = "off",
    val message: String? = null,
    val lastSync: Long = 0,
    val needsPassword: Boolean = false,
    val lastReport: SyncReport? = null,
)

@Serializable
data class SyncProgress(
    /** `running` or `done`. */
    val state: String = "done",
    val done: Int = 0,
    val total: Int = 0,
    val text: String = "",
    val time: Long = 0,
)

/** Where the notes sync to (`kind`: none, local, nextcloud, googleDrive, oneDrive). */
@Serializable
data class SyncTarget(
    val kind: String = "none",
    val server: String? = null,
    val user: String? = null,
    val folder: String? = null,
    val clientId: String? = null,
    val path: String? = null,
)

@Serializable
data class SyncSettings(
    val target: SyncTarget = SyncTarget(),
    val status: SyncStatus = SyncStatus(),
    val gdriveAvailable: Boolean = false,
    val hasMasterPassword: Boolean = false,
)

@Serializable
data class Release(
    val version: String = "",
    val notes: String = "",
    val url: String = "",
    val apkUrl: String? = null,
)

/** Which notes a list shows (notes.rs `NoteScope`). */
sealed interface Scope {
    val json: JsonObject

    data object All : Scope {
        override val json = kind("all")
    }

    data object Diary : Scope {
        override val json = kind("diary")
    }

    data object Todo : Scope {
        override val json = kind("todo")
    }

    data object Pinned : Scope {
        override val json = kind("pinned")
    }

    data object Trash : Scope {
        override val json = kind("trash")
    }

    data object Conflicts : Scope {
        override val json = kind("conflicts")
    }

    data class InFolder(val id: String) : Scope {
        override val json = buildJsonObject { put("kind", "folder"); put("id", id) }
    }

    data class WithTag(val id: String) : Scope {
        override val json = buildJsonObject { put("kind", "tag"); put("id", id) }
    }

    data class Search(val query: String) : Scope {
        override val json = buildJsonObject { put("kind", "search"); put("query", query) }
    }

    companion object {
        private fun kind(k: String) = buildJsonObject { put("kind", k) }
    }
}

private fun args(vararg pairs: Pair<String, Any?>): JsonObject = JsonObject(
    pairs.associate { (k, v) ->
        k to when (v) {
            null -> JsonNull
            is JsonElement -> v
            is String -> JsonPrimitive(v)
            is Number -> JsonPrimitive(v)
            is Boolean -> JsonPrimitive(v)
            else -> error("unsupported argument $k")
        }
    },
)

private suspend inline fun <reified T> get(cmd: String, vararg pairs: Pair<String, Any?>): T =
    Nisshoku.json.decodeFromJsonElement(Nisshoku.call(cmd, args(*pairs)))

/** The commands the native screens use (the editor calls the rest itself). */
object Api {
    suspend fun folders(): List<Folder> = get("list_folders")
    suspend fun notes(scope: Scope): List<NoteSummary> = get("list_notes", "scope" to scope.json)
    suspend fun note(id: String): Note = get("get_note", "id" to id)
    suspend fun createNote(parentId: String?, title: String? = null, body: String? = null): Note =
        get("create_note", "parentId" to parentId, "title" to title, "body" to body)

    suspend fun createTask(title: String, parentId: String?, due: Long?, allDay: Boolean): NoteSummary =
        get("create_task", "title" to title, "parentId" to parentId, "due" to due, "allDay" to allDay)

    suspend fun setTodoDue(id: String, due: Long, allDay: Boolean) =
        Nisshoku.call("set_todo_due", args("id" to id, "due" to due, "allDay" to allDay))

    suspend fun setNoteTodo(id: String, todo: Boolean) = Nisshoku.call("set_note_todo", args("id" to id, "todo" to todo))
    suspend fun saveNote(id: String, title: String, body: String, baseTitle: String?, baseBody: String?): NoteSummary =
        get("save_note", "id" to id, "title" to title, "body" to body, "baseTitle" to baseTitle, "baseBody" to baseBody)

    suspend fun tags(): List<Tag> = get("list_tags")
    suspend fun setNoteTags(noteId: String, titles: List<String>) =
        Nisshoku.call("set_note_tags", args("noteId" to noteId, "titles" to kotlinx.serialization.json.JsonArray(titles.map { JsonPrimitive(it) })))

    suspend fun renameTag(id: String, title: String) = Nisshoku.call("rename_tag", args("id" to id, "title" to title))
    suspend fun deleteTag(id: String) = Nisshoku.call("delete_tag", args("id" to id))
    suspend fun duplicateNote(id: String): Note = get("duplicate_note", "id" to id)
    suspend fun saveAsTemplate(noteId: String): String = get("save_as_template", "noteId" to noteId)
    suspend fun templates(): List<LinkedNote> = get("list_templates")
    suspend fun clipUrl(url: String, parentId: String?): Note = get("clip_url", "url" to url, "parentId" to parentId)
    suspend fun moveFolder(id: String, parentId: String) = Nisshoku.call("move_folder", args("id" to id, "parentId" to parentId))
    suspend fun resolveConflict(id: String, keep: String): String = get("resolve_conflict", "id" to id, "keep" to keep)
    suspend fun stats(): Stats = get("get_stats")
    suspend fun diaryStats(today: String): DiaryStats = get("diary_stats", "today" to today)
    suspend fun onThisDay(date: String): List<DiaryEntry> = get("on_this_day", "date" to date)
    suspend fun diaryEntries(query: String? = null): List<DiaryEntry> = get("diary_entries", "query" to query)
    suspend fun setDiaryEvents(noteId: String, events: List<DiaryEvent>) =
        Nisshoku.call("set_diary_events", args("noteId" to noteId, "events" to Nisshoku.json.encodeToJsonElement(events)))

    suspend fun revisions(noteId: String): List<Revision> = get("list_revisions", "noteId" to noteId)
    suspend fun revisionContent(noteId: String, revisionId: String): RevisionContent =
        get("revision_content", "noteId" to noteId, "revisionId" to revisionId)

    suspend fun restoreRevision(noteId: String, revisionId: String): Note = get("restore_revision", "noteId" to noteId, "revisionId" to revisionId)
    suspend fun saveSnapshot(noteId: String): Boolean = get("save_snapshot", "noteId" to noteId)

    suspend fun comments(noteId: String): List<Comment> = get("list_comments", "noteId" to noteId)
    suspend fun addComment(noteId: String, quote: String, before: String, text: String): Comment =
        get("add_comment", "noteId" to noteId, "quote" to quote, "before" to before, "text" to text)

    suspend fun updateComment(noteId: String, commentId: String, text: String?, resolved: Boolean?) =
        Nisshoku.call("update_comment", args("noteId" to noteId, "commentId" to commentId, "text" to text, "resolved" to resolved))

    suspend fun deleteComment(noteId: String, commentId: String) = Nisshoku.call("delete_comment", args("noteId" to noteId, "commentId" to commentId))

    suspend fun setWork(folderId: String, meta: WorkMeta?) =
        Nisshoku.call("set_work", args("folderId" to folderId, "meta" to meta?.let { Nisshoku.json.encodeToJsonElement(it) }))

    suspend fun workOverview(workId: String): WorkOverview = get("work_overview", "workId" to workId)
    suspend fun setNoteKind(noteId: String, kind: String?) = Nisshoku.call("set_note_kind", args("noteId" to noteId, "kind" to kind))
    suspend fun setChapterMeta(noteId: String, status: String, synopsis: String, pov: String?) =
        Nisshoku.call("set_chapter_meta", args("noteId" to noteId, "meta" to buildJsonObject {
            put("status", status); put("synopsis", synopsis); put("pov", pov)
        }))

    suspend fun createWorkNote(workId: String, kind: String, title: String): Note =
        get("create_work_note", "workId" to workId, "kind" to kind, "title" to title)

    suspend fun character(noteId: String): Character = get("get_character", "noteId" to noteId)
    suspend fun setCharacter(noteId: String, character: Character) =
        Nisshoku.call("set_character", args("noteId" to noteId, "character" to Nisshoku.json.encodeToJsonElement(character)))

    suspend fun openPlot(workId: String, template: String?): PlotDoc = get("open_plot", "workId" to workId, "template" to template)
    suspend fun setPlot(noteId: String, plot: Plot) =
        Nisshoku.call("set_plot", args("noteId" to noteId, "plot" to Nisshoku.json.encodeToJsonElement(plot)))

    suspend fun saveResource(data: ByteArray, name: String, mime: String): ResourceFile =
        Nisshoku.json.decodeFromString(Nisshoku.saveResource(data, name, mime))

    suspend fun resourceFiles(): List<ResourceFile> = get("resource_files")
    suspend fun storageInfo(): StorageInfo = get("storage_info")
    suspend fun deleteUnusedResources(): Int = get("delete_unused_resources")
    suspend fun diaryMonth(month: String): List<DiaryDay> = get("diary_month", "month" to month)
    suspend fun refreshWidgets() = Nisshoku.call("refresh_widgets")
    suspend fun applyWidgetActions(): Int = get("apply_widget_actions")

    suspend fun openDiary(date: String): Note = get("open_diary", "date" to date)
    suspend fun trashNote(id: String) = Nisshoku.call("trash_note", args("id" to id))
    suspend fun restoreNote(id: String) = Nisshoku.call("restore_note", args("id" to id))
    suspend fun deleteForever(id: String) = Nisshoku.call("delete_note_forever", args("id" to id))
    suspend fun emptyTrash(): Int = get("empty_trash")
    suspend fun setPinned(id: String, pinned: Boolean) = Nisshoku.call("set_note_pinned", args("id" to id, "pinned" to pinned))
    suspend fun setTodoCompleted(id: String, completed: Boolean) =
        Nisshoku.call("set_todo_completed", args("id" to id, "completed" to completed))

    suspend fun moveNote(id: String, parentId: String) = Nisshoku.call("move_note", args("id" to id, "parentId" to parentId))
    suspend fun createFolder(title: String, parentId: String?): String =
        get("create_folder", "title" to title, "parentId" to parentId)

    suspend fun renameFolder(id: String, title: String) = Nisshoku.call("rename_folder", args("id" to id, "title" to title))
    suspend fun setFolderIcon(id: String, emoji: String?) = Nisshoku.call("set_folder_icon", args("id" to id, "emoji" to emoji))
    suspend fun setFolderColor(id: String, color: String?) = Nisshoku.call("set_folder_color", args("id" to id, "color" to color))
    suspend fun trashFolder(id: String) = Nisshoku.call("trash_folder", args("id" to id))

    suspend fun preferences(): JsonObject = Nisshoku.call("get_preferences") as? JsonObject ?: JsonObject(emptyMap())
    suspend fun setPreferences(prefs: JsonObject) = Nisshoku.call("set_preferences", args("preferences" to prefs))

    suspend fun syncSettings(): SyncSettings = get("get_sync_settings")
    suspend fun syncNow(): SyncStatus = get("sync_now")
    suspend fun hasUnsentChanges(): Boolean = get("has_unsent_changes")
    suspend fun setForeground(visible: Boolean) = Nisshoku.call("set_foreground", args("visible" to visible))
    suspend fun disconnectSync() = Nisshoku.call("disconnect_sync")
    suspend fun nextcloudLoginStart(server: String): String = get("nextcloud_login_start", "server" to server)
    suspend fun nextcloudLoginWait(folder: String) = Nisshoku.call("nextcloud_login_wait", args("folder" to folder))
    suspend fun nextcloudConnect(server: String, user: String, password: String, folder: String) =
        Nisshoku.call("nextcloud_connect", args("server" to server, "user" to user, "password" to password, "folder" to folder))

    suspend fun gdriveLogin(folder: String, clientId: String?, clientSecret: String?) =
        Nisshoku.call("gdrive_login", args("folder" to folder, "clientId" to clientId, "clientSecret" to clientSecret))

    suspend fun onedriveLogin(clientId: String, folder: String) =
        Nisshoku.call("onedrive_login", args("clientId" to clientId, "folder" to folder))

    suspend fun setMasterPassword(password: String): SyncStatus = get("set_master_password", "password" to password)
    suspend fun setEncryption(enabled: Boolean, password: String?) =
        Nisshoku.call("set_encryption", args("enabled" to enabled, "password" to password))

    suspend fun latestRelease(): Release = get("latest_release")
}
