package app.nisshoku.android.ui

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.outlined.KeyboardArrowRight
import androidx.compose.material.icons.outlined.AcUnit
import androidx.compose.material.icons.outlined.Cloud
import androidx.compose.material.icons.outlined.FilterDrama
import androidx.compose.material.icons.outlined.SentimentDissatisfied
import androidx.compose.material.icons.outlined.SentimentNeutral
import androidx.compose.material.icons.outlined.SentimentSatisfied
import androidx.compose.material.icons.outlined.SentimentVeryDissatisfied
import androidx.compose.material.icons.outlined.SentimentVerySatisfied
import androidx.compose.material.icons.outlined.Thunderstorm
import androidx.compose.material.icons.outlined.Umbrella
import androidx.compose.material.icons.outlined.WbSunny
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import app.nisshoku.android.LocalEnglish
import app.nisshoku.android.engine.Api
import app.nisshoku.android.engine.Nisshoku
import app.nisshoku.android.tr
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import java.time.LocalDate
import java.time.format.TextStyle
import java.util.Locale

/** The moods of the diary (the web app's MOODS), with their colours. */
data class Mood(val key: String, val icon: ImageVector, val color: Color, val ja: String, val en: String)

val MOODS = listOf(
    Mood("great", Icons.Outlined.SentimentVerySatisfied, Color(0xFFE0773B), "とても良い", "Great"),
    Mood("good", Icons.Outlined.SentimentSatisfied, Color(0xFFE2A33C), "良い", "Good"),
    Mood("ok", Icons.Outlined.SentimentNeutral, Color(0xFFA39D92), "ふつう", "Okay"),
    Mood("low", Icons.Outlined.SentimentDissatisfied, Color(0xFF6F94B8), "いまいち", "Low"),
    Mood("bad", Icons.Outlined.SentimentVeryDissatisfied, Color(0xFF6A6FA8), "つらい", "Bad"),
)

private data class Weather(val key: String, val icon: ImageVector, val ja: String, val en: String)

private val WEATHERS = listOf(
    Weather("sunny", Icons.Outlined.WbSunny, "晴れ", "Sunny"),
    Weather("partly", Icons.Outlined.FilterDrama, "晴れ時々くもり", "Partly cloudy"),
    Weather("cloudy", Icons.Outlined.Cloud, "くもり", "Cloudy"),
    Weather("rain", Icons.Outlined.Umbrella, "雨", "Rain"),
    Weather("snow", Icons.Outlined.AcUnit, "雪", "Snow"),
    Weather("storm", Icons.Outlined.Thunderstorm, "雷雨", "Storm"),
)

/** Old entries kept an emoji. */
private val LEGACY = mapOf("😄" to "great", "🥰" to "great", "🙂" to "good", "😐" to "ok", "😴" to "ok", "😔" to "low", "😢" to "bad", "😠" to "bad")

fun moodOf(v: String?): Mood? = MOODS.find { it.key == (LEGACY[v] ?: v) }

/**
 * The head of a diary entry in the app's own editor: its day (with the
 * days before and after), and its mood and weather.
 */
@Composable
fun DiaryMetaBar(noteId: String, date: String, onDay: (String) -> Unit) {
    val english = LocalEnglish.current
    val haptics = LocalHapticFeedback.current
    val scope = rememberCoroutineScope()
    var mood by remember(noteId) { mutableStateOf<String?>(null) }
    var weather by remember(noteId) { mutableStateOf<String?>(null) }
    LaunchedEffect(noteId) {
        val e = runCatching { Api.diaryEntries() }.getOrDefault(emptyList()).find { it.noteId == noteId }
        mood = moodOf(e?.mood)?.key
        weather = WEATHERS.find { it.key == (LEGACY[e?.weather] ?: e?.weather) }?.key
    }
    fun save() {
        scope.launch {
            runCatching {
                Nisshoku.call("set_diary_meta", buildJsonObject {
                    put("noteId", JsonPrimitive(noteId))
                    put("mood", JsonPrimitive(mood ?: ""))
                    put("weather", JsonPrimitive(weather ?: ""))
                })
            }
        }
    }
    val day = runCatching { LocalDate.parse(date) }.getOrNull() ?: return
    val today = LocalDate.now()
    Column(Modifier.fillMaxWidth().padding(bottom = 8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { onDay(day.minusDays(1).toString()) }) {
                Icon(Icons.AutoMirrored.Outlined.KeyboardArrowLeft, tr("前の日", "The day before"))
            }
            Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                Text(
                    if (english) "${day.month.getDisplayName(TextStyle.FULL, Locale.US)} ${day.dayOfMonth}" else "${day.monthValue}月${day.dayOfMonth}日",
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    (if (english) "${day.dayOfWeek.getDisplayName(TextStyle.FULL, Locale.US)}, ${day.year}" else "${day.year}年 ${"月火水木金土日"[day.dayOfWeek.value - 1]}曜日") +
                        if (day == today) tr("・今日", " · Today") else "",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(onClick = { onDay(day.plusDays(1).toString()) }, enabled = day < today) {
                Icon(Icons.AutoMirrored.Outlined.KeyboardArrowRight, tr("次の日", "The day after"))
            }
        }
        Row(Modifier.horizontalScroll(rememberScrollState()).padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            MOODS.forEach { m ->
                val on = mood == m.key
                FilterChip(
                    selected = on,
                    onClick = {
                        haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                        mood = if (on) null else m.key
                        save()
                    },
                    label = { Text(if (english) m.en else m.ja) },
                    leadingIcon = { Icon(m.icon, null, tint = m.color, modifier = Modifier.size(20.dp)) },
                    colors = FilterChipDefaults.filterChipColors(selectedContainerColor = m.color.copy(alpha = 0.22f)),
                )
            }
        }
        Row(Modifier.horizontalScroll(rememberScrollState()).padding(top = 2.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            WEATHERS.forEach { w ->
                val on = weather == w.key
                FilterChip(
                    selected = on,
                    onClick = {
                        haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                        weather = if (on) null else w.key
                        save()
                    },
                    label = { Text(if (english) w.en else w.ja) },
                    leadingIcon = { Icon(w.icon, null, modifier = Modifier.size(20.dp)) },
                )
            }
        }
    }
}
