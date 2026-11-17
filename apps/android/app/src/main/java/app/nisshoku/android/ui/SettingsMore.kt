package app.nisshoku.android.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.Sort
import androidx.compose.material.icons.outlined.AutoStories
import androidx.compose.material.icons.outlined.BarChart
import androidx.compose.material.icons.outlined.CleaningServices
import androidx.compose.material.icons.outlined.DeleteSweep
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.FormatLineSpacing
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.LockOpen
import androidx.compose.material.icons.outlined.PlayArrow
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.Flag
import androidx.compose.material.icons.outlined.CalendarMonth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeFlexibleTopAppBar
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import app.nisshoku.android.engine.Api
import app.nisshoku.android.engine.StorageInfo
import app.nisshoku.android.engine.SyncSettings
import app.nisshoku.android.tr
import app.nisshoku.android.trNow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.longOrNull
import kotlin.math.roundToInt

/** What a settings dialog asks for (one open at a time). */
sealed interface SettingAsk {
    data class Text(val key: String, val title: String, val initial: String, val number: Boolean = false) : SettingAsk
    data class Long(val key: String, val title: String, val initial: String) : SettingAsk
    data object Encryption : SettingAsk
}

private fun JsonObject.str(key: String, default: String) = (this[key] as? JsonPrimitive)?.content ?: default
private fun JsonObject.bool(key: String, default: Boolean) = (this[key] as? JsonPrimitive)?.content?.toBooleanStrictOrNull() ?: default
private fun JsonObject.int(key: String, default: Int) = (this[key] as? JsonPrimitive)?.intOrNull ?: default
private fun JsonObject.num(key: String, default: Double) = (this[key] as? JsonPrimitive)?.content?.toDoubleOrNull() ?: default

@Composable
private fun Heading(icon: androidx.compose.ui.graphics.vector.ImageVector, text: String, hint: String? = null) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, null)
        Spacer(Modifier.padding(start = 16.dp))
        Column {
            Text(text, style = MaterialTheme.typography.bodyLarge)
            hint?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        }
    }
}

/** A setting that is on or off. */
@Composable
fun SwitchSetting(model: AppModel, prefs: JsonObject, key: String, default: Boolean, title: String, subtitle: String?, index: Int, count: Int) {
    val on = prefs.bool(key, default)
    SettingRow(
        title = title,
        subtitle = subtitle,
        index = index,
        count = count,
        onClick = { model.setPref(key, JsonPrimitive(!on)) },
        trailing = { Switch(checked = on, onCheckedChange = { model.setPref(key, JsonPrimitive(it)) }) },
    )
}

/** The encryption of the sync target (end-to-end, Joplin's). */
fun LazyListScope.encryptionSetting(sync: SyncSettings, onAsk: (SettingAsk) -> Unit) {
    if (sync.target.kind == "none" || sync.status.needsPassword) return
    item {
        Spacer(Modifier.height(8.dp))
        SettingRow(
            title = tr("エンドツーエンド暗号化", "End-to-end encryption"),
            subtitle = if (sync.hasMasterPassword) tr("有効です。同期先のデータはパスワードで暗号化されています", "On: what is synced is encrypted with your password")
            else tr("同期先に置くデータをパスワードで暗号化します", "Encrypts what is stored at the sync target with a password"),
            icon = if (sync.hasMasterPassword) Icons.Outlined.Lock else Icons.Outlined.LockOpen,
            index = 0,
            count = 1,
            onClick = { onAsk(SettingAsk.Encryption) },
        )
    }
}

/** General, writing, list, diary and data settings. */
fun LazyListScope.moreSettings(model: AppModel, prefs: JsonObject, onAsk: (SettingAsk) -> Unit, onStats: () -> Unit) {
    // ------------------------------------------------------------- General
    item { SectionLabel(tr("一般", "General")) }
    item {
        Block(0, 2) {
            Heading(Icons.Outlined.PlayArrow, tr("起動時に開くもの", "On start, open"))
            Choice(
                listOf("last" to tr("前のノート", "Last note"), "diary" to tr("今日の日記", "Diary"), "list" to tr("一覧", "List")),
                prefs.str("startup", "last").let { if (it == "none") "list" else it },
                { model.setPref("startup", JsonPrimitive(it)) },
            )
        }
    }
    item {
        val name = prefs.str("defaultNotebookName", "")
        SettingRow(
            title = tr("標準のノートブック名", "Default notebook name"),
            subtitle = name.ifEmpty { tr("（自動）", "(automatic)") },
            icon = Icons.Outlined.Folder,
            index = 1,
            count = 2,
            onClick = { onAsk(SettingAsk.Text("defaultNotebookName", trNow("標準のノートブック名", "Default notebook name"), name)) },
        )
    }

    // ------------------------------------------------------------- Writing
    item { SectionLabel(tr("執筆", "Writing")) }
    item {
        var height by remember(prefs) { mutableFloatStateOf(prefs.num("lineHeight", 1.95).toFloat()) }
        Block(0, 5) {
            Heading(Icons.Outlined.FormatLineSpacing, tr("行の高さ  ${"%.2f".format(height)}倍", "Line height  ${"%.2f".format(height)}×"))
            Slider(
                value = height,
                onValueChange = { height = (it * 20).roundToInt() / 20f },
                onValueChangeFinished = { model.setPref("lineHeight", JsonPrimitive(height.toDouble())) },
                valueRange = 1.4f..2.6f,
            )
        }
    }
    item {
        val goal = prefs.int("dailyGoal", 0)
        SettingRow(
            title = tr("1日の目標", "Daily goal"),
            subtitle = if (goal > 0) tr("${"%,d".format(goal)}字", "${"%,d".format(goal)} chars") else tr("なし", "None"),
            icon = Icons.Outlined.Flag,
            index = 1,
            count = 5,
            onClick = { onAsk(SettingAsk.Text("dailyGoal", trNow("1日の目標（字）", "Daily goal (chars)"), if (goal > 0) goal.toString() else "", number = true)) },
        )
    }
    item {
        SwitchSetting(model, prefs, "smartPunctuation", true, tr("記号を自動で変換する", "Smart punctuation"), tr("「...」を「…」に、「--」を「――」に", "\"...\" to \"…\", \"--\" to \"――\""), 2, 5)
    }
    item {
        SwitchSetting(model, prefs, "autoPair", true, tr("括弧を自動で閉じる（リッチ表示）", "Close brackets (formatted view)"), null, 3, 5)
    }
    item {
        SwitchSetting(model, prefs, "countSpaces", false, tr("空白も文字数に数える", "Count spaces as characters"), null, 4, 5)
    }

    // ------------------------------------------------------------ The list
    item { SectionLabel(tr("ノート一覧", "Note list")) }
    item {
        Block(0, 3) {
            Heading(Icons.AutoMirrored.Outlined.Sort, tr("並べ順", "Sort by"))
            Choice(
                listOf("updated" to tr("更新日", "Edited"), "created" to tr("作成日", "Created"), "title" to tr("タイトル", "Title"), "custom" to tr("手動", "Manual")),
                prefs.str("sortNotes", "updated"),
                { model.setPref("sortNotes", JsonPrimitive(it)) },
            )
        }
    }
    item { SwitchSetting(model, prefs, "showExcerpt", true, tr("本文の冒頭を表示する", "Show the start of the text"), null, 1, 3) }
    item { SwitchSetting(model, prefs, "compactList", false, tr("コンパクト表示", "Compact list"), tr("1件を短くして、多くのノートを一度に見られるようにします", "Shorter rows, so that more notes fit"), 2, 3) }

    // --------------------------------------------------------------- Diary
    item { SectionLabel(tr("日記", "Diary")) }
    item {
        val name = prefs.str("diaryNotebookName", "")
        SettingRow(
            title = tr("日記帳の名前", "Diary notebook name"),
            subtitle = name.ifEmpty { tr("日記", "Diary") },
            icon = Icons.Outlined.AutoStories,
            index = 0,
            count = 4,
            onClick = { onAsk(SettingAsk.Text("diaryNotebookName", trNow("日記帳の名前", "Diary notebook name"), name)) },
        )
    }
    item {
        Block(1, 4) {
            Text(tr("タイトルの形式", "Entry title"), style = MaterialTheme.typography.bodyLarge)
            Choice(
                listOf("iso" to "2026-11-04", "isoWeekday" to tr("曜日つき", "With day"), "ja" to tr("11月4日", "Japanese")),
                prefs.str("diaryTitleFormat", "iso"),
                { model.setPref("diaryTitleFormat", JsonPrimitive(it)) },
            )
        }
    }
    item {
        Block(2, 4) {
            Heading(Icons.Outlined.CalendarMonth, tr("週の始まり", "Week starts on"))
            Choice(listOf("sun" to tr("日曜日", "Sunday"), "mon" to tr("月曜日", "Monday")), prefs.str("weekStart", "sun"), { model.setPref("weekStart", JsonPrimitive(it)) })
        }
    }
    item {
        val template = prefs.str("diaryTemplate", "")
        SettingRow(
            title = tr("テンプレート", "Template"),
            subtitle = template.lineSequence().firstOrNull { it.isNotBlank() }?.take(40) ?: tr("なし", "None"),
            icon = Icons.Outlined.Description,
            index = 3,
            count = 4,
            onClick = { onAsk(SettingAsk.Long("diaryTemplate", trNow("日記のテンプレート", "Diary template"), template)) },
        )
    }

    // ---------------------------------------------------------------- Data
    item { SectionLabel(tr("データ", "Data")) }
    item {
        SettingRow(title = tr("統計", "Statistics"), subtitle = tr("書いた文字数と続けた日数", "What you wrote, and your streak"), icon = Icons.Outlined.BarChart, index = 0, count = 4, onClick = onStats)
    }
    item {
        Block(1, 4) {
            Heading(Icons.Outlined.History, tr("ノートの履歴を残す期間", "Keep note history"))
            Choice(
                listOf("30" to tr("30日", "30 d"), "90" to tr("90日", "90 d"), "365" to tr("1年", "1 year"), "0" to tr("ずっと", "Always")),
                prefs.int("historyDays", 90).toString(),
                { model.setPref("historyDays", JsonPrimitive(it.toInt())) },
            )
        }
    }
    item {
        Block(2, 4) {
            Heading(Icons.Outlined.DeleteSweep, tr("ゴミ箱を自動で空にする", "Empty the trash automatically"))
            Choice(
                listOf("0" to tr("しない", "Never"), "7" to tr("7日後", "7 d"), "30" to tr("30日後", "30 d"), "90" to tr("90日後", "90 d")),
                prefs.int("trashRetentionDays", 30).toString(),
                { model.setPref("trashRetentionDays", JsonPrimitive(it.toInt())) },
            )
        }
    }
    item { StorageRow(model) }
}

/** The space the notes take, and the cleanup of attachments no note uses. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StorageRow(model: AppModel) {
    var info by remember { mutableStateOf<StorageInfo?>(null) }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) { info = runCatching { Api.storageInfo() }.getOrNull() }
    val i = info
    SettingRow(
        title = tr("ストレージ", "Storage"),
        subtitle = i?.let {
            val total = formatBytes(it.databaseBytes + it.resourceBytes)
            if (it.unusedCount > 0) tr("$total・使われていない添付 ${it.unusedCount} 件（${formatBytes(it.unusedBytes)}）をタップで削除", "$total · tap to delete ${it.unusedCount} unused attachments (${formatBytes(it.unusedBytes)})")
            else tr("$total（添付 ${it.resourceCount} 件）", "$total (${it.resourceCount} attachments)")
        } ?: " ",
        icon = Icons.Outlined.CleaningServices,
        index = 3,
        count = 4,
        onClick = {
            if (busy || (i?.unusedCount ?: 0) == 0) return@SettingRow
            scope.launch {
                busy = true
                val n = runCatching { Api.deleteUnusedResources() }.getOrDefault(0)
                model.say(trNow("$n 件の添付ファイルを削除しました", "Deleted $n attachments"))
                info = runCatching { Api.storageInfo() }.getOrNull()
                busy = false
            }
        },
        trailing = if (busy) ({ LoadingIndicator(Modifier.heightIn(max = 32.dp)) }) else null,
    )
}

fun formatBytes(n: Long): String = when {
    n >= 1L shl 30 -> "%.1f GB".format(n / (1L shl 30).toDouble())
    n >= 1L shl 20 -> "%.1f MB".format(n / (1L shl 20).toDouble())
    n >= 1L shl 10 -> "%.0f KB".format(n / (1L shl 10).toDouble())
    else -> "$n B"
}

/** The dialog of a settings row (a name, a number, the diary's template, the encryption). */
@Composable
fun SettingDialog(ask: SettingAsk, model: AppModel, sync: SyncSettings, onDismiss: () -> Unit) {
    val scope = rememberCoroutineScope()
    when (ask) {
        is SettingAsk.Text -> TextPrompt(
            title = ask.title,
            label = ask.title,
            confirm = tr("保存", "Save"),
            initial = ask.initial,
            onDismiss = onDismiss,
            onDone = { v ->
                if (ask.number) model.setPref(ask.key, JsonPrimitive(v.filter { it.isDigit() }.toIntOrNull() ?: 0))
                else model.setPref(ask.key, JsonPrimitive(v))
                onDismiss()
            },
        )
        is SettingAsk.Long -> {
            var text by remember { mutableStateOf(ask.initial) }
            AlertDialog(
                onDismissRequest = onDismiss,
                title = { Text(ask.title) },
                text = {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(tr("新しい日記に最初から入れておく文章です（Markdown）。", "Text that new entries start with (Markdown)."), style = MaterialTheme.typography.bodySmall)
                        OutlinedTextField(value = text, onValueChange = { text = it }, minLines = 6, maxLines = 12, modifier = Modifier.fillMaxWidth())
                        if (text.isBlank()) {
                            val example = tr("## 今日のできごと\n\n\n## 思ったこと\n\n\n## 明日やること\n\n- [ ] \n", "## What happened\n\n\n## Thoughts\n\n\n## Tomorrow\n\n- [ ] \n")
                            TextButton(onClick = { text = example }) { Text(tr("例を入れる", "Use an example")) }
                        }
                    }
                },
                confirmButton = { Button(onClick = { model.setPref(ask.key, JsonPrimitive(text)); onDismiss() }) { Text(tr("保存", "Save")) } },
                dismissButton = { TextButton(onClick = onDismiss) { Text(tr("キャンセル", "Cancel")) } },
            )
        }
        SettingAsk.Encryption -> {
            var password by remember { mutableStateOf("") }
            var busy by remember { mutableStateOf(false) }
            var error by remember { mutableStateOf<String?>(null) }
            val enabling = !sync.hasMasterPassword
            AlertDialog(
                onDismissRequest = { if (!busy) onDismiss() },
                title = { Text(if (enabling) tr("暗号化を有効にする", "Turn encryption on") else tr("暗号化を無効にする", "Turn encryption off")) },
                text = {
                    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text(
                            if (enabling) tr("同期先のデータをこのパスワードで暗号化します。ほかの端末でも同じパスワードが必要です。忘れると復元できません。", "What is synced is encrypted with this password. Your other devices need it too. If it is lost, the notes cannot be recovered.")
                            else tr("同期先のデータを暗号化しない形で置き直します。", "What is synced is stored unencrypted again."),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        if (enabling) {
                            OutlinedTextField(
                                value = password,
                                onValueChange = { password = it },
                                label = { Text(tr("パスワード", "Password")) },
                                singleLine = true,
                                visualTransformation = PasswordVisualTransformation(),
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                                modifier = Modifier.fillMaxWidth(),
                            )
                        }
                        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                    }
                },
                confirmButton = {
                    Button(
                        enabled = !busy && (!enabling || password.length >= 4),
                        onClick = {
                            busy = true
                            scope.launch {
                                runCatching { Api.setEncryption(enabling, password.takeIf { enabling }) }
                                    .onSuccess {
                                        model.refreshSyncSettings()
                                        model.syncNow()
                                        onDismiss()
                                    }
                                    .onFailure { error = it.message }
                                busy = false
                            }
                        },
                    ) { Text(if (enabling) tr("有効にする", "Turn on") else tr("無効にする", "Turn off")) }
                },
                dismissButton = { TextButton(onClick = onDismiss, enabled = !busy) { Text(tr("キャンセル", "Cancel")) } },
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Statistics

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun StatsScreen(model: AppModel, onBack: () -> Unit) {
    val bar = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    var stats by remember { mutableStateOf<app.nisshoku.android.engine.Stats?>(null) }
    LaunchedEffect(Unit) { stats = runCatching { Api.stats() }.getOrNull() }
    val goal = model.prefInt("dailyGoal", 0)
    Scaffold(
        modifier = Modifier.nestedScroll(bar.nestedScrollConnection),
        topBar = {
            LargeFlexibleTopAppBar(
                title = { Text(tr("統計", "Statistics")) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, tr("戻る", "Back")) } },
                scrollBehavior = bar,
            )
        },
    ) { padding ->
        val s = stats
        if (s == null) {
            Column(Modifier.padding(padding).fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) { LoadingIndicator(Modifier.padding(48.dp)) }
            return@Scaffold
        }
        val days = s.days.mapNotNull { a -> (a.getOrNull(0) as? JsonPrimitive)?.content?.let { d -> d to ((a.getOrNull(1) as? JsonPrimitive)?.longOrNull ?: 0L) } }
        val month = days.sumOf { it.second }
        LazyColumn(
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = padding.calculateTopPadding(), bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            item {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    StatTile(tr("今日", "Today"), tr("${"%,d".format(s.today)}字", "%,d".format(s.today)), Modifier.weight(1f))
                    StatTile(tr("連続", "Streak"), tr("${s.streak}日", "${s.streak} d"), Modifier.weight(1f))
                }
            }
            item {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    StatTile(tr("この30日", "Last 30 days"), tr("${"%,d".format(month)}字", "%,d".format(month)), Modifier.weight(1f))
                    StatTile(tr("ノート", "Notes"), "%,d".format(s.totalNotes), Modifier.weight(1f))
                }
            }
            item {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    StatTile(tr("総文字数", "Total"), "%,d".format(s.totalChars), Modifier.weight(1f))
                    StatTile(tr("原稿用紙換算", "Manuscript pages"), tr("${(s.totalChars + 399) / 400}枚", "${(s.totalChars + 399) / 400}"), Modifier.weight(1f))
                }
            }
            item {
                Surface(shape = RoundedCornerShape(24.dp), color = MaterialTheme.colorScheme.surfaceContainerLow) {
                    Column(Modifier.padding(18.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Text(tr("この30日間", "The last 30 days"), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                        DayBars(days.map { it.second }, goal)
                        Row {
                            Text(days.firstOrNull()?.first?.substring(5)?.replace('-', '/') ?: "", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
                            Spacer(Modifier.weight(1f))
                            Text(tr("今日", "Today"), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
                        }
                    }
                }
            }
        }
    }
}

/** A bar a day, the goal (if any) as a line. */
@Composable
private fun DayBars(values: List<Long>, goal: Int) {
    val primary = MaterialTheme.colorScheme.primary
    val track = MaterialTheme.colorScheme.surfaceContainerHighest
    val line = MaterialTheme.colorScheme.tertiary
    val max = maxOf(values.maxOrNull() ?: 0L, goal.toLong(), 1L).toFloat()
    Canvas(Modifier.fillMaxWidth().height(140.dp)) {
        if (values.isEmpty()) return@Canvas
        val gap = 3.dp.toPx()
        val w = (size.width - gap * (values.size - 1)) / values.size
        values.forEachIndexed { i, v ->
            val x = i * (w + gap)
            drawRoundRect(track, Offset(x, 0f), Size(w, size.height), CornerRadius(w / 2))
            val h = (v / max) * size.height
            if (v > 0) drawRoundRect(primary, Offset(x, size.height - h), Size(w, maxOf(h, w)), CornerRadius(w / 2))
        }
        if (goal > 0) {
            val y = size.height - goal / max * size.height
            drawLine(line, Offset(0f, y), Offset(size.width, y), strokeWidth = 2.dp.toPx())
        }
    }
}
