package app.nisshoku.desktop

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.res.ColorStateList
import android.content.res.Configuration
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.View
import android.widget.RemoteViews
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.text.NumberFormat
import java.util.Calendar
import java.util.Locale

// The home-screen widgets. They show the snapshot the app writes
// (`widgets.json` in its data directory, see src-tauri/src/widgets.rs) and
// never open the database. Ticking a task off leaves an action file in
// `widget-actions/` for the app to apply, and updates the snapshot so that
// the widget shows it at once.

class TasksWidget : AppWidgetProvider() {
  override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) =
    Widgets.refreshAll(context)

  override fun onAppWidgetOptionsChanged(context: Context, manager: AppWidgetManager, id: Int, options: Bundle) =
    Widgets.refreshAll(context)

  override fun onReceive(context: Context, intent: Intent) {
    if (intent.action == Widgets.ACTION_COMPLETE) {
      intent.data?.lastPathSegment?.let { Widgets.complete(context, it) }
    } else {
      super.onReceive(context, intent)
    }
  }
}

class WritingWidget : AppWidgetProvider() {
  override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) =
    Widgets.refreshAll(context)
}

class DiaryWidget : AppWidgetProvider() {
  override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) =
    Widgets.refreshAll(context)
}

object Widgets {
  const val ACTION_COMPLETE = "app.nisshoku.desktop.COMPLETE_TASK"
  private const val SNAPSHOT = "widgets.json"
  private const val ACTIONS = "widget-actions"

  private class Snapshot(val json: JSONObject?) {
    val language: String =
      json?.optString("language")?.takeIf { it == "ja" || it == "en" }
        ?: if (Locale.getDefault().language == "ja") "ja" else "en"
    val ja get() = language == "ja"
    fun t(ja: String, en: String) = if (this.ja) ja else en

    /** The snapshot is of today (otherwise today's figures start again). */
    val isToday get() = json?.optString("date") == today()

    /** The app's theme (`light`, `dark`, `system`) and accent colour. */
    val theme: String = json?.optString("theme")?.ifEmpty { null } ?: "system"
    val accent: String = json?.optString("accent")?.ifEmpty { null } ?: "orange"
  }

  /** The app's colours (src/styles/tokens.css and layout.css), for its theme and accent. */
  private class Palette(
    /** The app sets its own theme; with `system`, the resources follow the system's (values-night). */
    val explicit: Boolean,
    /** An accent other than the resources' orange. */
    val accentExplicit: Boolean,
    val dark: Boolean,
    val text: Int,
    val text2: Int,
    val text3: Int,
    val border: Int,
    val danger: Int,
    val accent: Int,
    val accentText: Int,
    val accentSoft: Int,
  ) {
    /** Text in a base colour of the theme. */
    fun text(views: RemoteViews, id: Int, color: Int) { if (explicit) views.setTextColor(id, color) }
    /** An icon in a base colour of the theme. */
    fun icon(views: RemoteViews, id: Int, color: Int) { if (explicit) views.setInt(id, "setColorFilter", color) }
    fun accentText(views: RemoteViews, id: Int) { if (explicit || accentExplicit) views.setTextColor(id, accentText) }
    fun accentIcon(views: RemoteViews, id: Int) { if (explicit || accentExplicit) views.setInt(id, "setColorFilter", accentText) }
  }

  private fun palette(context: Context, s: Snapshot): Palette {
    val systemDark = (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
    val dark = when (s.theme) {
      "dark" -> true
      "light" -> false
      else -> systemDark
    }
    // accent, accent text, accent soft: light then dark.
    val accents = mapOf(
      "orange" to listOf(0xFFD9622B, 0xFFB04C1B, 0xFFF7E4D7, 0xFFE7804B, 0xFFF0A073, 0xFF4A3326),
      "vermilion" to listOf(0xFFC8392B, 0xFFA8301F, 0xFFF6DCD8, 0xFFE3644F, 0xFFF08A77, 0xFF4A2A24),
      "indigo" to listOf(0xFF3F5EA8, 0xFF34508F, 0xFFDDE4F4, 0xFF7D9BE0, 0xFFA3B9EC, 0xFF283453),
      "green" to listOf(0xFF4B8A5E, 0xFF3C6F4C, 0xFFDCECDF, 0xFF79B98B, 0xFF9DD0AB, 0xFF24392B),
      "sumi" to listOf(0xFF4A4540, 0xFF3A3632, 0xFFE6E2DB, 0xFFCFC8BD, 0xFFE2DCD2, 0xFF3A3631),
    )
    val a = (accents[s.accent] ?: accents.getValue("orange")).map { it.toInt() }
    return if (dark) {
      Palette(s.theme != "system", s.accent != "orange", true, 0xFFECE8E0.toInt(), 0xFFB4ADA1.toInt(), 0xFF847E74.toInt(), 0xFF3C3933.toInt(), 0xFFE0705C.toInt(), a[3], a[4], a[5])
    } else {
      Palette(s.theme != "system", s.accent != "orange", false, 0xFF2C2A26.toInt(), 0xFF69645B.toInt(), 0xFF9B958B.toInt(), 0xFFE6E1D5.toInt(), 0xFFC2412D.toInt(), a[0], a[1], a[2])
    }
  }

  /** The widget's background in the palette's theme. */
  private fun paintRoot(views: RemoteViews, p: Palette) {
    if (!p.explicit) return
    views.setInt(R.id.widget_root, "setBackgroundResource", if (p.dark) R.drawable.widget_background_dark else R.drawable.widget_background_light)
  }

  /** A pill-shaped button in the accent colour (tinting needs Android 12; before, the resource colours stay). */
  private fun paintPill(views: RemoteViews, id: Int, p: Palette) {
    if ((p.explicit || p.accentExplicit) && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      views.setColorStateList(id, "setBackgroundTintList", ColorStateList.valueOf(p.accentSoft))
    }
  }

  private fun today(): String {
    val c = Calendar.getInstance()
    return String.format(Locale.ROOT, "%04d-%02d-%02d", c.get(Calendar.YEAR), c.get(Calendar.MONTH) + 1, c.get(Calendar.DAY_OF_MONTH))
  }

  private fun snapshotFile(context: Context) = File(context.dataDir, SNAPSHOT)

  private fun read(context: Context): Snapshot =
    Snapshot(runCatching { JSONObject(snapshotFile(context).readText()) }.getOrNull())

  private fun writeAtomically(file: File, text: String) {
    val tmp = File(file.parentFile, file.name + ".widget.tmp")
    tmp.writeText(text)
    tmp.renameTo(file)
  }

  // ---------------------------------------------------------------------
  // Opening the app

  /** Opens the app at `uri` (`nisshoku://note/<id>`, `nisshoku://new-task`…). */
  private fun open(context: Context, uri: String): PendingIntent {
    val intent = Intent(context, MainActivity::class.java)
      .setAction(Intent.ACTION_VIEW)
      .setData(Uri.parse(uri))
      .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    return PendingIntent.getActivity(context, uri.hashCode(), intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
  }

  private fun completeIntent(context: Context, id: String): PendingIntent {
    val intent = Intent(context, TasksWidget::class.java)
      .setAction(ACTION_COMPLETE)
      .setData(Uri.parse("nisshoku-widget://complete/$id"))
    return PendingIntent.getBroadcast(context, id.hashCode(), intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
  }

  // ---------------------------------------------------------------------
  // Rendering

  fun refreshAll(context: Context) {
    val manager = AppWidgetManager.getInstance(context)
    val snapshot = read(context)
    for (id in manager.getAppWidgetIds(ComponentName(context, TasksWidget::class.java))) {
      manager.updateAppWidget(id, tasks(context, snapshot, manager.getAppWidgetOptions(id)))
    }
    for (id in manager.getAppWidgetIds(ComponentName(context, WritingWidget::class.java))) {
      manager.updateAppWidget(id, writing(context, snapshot))
    }
    for (id in manager.getAppWidgetIds(ComponentName(context, DiaryWidget::class.java))) {
      manager.updateAppWidget(id, diary(context, snapshot))
    }
  }

  private fun format(n: Long, s: Snapshot) = NumberFormat.getIntegerInstance(if (s.ja) Locale.JAPAN else Locale.US).format(n)

  private fun startOfDay(ms: Long): Long = Calendar.getInstance().apply {
    timeInMillis = ms
    set(Calendar.HOUR_OF_DAY, 0); set(Calendar.MINUTE, 0); set(Calendar.SECOND, 0); set(Calendar.MILLISECOND, 0)
  }.timeInMillis

  /** Days from today to `ms` (negative: past). */
  private fun dayOffset(ms: Long): Int =
    Math.round((startOfDay(ms) - startOfDay(System.currentTimeMillis())) / 86_400_000.0).toInt()

  /** As the app's dueLabel: "Today", "Tomorrow 14:30", "Fri", "11/20". */
  private fun dueLabel(due: Long, allDay: Boolean, s: Snapshot): String {
    val c = Calendar.getInstance().apply { timeInMillis = due }
    val days = dayOffset(due)
    val day = when {
      days == 0 -> s.t("今日", "Today")
      days == 1 -> s.t("明日", "Tomorrow")
      days == -1 -> s.t("昨日", "Yesterday")
      days in 2..6 -> if (s.ja) "${"日月火水木金土"[c.get(Calendar.DAY_OF_WEEK) - 1]}曜" else
        c.getDisplayName(Calendar.DAY_OF_WEEK, Calendar.SHORT, Locale.US) ?: ""
      else -> if (s.ja) "${c.get(Calendar.MONTH) + 1}/${c.get(Calendar.DAY_OF_MONTH)}" else
        "${c.getDisplayName(Calendar.MONTH, Calendar.SHORT, Locale.US)} ${c.get(Calendar.DAY_OF_MONTH)}"
    }
    if (allDay) return day
    return String.format(Locale.ROOT, "%s %02d:%02d", day, c.get(Calendar.HOUR_OF_DAY), c.get(Calendar.MINUTE))
  }

  private fun overdue(due: Long, allDay: Boolean) =
    due > 0 && (if (allDay) dayOffset(due) < 0 else due < System.currentTimeMillis())

  private fun tasks(context: Context, s: Snapshot, options: Bundle): RemoteViews {
    val views = RemoteViews(context.packageName, R.layout.widget_tasks)
    val p = palette(context, s)
    paintRoot(views, p)
    paintPill(views, R.id.add, p)
    p.accentIcon(views, R.id.add)
    p.text(views, R.id.title, p.text)
    p.text(views, R.id.empty, p.text3)
    p.text(views, R.id.more, p.text3)
    val tasks = s.json?.optJSONArray("tasks") ?: JSONArray()
    val open = s.json?.optInt("openTasks") ?: 0
    views.setTextViewText(R.id.title, if (open > 0) "${s.t("タスク", "Tasks")}  $open" else s.t("タスク", "Tasks"))
    views.setOnClickPendingIntent(R.id.add, open(context, "nisshoku://new-task"))
    views.setOnClickPendingIntent(R.id.widget_root, open(context, "nisshoku://tasks"))
    views.removeAllViews(R.id.tasks)

    // As many rows as the widget is tall (header 40dp, rows 44dp, padding
    // 24dp). The portrait height is the "max" one (the "min", landscape).
    val height = maxOf(
      options.getInt(AppWidgetManager.OPTION_APPWIDGET_MAX_HEIGHT, 0),
      options.getInt(AppWidgetManager.OPTION_APPWIDGET_MIN_HEIGHT, 180),
    )
    val rows = ((height - 64 - 18) / 44).coerceIn(1, 8)
    val shown = minOf(rows, tasks.length())
    for (i in 0 until shown) {
      val task = tasks.getJSONObject(i)
      val id = task.getString("id")
      val row = RemoteViews(context.packageName, R.layout.widget_task_row)
      p.text(row, R.id.task_title, p.text)
      p.text(row, R.id.due, p.text2)
      p.text(row, R.id.due_overdue, p.danger)
      p.icon(row, R.id.check, p.text3)
      row.setTextViewText(R.id.task_title, task.optString("title").ifEmpty { s.t("無題", "Untitled") })
      val due = task.optLong("due")
      val allDay = task.optBoolean("allDay")
      val late = overdue(due, allDay)
      val label = if (due > 0) dueLabel(due, allDay, s) else ""
      row.setTextViewText(if (late) R.id.due_overdue else R.id.due, label)
      row.setViewVisibility(R.id.due, if (late) View.GONE else View.VISIBLE)
      row.setViewVisibility(R.id.due_overdue, if (late) View.VISIBLE else View.GONE)
      row.setOnClickPendingIntent(R.id.check, completeIntent(context, id))
      row.setOnClickPendingIntent(R.id.row, open(context, "nisshoku://note/$id"))
      views.addView(R.id.tasks, row)
    }

    val rest = open - shown
    views.setViewVisibility(R.id.more, if (rest > 0) View.VISIBLE else View.GONE)
    views.setTextViewText(R.id.more, s.t("ほか ${rest}件", "$rest more"))
    views.setViewVisibility(R.id.empty, if (shown == 0) View.VISIBLE else View.GONE)
    views.setTextViewText(
      R.id.empty,
      if (s.json == null) s.t("日色を開くと表示されます", "Open Nisshoku to show your tasks")
      else s.t("タスクはありません 🎉", "Nothing to do 🎉"),
    )
    return views
  }

  private fun writing(context: Context, s: Snapshot): RemoteViews {
    val views = RemoteViews(context.packageName, R.layout.widget_writing)
    val p = palette(context, s)
    paintRoot(views, p)
    p.text(views, R.id.label, p.text2)
    p.text(views, R.id.count, p.text)
    p.text(views, R.id.unit, p.text2)
    p.accentText(views, R.id.detail)
    if ((p.explicit || p.accentExplicit) && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      views.setColorStateList(R.id.progress, "setProgressTintList", ColorStateList.valueOf(p.accent))
      views.setColorStateList(R.id.progress, "setProgressBackgroundTintList", ColorStateList.valueOf(p.border))
    }
    val w = s.json?.optJSONObject("writing")
    val today = if (s.isToday) w?.optLong("today") ?: 0 else 0
    val goal = w?.optLong("goal") ?: 0
    // A streak still counts today until the day is over.
    val streak = w?.optInt("streak") ?: 0
    views.setTextViewText(R.id.label, s.t("今日の執筆", "Written today"))
    views.setTextViewText(R.id.count, format(today, s))
    views.setTextViewText(R.id.unit, s.t("字", "chars"))
    if (goal > 0) {
      views.setViewVisibility(R.id.progress, View.VISIBLE)
      views.setProgressBar(R.id.progress, goal.toInt(), minOf(today, goal).toInt(), false)
    } else {
      views.setViewVisibility(R.id.progress, View.GONE)
    }
    val detail = when {
      goal > 0 && today >= goal -> s.t("目標を達成しました", "Goal reached")
      goal > 0 -> s.t("目標まであと ${format(goal - today, s)}字", "${format(goal - today, s)} to go")
      streak > 0 -> ""
      else -> s.t("今日も少し書いてみましょう", "Write a little today")
    }
    val streakText = if (streak > 0) s.t("${streak}日連続", "$streak-day streak") else ""
    views.setTextViewText(R.id.detail, listOf(streakText, detail).filter { it.isNotEmpty() }.joinToString(" · "))
    views.setOnClickPendingIntent(R.id.widget_root, open(context, "nisshoku://open"))
    return views
  }

  private fun diary(context: Context, s: Snapshot): RemoteViews {
    val views = RemoteViews(context.packageName, R.layout.widget_diary)
    val p = palette(context, s)
    paintRoot(views, p)
    p.text(views, R.id.weekday, p.text2)
    p.text(views, R.id.date, p.text)
    p.text(views, R.id.status, p.text3)
    for (id in listOf(R.id.write, R.id.new_note)) {
      paintPill(views, id, p)
      p.accentText(views, id)
    }
    val c = Calendar.getInstance()
    views.setTextViewText(
      R.id.weekday,
      if (s.ja) "${c.get(Calendar.YEAR)}年 ${"日月火水木金土"[c.get(Calendar.DAY_OF_WEEK) - 1]}曜日"
      else "${c.getDisplayName(Calendar.DAY_OF_WEEK, Calendar.LONG, Locale.US)}, ${c.get(Calendar.YEAR)}",
    )
    views.setTextViewText(
      R.id.date,
      if (s.ja) "${c.get(Calendar.MONTH) + 1}月${c.get(Calendar.DAY_OF_MONTH)}日"
      else "${c.getDisplayName(Calendar.MONTH, Calendar.LONG, Locale.US)} ${c.get(Calendar.DAY_OF_MONTH)}",
    )
    val d = if (s.isToday) s.json?.optJSONObject("diary") else null
    val chars = d?.optLong("chars") ?: 0
    val written = d != null && !d.isNull("noteId")
    views.setTextViewText(
      R.id.status,
      when {
        written && chars > 0 -> s.t("${format(chars, s)}字 書きました", "${format(chars, s)} chars written")
        written -> s.t("書きはじめました", "Started")
        else -> s.t("まだ書いていません", "Not written yet")
      },
    )
    views.setTextViewText(R.id.write, if (written) s.t("続きを書く", "Continue") else s.t("日記を書く", "Write"))
    views.setTextViewText(R.id.new_note, s.t("新しいノート", "New note"))
    views.setOnClickPendingIntent(R.id.write, open(context, "nisshoku://diary"))
    views.setOnClickPendingIntent(R.id.new_note, open(context, "nisshoku://new-note"))
    views.setOnClickPendingIntent(R.id.widget_root, open(context, "nisshoku://diary"))
    return views
  }

  // ---------------------------------------------------------------------
  // Actions

  /** Ticks a task off from the widget. */
  fun complete(context: Context, id: String) {
    val now = System.currentTimeMillis()
    val dir = File(context.dataDir, ACTIONS).apply { mkdirs() }
    val action = JSONObject().put("action", "complete").put("id", id).put("time", now)
    // Named so that the app applies them in order.
    writeAtomically(File(dir, "$now-${id.take(8)}.json"), action.toString())

    // Shows it at once: the app rewrites the snapshot when it applies the action.
    val file = snapshotFile(context)
    runCatching {
      val json = JSONObject(file.readText())
      val tasks = json.getJSONArray("tasks")
      val kept = JSONArray()
      for (i in 0 until tasks.length()) {
        if (tasks.getJSONObject(i).optString("id") != id) kept.put(tasks.getJSONObject(i))
      }
      if (kept.length() < tasks.length()) {
        json.put("tasks", kept).put("openTasks", maxOf(0, json.optInt("openTasks") - 1))
        writeAtomically(file, json.toString())
      }
    }
    refreshAll(context)
    MainActivity.onWidgetAction()
  }
}
