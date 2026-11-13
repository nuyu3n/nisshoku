package app.nisshoku.android.ui

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.util.Base64
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.MimeTypeMap
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.statusBars
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.viewinterop.AndroidView
import androidx.webkit.WebViewAssetLoader
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import app.nisshoku.android.BuildConfig
import app.nisshoku.android.engine.CommandException
import app.nisshoku.android.engine.Nisshoku
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonPrimitive
import org.json.JSONObject
import java.io.File

private const val ORIGIN = "https://appassets.androidplatform.net"

/**
 * A note in the editor: the web app's editor alone (`?embed=editor`), in a
 * web view whose calls go to the engine (assets/bridge.js). The rest of the
 * app is native; back from the note returns to it.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun EditorScreen(noteId: String, dark: Boolean, onClose: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val close by rememberUpdatedState(onClose)
    var loaded by remember { mutableStateOf(false) }
    var webView by remember { mutableStateOf<WebView?>(null) }

    // The system's file pickers, for the editor's "insert image / file" and exports.
    val pending = remember { mutableStateOf<Pair<Int, Boolean>?>(null) }
    val open = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        val (id, _) = pending.value ?: return@rememberLauncherForActivityResult
        pending.value = null
        webView?.reply(id, true, uri?.toString()?.let { JSONObject.quote(it) } ?: "null")
    }
    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("*/*")) { uri ->
        val (id, _) = pending.value ?: return@rememberLauncherForActivityResult
        pending.value = null
        webView?.reply(id, true, uri?.toString()?.let { JSONObject.quote(it) } ?: "null")
    }

    val bridge = remember {
        Bridge(
            context = context.applicationContext,
            scope = scope,
            view = { webView },
            onClose = { close() },
            onPick = { id, mimes ->
                pending.value = id to false
                open.launch(mimes.ifEmpty { arrayOf("*/*") })
            },
            onSave = { id, name ->
                pending.value = id to true
                save.launch(name)
            },
        )
    }

    // The engine's events (a sync changed the note…) for the page.
    LaunchedEffect(webView) {
        val view = webView ?: return@LaunchedEffect
        Nisshoku.events.collect { e ->
            view.evaluateJavascript("window.__nisshokuEmit && window.__nisshokuEmit(${JSONObject.quote(e.name)}, ${e.payload.ifEmpty { "null" }})", null)
        }
    }

    // The page pads itself by the system bars (as in the Tauri build); the
    // keyboard shrinks the view instead (imePadding), so the text being
    // typed stays above it.
    val density = LocalDensity.current
    val top = WindowInsets.statusBars.getTop(density) / density.density
    val imeOpen = WindowInsets.ime.getBottom(density) > 0
    val bottom = if (imeOpen) 0f else WindowInsets.navigationBars.getBottom(density) / density.density
    val insets = """{"top":$top,"right":0,"bottom":$bottom,"left":0}"""
    bridge.insets = insets
    LaunchedEffect(insets, webView) {
        webView?.evaluateJavascript("window.dispatchEvent(new CustomEvent('nisshoku-insets', { detail: $insets }))", null)
    }

    BackHandler {
        val view = webView
        if (view == null || !loaded) {
            close()
            return@BackHandler
        }
        view.evaluateJavascript("(window.__nisshokuBack && window.__nisshokuBack()) || false") { taken ->
            if (taken != "true") {
                view.evaluateJavascript("window.__nisshokuFlush ? window.__nisshokuFlush().then(() => 1) : 1") { close() }
            }
        }
    }

    val background = if (dark) Color.rgb(0x26, 0x25, 0x22) else Color.rgb(0xFA, 0xF9, 0xF5)
    Box(
        Modifier
            .fillMaxSize()
            .background(androidx.compose.ui.graphics.Color(background))
            .imePadding(),
    ) {
        AndroidView(
            factory = { ctx -> createWebView(ctx, bridge, background, noteId) { loaded = true }.also { webView = it } },
            modifier = Modifier.fillMaxSize(),
        )
        AnimatedVisibility(!loaded, exit = fadeOut(), modifier = Modifier.align(Alignment.Center)) {
            LoadingIndicator()
        }
    }

    DisposableEffect(Unit) {
        onDispose {
            webView?.let { view ->
                // What was typed last is saved before the page goes.
                view.evaluateJavascript("window.__nisshokuFlush && window.__nisshokuFlush()", null)
                view.postDelayed({ view.destroy() }, 1500)
            }
        }
    }
}

private fun WebView.reply(id: Int, ok: Boolean, json: String) {
    post { evaluateJavascript("window.__nisshokuReply($id, $ok, ${JSONObject.quote(json)})", null) }
}

@SuppressLint("SetJavaScriptEnabled")
private fun createWebView(context: Context, bridge: Bridge, background: Int, noteId: String, onLoaded: () -> Unit): WebView {
    val profile = Nisshoku.profileDir(context)
    val loader = WebViewAssetLoader.Builder()
        .addPathHandler("/profile/", ProfileHandler(profile))
        .addPathHandler("/", WebHandler(context))
        .build()
    return WebView(context).apply {
        // Compose would wrap the content's height: the page's viewport would
        // then be 0 high (100vh = 0, nothing shows).
        layoutParams = android.view.ViewGroup.LayoutParams(
            android.view.ViewGroup.LayoutParams.MATCH_PARENT,
            android.view.ViewGroup.LayoutParams.MATCH_PARENT,
        )
        setBackgroundColor(background)
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.allowFileAccess = false
        settings.allowContentAccess = false
        settings.textZoom = 100
        settings.setSupportZoom(false)
        settings.mediaPlaybackRequiresUserGesture = false
        isVerticalScrollBarEnabled = false
        overScrollMode = WebView.OVER_SCROLL_NEVER
        addJavascriptInterface(bridge, "NisshokuBridge")
        addJavascriptInterface(bridge.native, "NisshokuNative")
        addJavascriptInterface(bridge.insetsSource, "NisshokuInsets")
        val script = context.assets.open("bridge.js").bufferedReader().use { it.readText() }
            .replace("__PROFILE__", profile.path)
        val early = WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)
        if (early) WebViewCompat.addDocumentStartJavaScript(this, script, setOf(ORIGIN))
        webViewClient = object : WebViewClient() {
            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
                loader.shouldInterceptRequest(request.url)

            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val url = request.url
                if (url.host == Uri.parse(ORIGIN).host) return false
                runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, url).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
                return true
            }

            override fun onPageStarted(view: WebView, url: String?, favicon: android.graphics.Bitmap?) {
                if (!early) view.evaluateJavascript(script, null)
            }

            override fun onPageFinished(view: WebView, url: String?) {
                // The page shows after its first paint; a moment for it.
                view.postDelayed(onLoaded, 120)
            }
        }
        if (BuildConfig.DEBUG) WebView.setWebContentsDebuggingEnabled(true)
        // Loaded once the view has its size.
        val url = "$ORIGIN/index.html?embed=editor&id=${Uri.encode(noteId)}"
        if (height > 0) loadUrl(url)
        else addOnLayoutChangeListener(object : android.view.View.OnLayoutChangeListener {
            override fun onLayoutChange(v: android.view.View, l: Int, t: Int, r: Int, b: Int, ol: Int, ot: Int, or: Int, ob: Int) {
                if (b - t <= 0) return
                removeOnLayoutChangeListener(this)
                post { loadUrl(url) }
            }
        })
    }
}

/** The web app's files (assets/web, built from apps/desktop). */
private class WebHandler(private val context: Context) : WebViewAssetLoader.PathHandler {
    override fun handle(path: String): WebResourceResponse? {
        val name = path.trimStart('/').ifEmpty { "index.html" }
        return try {
            val stream = context.assets.open("web/$name")
            WebResourceResponse(mime(name), if (mime(name).startsWith("text/") || name.endsWith(".js")) "utf-8" else null, stream)
        } catch (e: Exception) {
            null
        }
    }
}

/** The profile's attachments (`convertFileSrc` in bridge.js). */
private class ProfileHandler(private val root: File) : WebViewAssetLoader.PathHandler {
    override fun handle(path: String): WebResourceResponse? {
        val file = File(root, path).canonicalFile
        val resources = File(root, "resources").canonicalFile
        if (!file.path.startsWith(resources.path) || !file.isFile) return null
        return WebResourceResponse(mime(file.name), null, file.inputStream())
    }
}

private fun mime(name: String): String {
    val ext = name.substringAfterLast('.', "").lowercase()
    return when (ext) {
        "js", "mjs" -> "text/javascript"
        "css" -> "text/css"
        "html" -> "text/html"
        "svg" -> "image/svg+xml"
        "woff2" -> "font/woff2"
        "woff" -> "font/woff"
        "ttf" -> "font/ttf"
        "json" -> "application/json"
        else -> MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext) ?: "application/octet-stream"
    }
}

/** The page's calls (bridge.js), on the web view's binder thread. */
private class Bridge(
    private val context: Context,
    private val scope: CoroutineScope,
    private val view: () -> WebView?,
    private val onClose: () -> Unit,
    private val onPick: (Int, Array<String>) -> Unit,
    private val onSave: (Int, String) -> Unit,
) {
    @Volatile var insets: String = """{"top":0,"right":0,"bottom":0,"left":0}"""

    private fun reply(id: Int, ok: Boolean, json: String) {
        view()?.reply(id, ok, json)
    }

    @JavascriptInterface
    fun invoke(id: Int, cmd: String, args: String) {
        scope.launch(Dispatchers.IO) {
            try {
                reply(id, true, Nisshoku.callRaw(cmd, args))
            } catch (e: CommandException) {
                reply(id, false, JSONObject.quote(e.message ?: cmd))
            } catch (e: Exception) {
                Log.w("Editor", "$cmd failed: $e")
                reply(id, false, JSONObject.quote(e.toString()))
            }
        }
    }

    @JavascriptInterface
    fun invokeRaw(id: Int, data: String, name: String, mime: String) {
        scope.launch(Dispatchers.IO) {
            try {
                reply(id, true, Nisshoku.saveResource(Base64.decode(data, Base64.DEFAULT), name, mime))
            } catch (e: Exception) {
                reply(id, false, JSONObject.quote(e.message ?: "save_resource_bytes"))
            }
        }
    }

    @JavascriptInterface
    fun appVersion(): String = BuildConfig.VERSION_NAME.removeSuffix("-dev")

    @JavascriptInterface
    fun openUrl(url: String) {
        runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
    }

    @JavascriptInterface
    fun writeClipboard(text: String) {
        context.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(ClipData.newPlainText("日色", text))
    }

    @JavascriptInterface
    fun readClipboard(): String =
        context.getSystemService(ClipboardManager::class.java)?.primaryClip?.takeIf { it.itemCount > 0 }
            ?.getItemAt(0)?.coerceToText(context)?.toString() ?: ""

    @JavascriptInterface
    fun pickFile(id: Int, extensions: String, directory: Boolean) {
        val exts = runCatching { Json.parseToJsonElement(extensions) as kotlinx.serialization.json.JsonArray }.getOrNull()
            ?.mapNotNull { (it as? JsonPrimitive)?.content } ?: emptyList()
        val mimes = exts.mapNotNull { MimeTypeMap.getSingleton().getMimeTypeFromExtension(it.lowercase()) }.distinct()
        view()?.post { onPick(id, mimes.toTypedArray()) }
    }

    @JavascriptInterface
    fun saveFile(id: Int, name: String) {
        view()?.post { onSave(id, name.substringAfterLast('/')) }
    }

    /** `window.NisshokuNative` of components/Embedded.tsx. */
    val native = object {
        @JavascriptInterface
        fun close() {
            view()?.post { onClose() }
        }

        @JavascriptInterface
        fun opened(id: String) {}
    }

    /** `window.NisshokuInsets` of lib/mobile.ts. */
    val insetsSource = object {
        @JavascriptInterface
        fun get(): String = insets
    }
}
