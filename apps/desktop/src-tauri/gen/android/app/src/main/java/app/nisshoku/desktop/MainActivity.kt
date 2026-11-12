package app.nisshoku.desktop

import android.content.Intent
import android.app.LocaleManager
import android.content.res.Configuration
import android.os.Build
import android.os.LocaleList
import android.net.Uri
import android.os.Bundle
import android.os.FileObserver
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.updateLayoutParams
import org.json.JSONObject
import java.lang.ref.WeakReference

class MainActivity : TauriActivity() {
  companion object {
    private var webView: WeakReference<WebView>? = null

    private fun dispatch(event: String, detail: String = "null") {
      val view = webView?.get() ?: return
      view.post {
        view.evaluateJavascript("window.dispatchEvent(new CustomEvent('$event', { detail: $detail }))", null)
      }
    }

    /** A widget did something (ticked a task off): the open app applies it at once. */
    fun onWidgetAction() = dispatch("nisshoku-widget-action")
  }

  /** The system bars' insets in CSS pixels, as JSON (read by the page at start). */
  @Volatile private var insets = "{\"top\":0,\"right\":0,\"bottom\":0,\"left\":0}"

  /** Where a widget asked to go (`nisshoku://…`), until the page takes it. */
  @Volatile private var pendingUri: String? = null

  /** Refreshes the widgets whenever the app rewrites their snapshot. */
  private var snapshotObserver: FileObserver? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    pendingUri = linkOf(intent)
    @Suppress("DEPRECATION")
    snapshotObserver = object : FileObserver(dataDir.path, MOVED_TO or CLOSE_WRITE) {
      override fun onEvent(event: Int, path: String?) {
        if (path == "widgets.json") Widgets.refreshAll(applicationContext)
      }
    }.apply { startWatching() }
  }

  // The widgets take the system's dark or light colours when the app follows it.
  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    Widgets.refreshAll(applicationContext)
  }

  override fun onDestroy() {
    snapshotObserver?.stopWatching()
    super.onDestroy()
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    val uri = linkOf(intent) ?: return
    if (webView?.get() != null) dispatch("nisshoku-intent", JSONObject.quote(uri)) else pendingUri = uri
  }

  /**
   * The `nisshoku://` link an intent asks the page to open: a widget's,
   * shortcut's or tile's link as is; text shared from another app or picked
   * in its text-selection menu as `nisshoku://share?title=…&text=…`.
   */
  private fun linkOf(intent: Intent?): String? {
    intent ?: return null
    val text = when (intent.action) {
      Intent.ACTION_SEND -> intent.getCharSequenceExtra(Intent.EXTRA_TEXT)
      Intent.ACTION_PROCESS_TEXT -> intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)
      else -> return intent.data?.takeIf { it.scheme == "nisshoku" }?.toString()
    }?.toString()?.takeIf { it.isNotBlank() } ?: return null
    val title = intent.getStringExtra(Intent.EXTRA_SUBJECT) ?: ""
    return Uri.Builder().scheme("nisshoku").authority("share")
      .appendQueryParameter("title", title)
      .appendQueryParameter("text", text)
      .build().toString()
  }

  // The page draws under the status and navigation bars (its background runs
  // behind them) and pads its content by their size: the web view's
  // env(safe-area-inset-*) only covers display cutouts. The keyboard instead
  // shrinks the web view, so that the text being typed scrolls above it.
  override fun onWebViewCreate(webView: WebView) {
    MainActivity.webView = WeakReference(webView)
    webView.addJavascriptInterface(object {
      @JavascriptInterface fun get(): String = insets
    }, "NisshokuInsets")
    webView.addJavascriptInterface(object {
      /** The link a widget opened the app with, once. */
      @JavascriptInterface fun takeLink(): String? = pendingUri.also { pendingUri = null }

      /**
       * The app's language (`ja` or `en`), chosen in its settings: Android 13+
       * shows the app's name, shortcuts and widgets in it too.
       */
      @JavascriptInterface fun setLanguage(language: String) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        if (language != "ja" && language != "en") return
        runOnUiThread {
          val manager = getSystemService(LocaleManager::class.java) ?: return@runOnUiThread
          if (manager.applicationLocales.toLanguageTags() != language) {
            manager.applicationLocales = LocaleList.forLanguageTags(language)
          }
        }
      }

      /** The language Android has for the app (Android 13+ per-app setting), or "". */
      @JavascriptInterface fun systemLanguage(): String {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return ""
        return getSystemService(LocaleManager::class.java)?.applicationLocales?.get(0)?.language ?: ""
      }

      /** A sync begins: keep it going off screen, with its progress in a notification (SyncService.kt). */
      @JavascriptInterface fun syncStarted(dir: String) {
        SyncService.start(this@MainActivity, dir)
      }

      /** One sync in the background as soon as possible (the app left with changes not sent). */
      @JavascriptInterface fun syncInBackground() {
        SyncWorker.runOnce(applicationContext)
      }

      /** Syncs in the background every `minutes` (0: never), the app closed too (SyncWorker.kt). */
      @JavascriptInterface fun setBackgroundSync(minutes: Int, dir: String) {
        SyncWorker.schedule(applicationContext, minutes, dir)
      }

      /** Light or dark icons on the system bars, for the app's own theme (which may differ from the system's). */
      @JavascriptInterface fun setDarkTheme(dark: Boolean) {
        runOnUiThread {
          WindowCompat.getInsetsController(window, window.decorView).apply {
            isAppearanceLightStatusBars = !dark
            isAppearanceLightNavigationBars = !dark
          }
        }
      }
    }, "NisshokuAndroid")
    ViewCompat.setOnApplyWindowInsetsListener(webView) { view, windowInsets ->
      val bars = windowInsets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      val ime = windowInsets.getInsets(WindowInsetsCompat.Type.ime()).bottom
      view.updateLayoutParams<ViewGroup.MarginLayoutParams> { bottomMargin = ime }
      val d = resources.displayMetrics.density
      val bottom = if (ime > 0) 0 else bars.bottom
      insets = "{\"top\":${bars.top / d},\"right\":${bars.right / d},\"bottom\":${bottom / d},\"left\":${bars.left / d}}"
      dispatch("nisshoku-insets", insets)
      windowInsets
    }
  }
}
