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
    // A notebook that is gone (deleted or in the trash since it was chosen)
    // gives way to the default one, rather than hiding the task.
    let parent = match parent_id.filter(|p| !p.is_empty()) {
        Some(p)
            if db
                .load::<nisshoku_core::Folder>(&p)
                .map_err(err)?
                .is_some_and(|f| f.deleted_time == 0) =>
        {
            p
        }
        _ => default_folder(&db)?,
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
    db.save_meta(&mut n).map_err(err)
}

/// Notifies to-dos as they fall due (every minute; at 9:00 for all-day
/// ones, which are kept at that hour). The preference `taskReminders`
/// (default on) turns it off. Each to-do is notified once per due time.
pub async fn remind_due<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    use tauri::Manager;
    use tauri_plugin_notification::NotificationExt;
    const SEEN: &str = "app.tasks.notified";
    let state = app.state::<crate::state::AppState>();
    // Due while the app was closed: only what fell due in the last hour.
    let mut since = nisshoku_core::time::now_ms() - 3_600_000;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        let now = nisshoku_core::time::now_ms();
        let due: Vec<(String, String, i64)> = {
            let db = state.db();
            let on = crate::notes::preferences(&db)
                .get("taskReminders")
                .and_then(|v| v.as_bool())
                != Some(false);
            if !on {
                since = now;
                continue;
            }
            let seen: std::collections::HashMap<String, i64> = db
                .setting(SEEN)
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            match db.query::<Note, _>(
                "is_todo = 1 AND todo_completed = 0 AND deleted_time = 0 AND is_conflict = 0 \
                 AND todo_due > ?1 AND todo_due <= ?2",
                [since, now],
            ) {
                Ok(list) => list
                    .into_iter()
                    .filter(|n| seen.get(&n.id) != Some(&n.todo_due))
                    .map(|n| (n.id, n.title, n.todo_due))
                    .collect(),
                Err(e) => {
                    tracing::warn!("could not look for due tasks: {e}");
                    Vec::new()
                }
            }
        };
        since = now;
        if due.is_empty() {
            continue;
        }
        for (_, title, _) in &due {
            let title = if title.is_empty() { tr("無題のタスク", "Untitled task") } else { title.as_str() };
            if let Err(e) = app
                .notification()
                .builder()
                .title(tr("期限になりました", "Due now"))
                .body(title)
                .show()
            {
                tracing::warn!("could not show a notification: {e}");
            }
        }
        let db = state.db();
        let mut seen: std::collections::HashMap<String, i64> = db
            .setting(SEEN)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        for (id, _, t) in due {
            seen.insert(id, t);
        }
        // Forget what is long past.
        seen.retain(|_, t| *t > now - 30 * 86_400_000);
        if let Ok(json) = serde_json::to_string(&seen) {
            let _ = db.set_setting(SEEN, &json);
        }
    }
}
