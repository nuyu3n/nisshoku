package app.nisshoku.android.ui

import android.Manifest
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.text.format.DateUtils
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.OpenInNew
import androidx.compose.material.icons.outlined.Cloud
import androidx.compose.material.icons.outlined.EditNote
import androidx.compose.material.icons.outlined.CloudOff
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material.icons.outlined.Key
import androidx.compose.material.icons.outlined.Language
import androidx.compose.material.icons.outlined.NewReleases
import androidx.compose.material.icons.outlined.Notifications
import androidx.compose.material.icons.outlined.Palette
import androidx.compose.material.icons.outlined.Schedule
import androidx.compose.material.icons.outlined.Sync
import androidx.compose.material.icons.outlined.SystemUpdate
import androidx.compose.material.icons.outlined.TextFields
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeFlexibleTopAppBar
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import app.nisshoku.android.BuildConfig
import app.nisshoku.android.Notifications
import app.nisshoku.android.engine.Api
import app.nisshoku.android.engine.Release
import app.nisshoku.android.tr
import app.nisshoku.android.trNow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonPrimitive
import kotlin.math.roundToInt

/** One setting in a group: its shape is the group's (see `groupShape`). */
@Composable
private fun SettingRow(
    title: String,
    index: Int,
    count: Int,
    icon: ImageVector? = null,
    subtitle: String? = null,
    onClick: (() -> Unit)? = null,
    trailing: @Composable (() -> Unit)? = null,
) {
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = subtitle?.let { { Text(it) } },
        leadingContent = icon?.let { { Icon(it, null) } },
        trailingContent = trailing,
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
        modifier = Modifier
            .clip(groupShape(index, count))
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier),
    )
}

/** A choice of a few, as connected toggle buttons (Material 3 Expressive). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Choice(options: List<Pair<String, String>>, selected: String, onSelect: (String) -> Unit, modifier: Modifier = Modifier) {
    Row(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween),
    ) {
        options.forEachIndexed { index, (value, label) ->
            ToggleButton(
                checked = value == selected,
                onCheckedChange = { onSelect(value) },
                shapes = when (index) {
                    0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                    options.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                    else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                },
                modifier = Modifier.weight(1f).semantics { role = Role.RadioButton },
            ) { Text(label, maxLines = 1) }
        }
    }
}

/** A group's card holding something other than rows (choices, sliders). */
@Composable
private fun Block(index: Int, count: Int, content: @Composable () -> Unit) {
    Surface(color = MaterialTheme.colorScheme.surfaceContainerLow, shape = groupShape(index, count), modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(horizontal = 16.dp, vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) { content() }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SettingsScreen(model: AppModel, snackbar: SnackbarHostState, onBack: () -> Unit, onSyncSetup: (String) -> Unit) {
    val prefs by model.prefs.collectAsState()
    val sync by model.sync.collectAsState()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val bar = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    var release by remember { mutableStateOf<Release?>(null) }
    var askPassword by remember { mutableStateOf(false) }
    var notificationsOn by remember { mutableStateOf(Notifications.allowed(context)) }
    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { notificationsOn = it }
    LaunchedEffect(Unit) { model.refreshSyncSettings() }

    fun str(key: String, default: String) = (prefs[key] as? JsonPrimitive)?.content ?: default
    fun bool(key: String, default: Boolean) = (prefs[key] as? JsonPrimitive)?.content?.toBooleanStrictOrNull() ?: default
    fun num(key: String, default: Double) = (prefs[key] as? JsonPrimitive)?.content?.toDoubleOrNull() ?: default

    Scaffold(
        modifier = Modifier.nestedScroll(bar.nestedScrollConnection),
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = {
            LargeFlexibleTopAppBar(
                title = { Text(tr("設定", "Settings")) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, tr("戻る", "Back")) } },
                scrollBehavior = bar,
            )
        },
    ) { padding ->
        LazyColumn(
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = padding.calculateTopPadding(), bottom = padding.calculateBottomPadding() + 32.dp),
            verticalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            // ---------------------------------------------------------- Sync
            item { SectionLabel(tr("同期", "Sync")) }
            item {
                val status = sync.status
                val configured = sync.target.kind != "none"
                val where = when (sync.target.kind) {
                    "nextcloud" -> "Nextcloud · ${sync.target.server.orEmpty().removePrefix("https://")}"
                    "googleDrive" -> "Google Drive · ${sync.target.folder.orEmpty()}"
                    "oneDrive" -> "OneDrive · ${sync.target.folder.orEmpty()}"
                    "local" -> sync.target.path.orEmpty()
                    else -> tr("設定されていません", "Not set up")
                }
                SettingRow(
                    title = tr("同期先", "Sync target"),
                    subtitle = where,
                    icon = if (configured) Icons.Outlined.Cloud else Icons.Outlined.CloudOff,
                    index = 0,
                    count = if (configured) 3 else 1,
                    onClick = { onSyncSetup(if (configured) sync.target.kind else "") },
                )
            }
            if (sync.target.kind != "none") {
                item {
                    val status = sync.status
                    val text = when (status.state) {
                        "syncing" -> tr("同期しています…", "Syncing…")
                        "error" -> status.message ?: tr("エラー", "Error")
                        else -> if (status.lastSync > 0) tr("最終同期 ", "Last synced ") +
                            DateUtils.getRelativeTimeSpanString(status.lastSync, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS) else tr("まだ同期していません", "Not synced yet")
                    }
                    SettingRow(
                        title = tr("今すぐ同期", "Sync now"),
                        subtitle = text,
                        icon = Icons.Outlined.Sync,
                        index = 1,
                        count = 3,
                        onClick = model::syncNow,
                        trailing = if (status.state == "syncing") ({ LoadingIndicator(Modifier.size(32.dp)) }) else null,
                    )
                }
                item {
                    val minutes = model.prefInt("backgroundSyncMinutes", 60)
                    Block(2, 3) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(Icons.Outlined.Schedule, null)
                            Spacer(Modifier.width(16.dp))
                            Column {
                                Text(tr("バックグラウンド同期", "Background sync"), style = MaterialTheme.typography.bodyLarge)
                                Text(tr("アプリを閉じていても同期します", "Syncs with the app closed too"), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                        Choice(
                            listOf("0" to tr("しない", "Off"), "15" to tr("15分", "15 min"), "60" to tr("1時間", "1 hour"), "360" to tr("6時間", "6 hours")),
                            minutes.toString(),
                            { model.setPref("backgroundSyncMinutes", JsonPrimitive(it.toInt())) },
                        )
                    }
                }
                if (sync.status.needsPassword) {
                    item {
                        Spacer(Modifier.height(8.dp))
                        SettingRow(
                            title = tr("暗号化のパスワードを入力", "Enter the encryption password"),
                            subtitle = tr("同期先は暗号化されています", "The sync target is encrypted"),
                            icon = Icons.Outlined.Key,
                            index = 0,
                            count = 1,
                            onClick = { askPassword = true },
                        )
                    }
                }
            }

            // ---------------------------------------------------- Appearance
            item { SectionLabel(tr("表示", "Appearance")) }
            item {
                Block(0, 3) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Outlined.Palette, null)
                        Spacer(Modifier.width(16.dp))
                        Text(tr("テーマ", "Theme"), style = MaterialTheme.typography.bodyLarge)
                    }
                    Choice(
                        listOf("system" to tr("自動", "System"), "light" to tr("ライト", "Light"), "dark" to tr("ダーク", "Dark")),
                        str("theme", "system"),
                        { model.setPref("theme", JsonPrimitive(it)) },
                    )
                }
            }
            item {
                SettingRow(
                    title = tr("壁紙の色を使う", "Colours from the wallpaper"),
                    subtitle = tr("Material You（Android 12 以降）", "Material You (Android 12+)"),
                    index = 1,
                    count = 3,
                    onClick = { model.setPref("materialYou", JsonPrimitive(!bool("materialYou", true))) },
                    trailing = {
                        Switch(
                            checked = bool("materialYou", true) && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S,
                            enabled = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S,
                            onCheckedChange = { model.setPref("materialYou", JsonPrimitive(it)) },
                        )
                    },
                )
            }
            item {
                Block(2, 3) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Outlined.Language, null)
                        Spacer(Modifier.width(16.dp))
                        Text(tr("言語", "Language"), style = MaterialTheme.typography.bodyLarge)
                    }
                    Choice(listOf("ja" to "日本語", "en" to "English"), str("language", if (model.prefString("language", "") == "en") "en" else "ja"), { model.setPref("language", JsonPrimitive(it)) })
                }
            }
            if (!(bool("materialYou", true) && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S)) {
                item {
                    Spacer(Modifier.height(3.dp))
                    Block(0, 1) {
                        Text(tr("アクセントカラー", "Accent"), style = MaterialTheme.typography.bodyLarge)
                        Choice(
                            listOf("orange" to tr("橙", "Orange"), "vermilion" to tr("朱", "Red"), "indigo" to tr("藍", "Indigo"), "green" to tr("緑", "Green"), "sumi" to tr("墨", "Ink")),
                            str("accent", "orange"),
                            { model.setPref("accent", JsonPrimitive(it)) },
                        )
                    }
                }
            }

            // -------------------------------------------------------- Editor
            item { SectionLabel(tr("エディタ", "Editor")) }
            item {
                Block(0, 1) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Outlined.EditNote, null)
                        Spacer(Modifier.width(16.dp))
                        Text(tr("ノートを開くエディタ", "Editor for notes"), style = MaterialTheme.typography.bodyLarge)
                    }
                    Choice(
                        listOf("auto" to tr("自動", "Auto"), "native" to tr("シンプル", "Simple"), "rich" to tr("リッチ", "Formatted")),
                        str("editorMode", "auto"),
                        { model.setPref("editorMode", JsonPrimitive(it)) },
                    )
                    Text(
                        when (str("editorMode", "auto")) {
                            "native" -> tr("いつも Android 標準の入力欄で書きます。日本語入力がいちばん安定します。数式や表は記号のまま表示されます。", "Always the native text field: the most reliable with the keyboard. Math and tables show as their source.")
                            "rich" -> tr("いつも整形表示のエディタで開きます（数式・表・画像をそのまま表示）。", "Always the formatted editor (math, tables and images as they look).")
                            else -> tr("文章だけのノートはシンプルエディタで、数式・表・画像のあるノートはリッチ表示で開きます。", "Notes of plain text open in the simple editor; those with math, tables or images in the formatted one.")
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            item { Spacer(Modifier.height(5.dp)) }
            item {
                Block(0, 3) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Outlined.TextFields, null)
                        Spacer(Modifier.width(16.dp))
                        Text(tr("書体", "Typeface"), style = MaterialTheme.typography.bodyLarge)
                    }
                    Choice(listOf("serif" to tr("明朝", "Serif"), "sans" to tr("ゴシック", "Sans")), str("editorFont", "serif"), { model.setPref("editorFont", JsonPrimitive(it)) })
                }
            }
            item {
                var size by remember(prefs) { mutableFloatStateOf(num("fontSize", 17.0).toFloat()) }
                Block(1, 3) {
                    Text(tr("文字の大きさ  ${size.roundToInt()}px", "Text size  ${size.roundToInt()}px"), style = MaterialTheme.typography.bodyLarge)
                    Slider(
                        value = size,
                        onValueChange = { size = it },
                        onValueChangeFinished = { model.setPref("fontSize", JsonPrimitive(size.roundToInt())) },
                        valueRange = 13f..26f,
                        steps = 12,
                    )
                }
            }
            item {
                SettingRow(
                    title = tr("行番号", "Line numbers"),
                    index = 2,
                    count = 3,
                    onClick = { model.setPref("lineNumbers", JsonPrimitive(!bool("lineNumbers", false))) },
                    trailing = { Switch(checked = bool("lineNumbers", false), onCheckedChange = { model.setPref("lineNumbers", JsonPrimitive(it)) }) },
                )
            }

            // ------------------------------------------------- Notifications
            item { SectionLabel(tr("通知", "Notifications")) }
            if (!notificationsOn && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                item {
                    SettingRow(
                        title = tr("通知を許可", "Allow notifications"),
                        subtitle = tr("同期の進み具合とタスクの期限を知らせます", "For the sync's progress and due tasks"),
                        icon = Icons.Outlined.Notifications,
                        index = 0,
                        count = 2,
                        onClick = { permission.launch(Manifest.permission.POST_NOTIFICATIONS) },
                    )
                }
            }
            item {
                val n = if (!notificationsOn && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) 2 else 1
                SettingRow(
                    title = tr("タスクの期限を通知", "Remind of due tasks"),
                    index = n - 1,
                    count = n,
                    onClick = { model.setPref("taskReminders", JsonPrimitive(!bool("taskReminders", true))) },
                    trailing = { Switch(checked = bool("taskReminders", true), onCheckedChange = { model.setPref("taskReminders", JsonPrimitive(it)) }) },
                )
            }

            // --------------------------------------------------------- About
            item { SectionLabel(tr("このアプリについて", "About")) }
            item {
                SettingRow(
                    title = tr("日色", "Nisshoku"),
                    subtitle = tr("バージョン ", "Version ") + BuildConfig.VERSION_NAME,
                    icon = Icons.Outlined.Info,
                    index = 0,
                    count = 4,
                )
            }
            item {
                var checking by remember { mutableStateOf(false) }
                SettingRow(
                    title = tr("アップデートを確認", "Check for updates"),
                    icon = Icons.Outlined.SystemUpdate,
                    index = 1,
                    count = 4,
                    onClick = {
                        if (!checking) scope.launch {
                            checking = true
                            model.checkForUpdate(quiet = false)
                            checking = false
                        }
                    },
                    trailing = if (checking) ({ LoadingIndicator(Modifier.size(32.dp)) }) else null,
                )
            }
            item {
                SettingRow(
                    title = tr("起動時にアップデートを確認", "Check for updates at start"),
                    index = 2,
                    count = 4,
                    onClick = { model.setPref("autoUpdateCheck", JsonPrimitive(!bool("autoUpdateCheck", true))) },
                    trailing = { Switch(checked = bool("autoUpdateCheck", true), onCheckedChange = { model.setPref("autoUpdateCheck", JsonPrimitive(it)) }) },
                )
            }
            item {
                SettingRow(
                    title = tr("新しくなったこと", "What's new"),
                    icon = Icons.Outlined.NewReleases,
                    index = 3,
                    count = 4,
                    onClick = model::showAllReleases,
                )
            }
        }
    }

    if (askPassword) {
        PasswordPrompt(
            onDismiss = { askPassword = false },
            onDone = { password ->
                askPassword = false
                scope.launch {
                    runCatching { Api.setMasterPassword(password) }
                        .onFailure { model.say(it.message ?: "") }
                    model.refreshSyncSettings()
                }
            },
        )
    }
}

@Composable
private fun PasswordPrompt(onDismiss: () -> Unit, onDone: (String) -> Unit) {
    var text by remember { mutableStateOf("") }
    androidx.compose.material3.AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("暗号化のパスワード", "Encryption password")) },
        text = {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = { Button(onClick = { onDone(text) }, enabled = text.isNotEmpty()) { Text(tr("OK", "OK")) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("キャンセル", "Cancel")) } },
    )
}

/** Choosing and signing in to a sync target. `kind` is the current one (or ""). */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SyncSetupScreen(model: AppModel, kind: String, onBack: () -> Unit) {
    val sync by model.sync.collectAsState()
    val scope = rememberCoroutineScope()
    var choice by remember { mutableStateOf(kind.ifEmpty { "nextcloud" }) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var server by remember { mutableStateOf(sync.target.server ?: "https://") }
    var user by remember { mutableStateOf(sync.target.user ?: "") }
    var password by remember { mutableStateOf("") }
    var folder by remember { mutableStateOf(sync.target.folder ?: "Nisshoku") }
    var clientId by remember { mutableStateOf(sync.target.clientId ?: "") }
    var clientSecret by remember { mutableStateOf("") }
    var manual by remember { mutableStateOf(false) }

    fun run(block: suspend () -> Unit) {
        busy = true
        error = null
        scope.launch {
            try {
                block()
                model.refreshSyncSettings()
                model.syncNow()
                onBack()
            } catch (e: Exception) {
                error = e.message
            } finally {
                busy = false
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(tr("同期先", "Sync target")) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, tr("戻る", "Back")) } },
            )
        },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .animateContentSize(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Choice(
                listOf("nextcloud" to "Nextcloud", "googleDrive" to "Google Drive", "oneDrive" to "OneDrive"),
                choice,
                { choice = it; error = null },
            )
            when (choice) {
                "nextcloud" -> {
                    OutlinedTextField(server, { server = it }, label = { Text(tr("サーバーの URL", "Server URL")) }, singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri), modifier = Modifier.fillMaxWidth())
                    OutlinedTextField(folder, { folder = it }, label = { Text(tr("フォルダ", "Folder")) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    AnimatedVisibility(manual) {
                        Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                            OutlinedTextField(user, { user = it }, label = { Text(tr("ユーザー名", "User name")) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                            OutlinedTextField(password, { password = it }, label = { Text(tr("アプリパスワード", "App password")) }, singleLine = true,
                                visualTransformation = PasswordVisualTransformation(),
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password), modifier = Modifier.fillMaxWidth())
                        }
                    }
                    if (manual) {
                        Button(onClick = { run { Api.nextcloudConnect(server.trim(), user.trim(), password, folder) } }, enabled = !busy && user.isNotBlank() && password.isNotEmpty(), modifier = Modifier.fillMaxWidth().height(56.dp)) {
                            Text(tr("接続", "Connect"))
                        }
                    } else {
                        Button(onClick = {
                            run {
                                Api.nextcloudLoginStart(server.trim())
                                Api.nextcloudLoginWait(folder)
                            }
                        }, enabled = !busy && server.length > 8, modifier = Modifier.fillMaxWidth().height(56.dp)) {
                            Text(tr("ブラウザでログイン", "Sign in in the browser"))
                        }
                    }
                    TextButton(onClick = { manual = !manual }) {
                        Text(if (manual) tr("ブラウザでログインする", "Sign in in the browser instead") else tr("アプリパスワードを入力する", "Enter an app password instead"))
                    }
                }
                "googleDrive" -> {
                    OutlinedTextField(folder, { folder = it }, label = { Text(tr("マイドライブのフォルダ", "Folder in My Drive")) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    if (!sync.gdriveAvailable || manual) {
                        OutlinedTextField(clientId, { clientId = it }, label = { Text(tr("クライアント ID", "Client ID")) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                        OutlinedTextField(clientSecret, { clientSecret = it }, label = { Text(tr("クライアント シークレット", "Client secret")) }, singleLine = true,
                            visualTransformation = PasswordVisualTransformation(), modifier = Modifier.fillMaxWidth())
                    }
                    Button(onClick = {
                        run { Api.gdriveLogin(folder, clientId.takeIf { it.isNotBlank() }, clientSecret.takeIf { it.isNotBlank() }) }
                    }, enabled = !busy, modifier = Modifier.fillMaxWidth().height(56.dp)) {
                        Text(tr("Google でログイン", "Sign in with Google"))
                    }
                    if (sync.gdriveAvailable) {
                        TextButton(onClick = { manual = !manual }) { Text(tr("自分のクライアントを使う", "Use my own client")) }
                    }
                }
                "oneDrive" -> {
                    OutlinedTextField(clientId, { clientId = it }, label = { Text(tr("アプリケーション (クライアント) ID", "Application (client) ID")) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    OutlinedTextField(folder, { folder = it }, label = { Text(tr("フォルダ", "Folder")) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    Button(onClick = { run { Api.onedriveLogin(clientId.trim(), folder) } }, enabled = !busy && clientId.isNotBlank(), modifier = Modifier.fillMaxWidth().height(56.dp)) {
                        Text(tr("Microsoft でログイン", "Sign in with Microsoft"))
                    }
                }
            }
            AnimatedVisibility(busy) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    LoadingIndicator(Modifier.size(40.dp))
                    Spacer(Modifier.width(12.dp))
                    Text(tr("ブラウザでのログインを待っています…", "Waiting for the sign-in in the browser…"), color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            error?.let {
                Surface(color = MaterialTheme.colorScheme.errorContainer, shape = RoundedCornerShape(16.dp)) {
                    Text(it, color = MaterialTheme.colorScheme.onErrorContainer, modifier = Modifier.padding(16.dp))
                }
            }
            if (sync.target.kind != "none") {
                Spacer(Modifier.height(12.dp))
                FilledTonalButton(onClick = {
                    scope.launch {
                        runCatching { Api.disconnectSync() }.onFailure { error = it.message }
                        model.refreshSyncSettings()
                        model.say(trNow("同期を解除しました", "Sync turned off"))
                        onBack()
                    }
                }, modifier = Modifier.fillMaxWidth()) { Text(tr("同期を解除", "Turn sync off")) }
            }
            Spacer(Modifier.height(32.dp))
        }
    }
}
