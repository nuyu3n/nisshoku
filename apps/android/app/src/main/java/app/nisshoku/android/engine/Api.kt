package app.nisshoku.android.engine

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.decodeFromJsonElement
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
    val tags: List<String> = emptyList(),
)

@Serializable
data class Note(
    val id: String,
    val parentId: String = "",
    val title: String = "",
    val body: String = "",
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

    suspend fun createTask(title: String, parentId: String?): NoteSummary =
        get("create_task", "title" to title, "parentId" to parentId, "due" to null, "allDay" to null)

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
