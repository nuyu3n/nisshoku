//! To-dos as tasks: quick creation and due dates.
//!
//! The due date is Joplin's `todo_due` (Unix ms; Joplin shows it and sets an
//! alarm for it). A task due on a day rather than at a time is kept at 9:00
//! that day (so that Joplin's alarm comes in the morning) and marked in
//! `user_data`, so that only the day is shown.

use nisshoku_core::Note;
use serde_json::json;
use tauri::State;

use crate::i18n::tr;
use crate::notes::{CmdResult, NoteSummary, default_folder, err, summary};
use crate::state::AppState;
use crate::userdata;

const ALL_DAY_KEY: &str = "dueAllDay";

pub fn due_all_day(n: &Note) -> bool {
    n.todo_due != 0
        && userdata::get(&n.user_data, ALL_DAY_KEY).and_then(|v| v.as_bool()) == Some(true)
}

fn set_due(n: &mut Note, due: i64, all_day: bool) {
    n.todo_due = due.max(0);
    n.user_data = userdata::set(
        &n.user_data,
        ALL_DAY_KEY,
        (all_day && due > 0).then_some(json!(true)),
    );
}

/// Creates a to-do titled `title` (in `parent_id`, or the default notebook).
#[tauri::command]
pub fn create_task(
    state: State<'_, AppState>,
    title: String,
    parent_id: Option<String>,
    due: Option<i64>,
    all_day: Option<bool>,
) -> CmdResult<NoteSummary> {
    let title = title.trim();
    if title.is_empty() {
        return Err(tr("タスクの名前を入力してください", "Enter what to do").into());
    }
    let db = state.db();
    let parent = match parent_id.filter(|p| !p.is_empty()) {
        Some(p) => p,
        None => default_folder(&db)?,
    };
    let mut n = Note::new(&parent, title, "");
    n.is_todo = 1;
    set_due(&mut n, due.unwrap_or(0), all_day.unwrap_or(false));
    db.save(&mut n).map_err(err)?;
    Ok(summary(&n))
}

/// Sets when a to-do is due (`due` 0 removes the due date).
#[tauri::command]
pub fn set_todo_due(
    state: State<'_, AppState>,
    id: String,
    due: i64,
    all_day: bool,
) -> CmdResult<()> {
    let db = state.db();
    let mut n = db
        .load::<Note>(&id)
        .map_err(err)?
        .ok_or_else(|| tr("ノートが見つかりません", "Note not found").to_string())?;
    n.is_todo = 1;
    set_due(&mut n, due, all_day);
    db.save(&mut n).map_err(err)
}
