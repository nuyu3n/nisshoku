package app.nisshoku.android.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Event
import androidx.compose.material3.Button
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.nisshoku.android.tr
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter

/** When a task due on `date` (a day, no time) is due: 9:00 that day, as the engine keeps them. */
fun allDayDue(date: LocalDate): Long = date.atTime(9, 0).atZone(ZoneId.systemDefault()).toInstant().toEpochMilli()

/** A new task: what to do, and optionally the day it is due. The keyboard is up from the start. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NewTaskSheet(onDismiss: () -> Unit, onAdd: (title: String, due: Long?) -> Unit) {
    var title by remember { mutableStateOf("") }
    var due by remember { mutableStateOf<LocalDate?>(null) }
    var picking by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    val today = remember { LocalDate.now() }
    fun add() {
        if (title.isNotBlank()) onAdd(title.trim(), due?.let(::allDayDue))
    }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.padding(horizontal = 20.dp).padding(bottom = 20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(tr("新しいタスク", "New task"), style = MaterialTheme.typography.titleLarge)
            OutlinedTextField(
                value = title,
                onValueChange = { title = it },
                placeholder = { Text(tr("やること", "What to do")) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { add() }),
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(selected = due == today, onClick = { due = if (due == today) null else today }, label = { Text(tr("今日", "Today")) })
                val tomorrow = today.plusDays(1)
                FilterChip(selected = due == tomorrow, onClick = { due = if (due == tomorrow) null else tomorrow }, label = { Text(tr("明日", "Tomorrow")) })
                val other = due != null && due != today && due != tomorrow
                FilterChip(
                    selected = other,
                    onClick = { picking = true },
                    leadingIcon = { Icon(Icons.Outlined.Event, null) },
                    label = { Text(if (other) due!!.format(DateTimeFormatter.ofPattern("M/d")) else tr("日付", "Date")) },
                )
            }
            Button(onClick = ::add, enabled = title.isNotBlank(), modifier = Modifier.fillMaxWidth().height(52.dp)) {
                Text(tr("追加", "Add"))
            }
        }
    }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    if (picking) {
        DuePicker(
            initial = due?.let(::allDayDue) ?: 0,
            onDismiss = { picking = false },
            onPick = {
                due = it
                picking = false
            },
        )
    }
}

/** Picks the day something is due. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DuePicker(initial: Long, onDismiss: () -> Unit, onPick: (LocalDate) -> Unit) {
    // The picker counts in UTC days.
    val start = (if (initial > 0) Instant.ofEpochMilli(initial).atZone(ZoneId.systemDefault()).toLocalDate() else LocalDate.now())
        .atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli()
    val state = rememberDatePickerState(initialSelectedDateMillis = start)
    DatePickerDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(
                onClick = { state.selectedDateMillis?.let { onPick(Instant.ofEpochMilli(it).atZone(ZoneOffset.UTC).toLocalDate()) } },
                enabled = state.selectedDateMillis != null,
            ) { Text(tr("決定", "OK")) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("キャンセル", "Cancel")) } },
    ) {
        DatePicker(state = state)
    }
}
