package app.nisshoku.android.ui

import android.text.format.DateUtils
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.PushPin
import androidx.compose.material.icons.outlined.RadioButtonUnchecked
import androidx.compose.material.icons.outlined.WarningAmber
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.nisshoku.android.engine.NoteSummary
import app.nisshoku.android.tr
import app.nisshoku.android.trNow
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.temporal.ChronoUnit
import app.nisshoku.android.ui.theme.noteTitle

/**
 * The shape of item `index` of `count` in a group (Material 3 Expressive's
 * segmented lists): the group's outer corners round, the inner ones tight.
 */
fun groupShape(index: Int, count: Int, outer: Int = 24, inner: Int = 6): Shape {
    val top = if (index == 0) outer else inner
    val bottom = if (index == count - 1) outer else inner
    return RoundedCornerShape(topStart = top.dp, topEnd = top.dp, bottomStart = bottom.dp, bottomEnd = bottom.dp)
}

/** "Just now", "3 minutes ago", "Yesterday", a date… */
fun relativeTime(ms: Long): String = when {
    ms <= 0 -> ""
    // (Not "0 min. ago"; nor "in 1 min." for a clock a little ahead of ours.)
    kotlin.math.abs(System.currentTimeMillis() - ms) < 60_000 -> trNow("たった今", "Just now")
    else -> DateUtils.getRelativeTimeSpanString(ms, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS, DateUtils.FORMAT_ABBREV_RELATIVE).toString()
}

/** When a task is due: "Today", "Tomorrow 14:30", else the date. */
fun dueText(ms: Long, allDay: Boolean): String {
    if (ms <= 0) return ""
    val date = Instant.ofEpochMilli(ms).atZone(ZoneId.systemDefault())
    val day = when (ChronoUnit.DAYS.between(LocalDate.now(), date.toLocalDate())) {
        0L -> trNow("今日", "Today")
        1L -> trNow("明日", "Tomorrow")
        -1L -> trNow("昨日", "Yesterday")
        else -> DateUtils.formatDateTime(app.nisshoku.android.engine.Nisshoku.context, ms, DateUtils.FORMAT_SHOW_DATE or DateUtils.FORMAT_ABBREV_MONTH)
    }
    return if (allDay) day else "$day ${"%02d:%02d".format(date.hour, date.minute)}"
}

/** Whether a task is past its due time (one due on a day: once the day is over). */
fun overdue(ms: Long, allDay: Boolean): Boolean = ms > 0 &&
    if (allDay) Instant.ofEpochMilli(ms).atZone(ZoneId.systemDefault()).toLocalDate() < LocalDate.now() else ms < System.currentTimeMillis()

/** How the lists show notes (the preferences `showExcerpt`, `compactList`). */
data class ListLook(val excerpt: Boolean = true, val compact: Boolean = false)

val LocalListLook = androidx.compose.runtime.compositionLocalOf { ListLook() }

/** A note in a list: its title in the paper's serif, a line of its text, when it changed. */
@Composable
fun NoteCard(
    note: NoteSummary,
    shape: Shape,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
    onToggleDone: (() -> Unit)?,
    modifier: Modifier = Modifier,
    /** Words searched for, marked where they appear. */
    highlight: String? = null,
) {
    val haptics = LocalHapticFeedback.current
    val mark = SpanStyle(background = MaterialTheme.colorScheme.primaryContainer, color = MaterialTheme.colorScheme.onPrimaryContainer)
    val look = LocalListLook.current
    val pad = if (look.compact) 9.dp else 14.dp
    Surface(
        shape = shape,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .combinedClickable(
                onClick = onClick,
                onLongClick = {
                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                    onLongClick()
                },
            ),
    ) {
        Row(Modifier.padding(start = if (onToggleDone != null) 4.dp else 18.dp, end = 18.dp, top = pad, bottom = pad)) {
            if (onToggleDone != null) {
                val done by animateFloatAsState(if (note.todoCompleted) 1f else 0f, label = "done")
                IconButton(onClick = {
                    haptics.performHapticFeedback(HapticFeedbackType.ToggleOn)
                    onToggleDone()
                }) {
                    Icon(
                        if (note.todoCompleted) Icons.Outlined.CheckCircle else Icons.Outlined.RadioButtonUnchecked,
                        contentDescription = tr("完了", "Done"),
                        tint = if (note.todoCompleted) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline,
                        modifier = Modifier.scale(1f + 0.15f * done * (1f - done) * 4f),
                    )
                }
            }
            Column(Modifier.weight(1f).padding(top = if (onToggleDone != null && !look.compact) 10.dp else 0.dp).align(if (look.compact) Alignment.CenterVertically else Alignment.Top)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (note.isConflict) {
                        Icon(Icons.Outlined.WarningAmber, null, tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(6.dp))
                    }
                    Text(
                        highlighted(note.title.ifBlank { tr("無題", "Untitled") }, highlight, mark),
                        style = MaterialTheme.typography.titleMedium.noteTitle(),
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        textDecoration = if (note.todoCompleted) TextDecoration.LineThrough else null,
                        color = if (note.todoCompleted) MaterialTheme.colorScheme.outline else MaterialTheme.colorScheme.onSurface,
                        modifier = Modifier.weight(1f, fill = false),
                    )
                    if (note.pinned) {
                        Spacer(Modifier.width(6.dp))
                        Icon(Icons.Outlined.PushPin, tr("ピン留め", "Pinned"), tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(16.dp))
                    }
                }
                if (note.excerpt.isNotBlank() && look.excerpt) {
                    Spacer(Modifier.height(4.dp))
                    Text(
                        highlighted(note.excerpt, highlight, mark),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = if (look.compact) 1 else 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Spacer(Modifier.height(if (look.compact) 3.dp else 6.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
                    val due = if (note.isTodo && note.todoDue > 0) dueText(note.todoDue, note.dueAllDay) else ""
                    if (due.isNotEmpty()) {
                        val late = !note.todoCompleted && overdue(note.todoDue, note.dueAllDay)
                        Text(
                            due,
                            style = MaterialTheme.typography.labelMedium,
                            color = when {
                                note.todoCompleted -> MaterialTheme.colorScheme.outline
                                late -> MaterialTheme.colorScheme.error
                                else -> MaterialTheme.colorScheme.primary
                            },
                        )
                    }
                    Text(
                        (note.diaryDate?.takeIf { it != note.title } ?: relativeTime(note.updatedTime)),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.outline,
                    )
                    if (note.charCount > 0) {
                        Text(
                            tr("${note.charCount}字", "${note.charCount} chars"),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.outline,
                        )
                    }
                    note.tags.take(2).forEach { tag ->
                        Text(
                            "#$tag",
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.secondary,
                            maxLines = 1,
                        )
                    }
                }
            }
        }
    }
}

/** `text` with each word of `query` marked (ignoring case). */
fun highlighted(text: String, query: String?, style: SpanStyle): androidx.compose.ui.text.AnnotatedString =
    androidx.compose.ui.text.buildAnnotatedString {
        append(text)
        val words = query?.split(' ', '　')?.filter { it.isNotEmpty() && !it.contains(':') } ?: return@buildAnnotatedString
        for (word in words) {
            var at = text.indexOf(word, ignoreCase = true)
            while (at >= 0) {
                addStyle(style, at, at + word.length)
                at = text.indexOf(word, at + word.length, ignoreCase = true)
            }
        }
    }

/** A notebook's mark: its emoji, or a dot in its colour. */
@Composable
fun FolderMark(icon: String, color: String, fallback: ImageVector) {
    val tint = parseColor(color)
    Box(Modifier.size(28.dp), contentAlignment = Alignment.Center) {
        when {
            icon.isNotEmpty() -> Text(icon, style = MaterialTheme.typography.titleMedium)
            else -> Icon(fallback, null, tint = tint ?: MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (icon.isNotEmpty() && tint != null) {
            Box(
                Modifier
                    .align(Alignment.BottomEnd)
                    .size(9.dp)
                    .clip(CircleShape)
                    .background(tint),
            )
        }
    }
}

fun parseColor(hex: String): Color? = runCatching {
    if (hex.length == 7 && hex.startsWith("#")) Color(("FF" + hex.substring(1)).toLong(16)) else null
}.getOrNull()

/** What an empty list says. */
@Composable
fun EmptyState(icon: ImageVector, title: String, text: String, modifier: Modifier = Modifier) {
    Column(
        modifier.fillMaxSize().padding(40.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Surface(shape = CircleShape, color = MaterialTheme.colorScheme.primaryContainer, modifier = Modifier.size(96.dp)) {
            Box(contentAlignment = Alignment.Center) {
                Icon(icon, null, tint = MaterialTheme.colorScheme.onPrimaryContainer, modifier = Modifier.size(44.dp))
            }
        }
        Spacer(Modifier.height(20.dp))
        Text(title, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
        Spacer(Modifier.height(8.dp))
        Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
    }
}

/** A heading over a group of the list. */
@Composable
fun SectionLabel(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
        modifier = modifier.padding(start = 20.dp, top = 18.dp, bottom = 8.dp),
    )
}
