package app.nisshoku.android

import android.Manifest
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.fillMaxSize
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import app.nisshoku.android.ui.AppModel
import app.nisshoku.android.ui.EditorScreen
import app.nisshoku.android.ui.HomeScreen
import app.nisshoku.android.ui.SearchScreen
import app.nisshoku.android.ui.SettingsScreen
import app.nisshoku.android.ui.SyncSetupScreen
import app.nisshoku.android.ui.theme.NisshokuTheme
import app.nisshoku.android.ui.theme.isDark
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonPrimitive

class MainActivity : ComponentActivity() {
    private val model: AppModel by viewModels()

    /** A link to follow (`nisshoku://…`, or text shared to the app), until the screens take it. */
    private val pendingLink = MutableStateFlow<Uri?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        val splash = installSplashScreen()
        super.onCreate(savedInstanceState)
        splash.setKeepOnScreenCondition { !model.home.value.ready }
        pendingLink.value = linkOf(intent)

        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) = model.setForeground(true)
            override fun onStop(owner: LifecycleOwner) = model.setForeground(false)
        })

        setContent {
            val prefs by model.prefs.collectAsState()
            val theme = model.themeSettings(prefs)
            val english = (prefs["language"] as? JsonPrimitive)?.content?.let { it == "en" } ?: Lang.english
            val dark = isDark(theme)
            // Android 13+: the app's language for the system's texts too
            // (dates, "3 min. ago"), its name and shortcuts.
            LaunchedEffect(english) {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    val tag = if (english) "en" else "ja"
                    val manager = getSystemService(android.app.LocaleManager::class.java)
                    if (manager != null && manager.applicationLocales.toLanguageTags() != tag) {
                        manager.applicationLocales = android.os.LocaleList.forLanguageTags(tag)
                    }
                }
            }
            LaunchedEffect(dark) {
                val style = if (dark) SystemBarStyle.dark(android.graphics.Color.TRANSPARENT)
                else SystemBarStyle.light(android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT)
                enableEdgeToEdge(statusBarStyle = style, navigationBarStyle = style)
            }
            CompositionLocalProvider(LocalEnglish provides english) {
                NisshokuTheme(theme) {
                    Surface(color = MaterialTheme.colorScheme.background, modifier = Modifier.fillMaxSize()) {
                        App(dark)
                    }
                }
            }
        }
    }

    @androidx.compose.runtime.Composable
    private fun App(dark: Boolean) {
        val nav = rememberNavController()
        val snackbar = remember { SnackbarHostState() }
        val scope = rememberCoroutineScope()

        LaunchedEffect(Unit) {
            model.messages.collect { m ->
                launch {
                    val result = snackbar.showSnackbar(m.text, actionLabel = m.action, duration = if (m.action != null) SnackbarDuration.Long else SnackbarDuration.Short)
                    if (result == SnackbarResult.ActionPerformed) m.run?.invoke()
                }
            }
        }

        // Notifications (the sync's progress, due tasks): asked for once, at the start.
        val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
        LaunchedEffect(Unit) {
            val prefs = getSharedPreferences("app", MODE_PRIVATE)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !Notifications.allowed(this@MainActivity) && !prefs.getBoolean("askedNotifications", false)) {
                prefs.edit().putBoolean("askedNotifications", true).apply()
                permission.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        }

        // Links from widgets, shortcuts, the tile, and text shared from other apps.
        val link by pendingLink.collectAsState()
        val ready by remember { model.home }.collectAsState()
        LaunchedEffect(link, ready.ready) {
            val uri = link ?: return@LaunchedEffect
            if (!ready.ready) return@LaunchedEffect
            pendingLink.value = null
            follow(uri, nav)
        }

        val open: (String) -> Unit = { id -> nav.navigate("note/$id") { launchSingleTop = true } }
        NavHost(
            navController = nav,
            startDestination = "home",
            enterTransition = { slideInHorizontally(tween(380)) { it / 3 } + fadeIn(tween(260)) },
            exitTransition = { slideOutHorizontally(tween(380)) { -it / 6 } + fadeOut(tween(200)) },
            popEnterTransition = { slideInHorizontally(tween(380)) { -it / 6 } + fadeIn(tween(260)) },
            popExitTransition = { scaleOut(tween(300), targetScale = 0.92f) + fadeOut(tween(260)) },
        ) {
            composable("home") {
                HomeScreen(
                    model = model,
                    snackbar = snackbar,
                    onOpen = open,
                    onSearch = { nav.navigate("search") },
                    onSettings = { nav.navigate("settings") },
                )
            }
            composable(
                "note/{id}",
                enterTransition = { scaleIn(tween(320), initialScale = 0.94f) + fadeIn(tween(240)) },
            ) { entry ->
                val id = entry.arguments?.getString("id").orEmpty()
                EditorScreen(noteId = id, dark = dark, onClose = {
                    model.editorClosed()
                    if (nav.currentBackStackEntry?.destination?.route == "note/{id}") nav.popBackStack()
                })
            }
            composable("search") {
                SearchScreen(onBack = { nav.popBackStack() }, onOpen = open)
            }
            composable("settings") {
                SettingsScreen(model = model, snackbar = snackbar, onBack = { nav.popBackStack() }, onSyncSetup = { nav.navigate("sync?kind=$it") })
            }
            composable("sync?kind={kind}") { entry ->
                SyncSetupScreen(model = model, kind = entry.arguments?.getString("kind").orEmpty(), onBack = { nav.popBackStack() })
            }
        }
    }

    private suspend fun follow(uri: Uri, nav: NavHostController) {
        val id = when (uri.host) {
            "diary" -> model.todaysDiary()
            "new-note" -> model.newNote()
            "new-task" -> {
                nav.navigate("home") { launchSingleTop = true }
                null
            }
            "note" -> uri.lastPathSegment
            "share" -> model.newNote(uri.getQueryParameter("title")?.ifBlank { null }, uri.getQueryParameter("text"))
            else -> null
        }
        if (id != null) nav.navigate("note/$id") { launchSingleTop = true }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        linkOf(intent)?.let { pendingLink.value = it }
    }

    /**
     * The link an intent asks to follow: a widget's, shortcut's or tile's as
     * is; text shared from another app, or picked in its text-selection menu,
     * as `nisshoku://share?title=…&text=…`.
     */
    private fun linkOf(intent: Intent?): Uri? {
        intent ?: return null
        val text = when (intent.action) {
            Intent.ACTION_SEND -> intent.getCharSequenceExtra(Intent.EXTRA_TEXT)
            Intent.ACTION_PROCESS_TEXT -> intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)
            else -> return intent.data?.takeIf { it.scheme == "nisshoku" }
        }?.toString()?.takeIf { it.isNotBlank() } ?: return null
        return Uri.Builder().scheme("nisshoku").authority("share")
            .appendQueryParameter("title", intent.getStringExtra(Intent.EXTRA_SUBJECT) ?: "")
            .appendQueryParameter("text", text)
            .build()
    }
}
