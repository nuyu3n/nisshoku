package app.nisshoku.android.engine

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.util.Log
import androidx.core.content.FileProvider
import app.nisshoku.android.Notifications
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import java.io.File

/** The engine's JNI side: certificate checks and the log (crates/nisshoku-mobile/src/android.rs). */
object NativeInit {
    init {
        System.loadLibrary("nisshoku_mobile")
    }

    @JvmStatic external fun init(context: Context)
}

/** The message of a failed command, for the user. */
private fun EngineException.text(): String? = (this as? EngineException.Failed)?.reason ?: message

/** An event of the engine (`sync-status`, `sync-progress`, `data-changed`), its payload as JSON. */
data class EngineEvent(val name: String, val payload: String)

/** A command failed; the message is for the user. */
class CommandException(message: String) : Exception(message)

/**
 * The engine (the app's Rust core, crates/nisshoku-engine) over the profile,
 * opened once for the process. The commands are those the desktop window
 * calls, by name with JSON arguments.
 */
object Nisshoku {
    val json = Json {
        ignoreUnknownKeys = true
        explicitNulls = false
        encodeDefaults = true
        coerceInputValues = true
    }

    private val openLock = Mutex()
    @Volatile private var engine: Engine? = null
    private lateinit var appContext: Context

    private val _events = MutableSharedFlow<EngineEvent>(extraBufferCapacity = 64)
    val events: SharedFlow<EngineEvent> = _events.asSharedFlow()

    private val _widgetActions = MutableSharedFlow<Unit>(extraBufferCapacity = 4)

    /** A widget did something (ticked a task off): the open app applies it at once. */
    val widgetActions: SharedFlow<Unit> = _widgetActions.asSharedFlow()

    fun widgetAction() {
        _widgetActions.tryEmit(Unit)
    }

    /** Background work of the app's own (not tied to a screen). */
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    /** The profile: the app's data directory, as the Tauri build had it. */
    fun profileDir(context: Context): File = context.dataDir

    /** The application's context (for what needs one outside of a screen, like formatting dates). */
    val context: Context get() = appContext

    fun setUp(context: Context) {
        appContext = context.applicationContext
    }

    private suspend fun engine(): Engine {
        engine?.let { return it }
        return openLock.withLock {
            engine ?: withContext(Dispatchers.IO) {
                NativeInit.init(appContext)
                Engine.open(profileDir(appContext).path, appContext.packageName.removeSuffix(".dev"), Host(appContext))
            }.also { engine = it }
        }
    }

    /** Runs a command; its reply as JSON. */
    suspend fun call(cmd: String, args: JsonObject = JsonObject(emptyMap())): JsonElement {
        val engine = engine()
        val reply = try {
            withContext(Dispatchers.IO) { engine.invoke(cmd, args.toString()) }
        } catch (e: EngineException) {
            throw CommandException(e.text() ?: cmd)
        }
        return if (reply.isEmpty()) JsonNull else json.parseToJsonElement(reply)
    }

    /** Runs a command with arguments as JSON text (the editor's bridge). */
    suspend fun callRaw(cmd: String, args: String): String {
        val engine = engine()
        return try {
            withContext(Dispatchers.IO) { engine.invoke(cmd, args) }
        } catch (e: EngineException) {
            throw CommandException(e.text() ?: cmd)
        }
    }

    /** Adds a file as an attachment (pasted or dropped in the editor). */
    suspend fun saveResource(data: ByteArray, name: String, mime: String): String {
        val engine = engine()
        return try {
            withContext(Dispatchers.IO) { engine.saveResource(data, name, mime) }
        } catch (e: EngineException) {
            throw CommandException(e.text() ?: "save_resource_bytes")
        }
    }

    /** What only the app can do, for the engine. */
    private class Host(private val context: Context) : NativeHost {
        override fun emit(event: String, payload: String) {
            _events.tryEmit(EngineEvent(event, payload))
        }

        override fun openUrl(url: String): Boolean = try {
            context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            true
        } catch (e: ActivityNotFoundException) {
            false
        }

        override fun openPath(path: String): Boolean = try {
            val file = File(path)
            val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
            val mime = context.contentResolver.getType(uri) ?: "*/*"
            context.startActivity(
                Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_GRANT_READ_URI_PERMISSION),
            )
            true
        } catch (e: Exception) {
            if (e is kotlinx.coroutines.CancellationException) throw e
            Log.w("Nisshoku", "could not open $path: $e")
            false
        }

        override fun notify(title: String, body: String) {
            Notifications.reminder(context, title, body)
        }

        override fun openContent(uri: String, write: Boolean): Int = try {
            context.contentResolver.openFileDescriptor(Uri.parse(uri), if (write) "wt" else "r")
                ?.detachFd() ?: -1
        } catch (e: Exception) {
            if (e is kotlinx.coroutines.CancellationException) throw e
            Log.w("Nisshoku", "could not open $uri: $e")
            -1
        }
    }
}
