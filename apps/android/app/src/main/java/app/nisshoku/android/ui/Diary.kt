package app.nisshoku.android.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.outlined.KeyboardArrowRight
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.nisshoku.android.LocalEnglish
import app.nisshoku.android.engine.Api
import app.nisshoku.android.engine.DiaryDay
import app.nisshoku.android.tr
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.YearMonth
import java.time.format.TextStyle
import java.util.Locale

/**
 * The diary's month: the days written are filled, today is ringed. A tap
 * opens the day's entry (made if there is none yet). `version` changes when
 * the notes do.
 */
@Composable
fun DiaryCalendar(version: Int, weekStartsMonday: Boolean, onOpen: (String) -> Unit) {
    val today = remember { LocalDate.now() }
    var monthText by rememberSaveable { mutableStateOf(YearMonth.from(today).toString()) }
    val month = YearMonth.parse(monthText)
    var days by remember { mutableStateOf<Map<String, DiaryDay>>(emptyMap()) }
    val english = LocalEnglish.current
    LaunchedEffect(monthText, version) {
        days = runCatching { Api.diaryMonth(monthText) }.getOrDefault(emptyList()).associateBy { it.date }
    }

    Surface(shape = RoundedCornerShape(28.dp), color = MaterialTheme.colorScheme.surfaceContainerLow, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(horizontal = 10.dp, vertical = 8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = { monthText = month.minusMonths(1).toString() }) {
                    Icon(Icons.AutoMirrored.Outlined.KeyboardArrowLeft, tr("前の月", "Previous month"))
                }
                Text(
                    if (english) "${month.month.getDisplayName(TextStyle.FULL, Locale.US)} ${month.year}" else "${month.year}年${month.monthValue}月",
                    style = MaterialTheme.typography.titleMedium,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.weight(1f),
                )
                if (month != YearMonth.from(today)) {
                    TextButton(onClick = { monthText = YearMonth.from(today).toString() }) { Text(tr("今月", "This month")) }
                }
                IconButton(onClick = { monthText = month.plusMonths(1).toString() }, enabled = month < YearMonth.from(today)) {
                    Icon(Icons.AutoMirrored.Outlined.KeyboardArrowRight, tr("次の月", "Next month"))
                }
            }
            val first = if (weekStartsMonday) DayOfWeek.MONDAY else DayOfWeek.SUNDAY
            Row(Modifier.fillMaxWidth()) {
                for (i in 0 until 7) {
                    val day = first.plus(i.toLong())
                    Text(
                        if (english) day.getDisplayName(TextStyle.NARROW, Locale.US) else "月火水木金土日"[day.value - 1].toString(),
                        style = MaterialTheme.typography.labelMedium,
                        color = when (day) {
                            DayOfWeek.SUNDAY -> MaterialTheme.colorScheme.error
                            DayOfWeek.SATURDAY -> MaterialTheme.colorScheme.tertiary
                            else -> MaterialTheme.colorScheme.outline
                        },
                        textAlign = TextAlign.Center,
                        modifier = Modifier.weight(1f).padding(vertical = 4.dp),
                    )
                }
            }
            AnimatedContent(
                targetState = month,
                transitionSpec = {
                    val forward = targetState > initialState
                    (slideInHorizontally { if (forward) it / 4 else -it / 4 } + fadeIn()) togetherWith
                        (slideOutHorizontally { if (forward) -it / 4 else it / 4 } + fadeOut())
                },
                label = "month",
            ) { shown ->
                Column {
                    val offset = (shown.atDay(1).dayOfWeek.value - first.value + 7) % 7
                    val cells = offset + shown.lengthOfMonth()
                    for (week in 0 until (cells + 6) / 7) {
                        Row(Modifier.fillMaxWidth()) {
                            for (i in 0 until 7) {
                                val n = week * 7 + i - offset + 1
                                Box(Modifier.weight(1f).aspectRatio(1f).padding(3.dp), contentAlignment = Alignment.Center) {
                                    if (n in 1..shown.lengthOfMonth()) {
                                        val date = shown.atDay(n)
                                        val entry = days[date.toString()].takeIf { shown == month }
                                        DayCell(n, entry, date == today, date > today) { onOpen(date.toString()) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun DayCell(day: Int, entry: DiaryDay?, today: Boolean, future: Boolean, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val written = entry != null
    val label = tr("${day}日", "Day $day") + if (written) tr("、記入済み", ", written") else ""
    Box(
        Modifier
            .fillMaxSize()
            .clip(CircleShape)
            .then(if (today && !written) Modifier.border(2.dp, scheme.primary, CircleShape) else Modifier)
            .then(if (written) Modifier.border(0.dp, Color.Transparent) else Modifier)
            .clickable(enabled = !future, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        // A day written is filled (in its mood's colour, if it has one).
        val mood = moodOf(entry?.mood)
        if (written) {
            Surface(color = mood?.color?.copy(alpha = if (today) 1f else 0.35f) ?: if (today) scheme.primary else scheme.primaryContainer, shape = CircleShape, modifier = Modifier.fillMaxSize()) {}
        }
        Text(
            "$day",
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = if (today || written) FontWeight.SemiBold else FontWeight.Normal,
            color = when {
                written && today -> scheme.onPrimary
                written -> scheme.onPrimaryContainer
                future -> scheme.outline.copy(alpha = 0.5f)
                today -> scheme.primary
                else -> scheme.onSurface
            },
        )
    }
}
