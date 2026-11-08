//! Data for the home-screen widgets (Android).
//!
//! The app writes a snapshot of what the widgets show (`widgets.json` in its
//! data directory) and the widgets render it; they never open the database.
//! What a widget does by itself (completing a task) is left as one file per
//! action in `widget-actions/`, which the app applies the next time it runs
//! (and at once when it is open). The activity watches the snapshot and
//! refreshes the widgets when it changes: see `MainActivity.kt` and
//! `Widgets.kt`.

use std::collections::HashMap;
use std::path::Path;

use nisshoku_core::{Database, Folder, Note, time::now_ms};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::notes::{CmdResult, err, preferences};
use crate::state::AppState;

pub const SNAPSHOT_FILE: &str = "widgets.json";
pub const ACTIONS_DIR: &str = "widget-actions";
/// Tasks listed in the snapshot (the widget shows as many as fit).
const MAX_TASKS: usize = 20;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The day the snapshot was made (YYYY-MM-DD): on a later day, the
    /// widgets treat today's figures as zero.
    date: String,
    /// `ja` or `en`; `None` follows the system.
    language: Option<String>,
    /// The app's theme: `light`, `dark` or `system`.
    theme: String,
    /// The app's accent colour: `orange`, `vermilion`, `indigo`, `green` or `sumi`.
    accent: String,
    tasks: Vec<WidgetTask>,
    open_tasks: usize,
    writing: Writing,
    diary: Diary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetTask {
    id: String,
    title: String,
    due: i64,
    all_day: bool,
    notebook: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Writing {
    today: i64,
    goal: i64,
    streak: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diary {
    note_id: Option<String>,
    chars: usize,
}

/// Open to-dos in the order the widget lists them: by due date, then those
/// without one (the most recently ordered first).
fn open_tasks(db: &Database) -> CmdResult<Vec<Note>> {
    let mut tasks = db
        .query::<Note, _>(
            "deleted_time = 0 AND is_conflict = 0 AND is_todo = 1 AND todo_completed = 0",
            [],
        )
        .map_err(err)?;
    tasks.sort_by(|a, b| match (a.todo_due, b.todo_due) {
        (0, 0) => b
            .order
            .total_cmp(&a.order)
            .then(b.created_time.cmp(&a.created_time)),
        (0, _) => std::cmp::Ordering::Greater,
        (_, 0) => std::cmp::Ordering::Less,
        (x, y) => x.cmp(&y),
    });
    Ok(tasks)
}

pub fn snapshot(db: &Database) -> CmdResult<Snapshot> {
    let folders: HashMap<String, String> = db
        .all::<Folder>()
        .map_err(err)?
        .into_iter()
        .map(|f| (f.id, f.title))
        .collect();
    let tasks = open_tasks(db)?;
    let prefs = preferences(db);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let (written, streak) = crate::notes::today_and_streak(db);
    let diary = crate::diary::entry_on(db, &today);
    Ok(Snapshot {
        language: prefs
            .get("language")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        theme: prefs
            .get("theme")
            .and_then(|v| v.as_str())
            .unwrap_or("system")
            .to_string(),
        accent: prefs
            .get("accent")
            .and_then(|v| v.as_str())
            .unwrap_or("orange")
            .to_string(),
        open_tasks: tasks.len(),
        tasks: tasks
            .iter()
            .take(MAX_TASKS)
            .map(|n| WidgetTask {
                id: n.id.clone(),
                title: n.title.clone(),
                due: n.todo_due,
                all_day: crate::tasks::due_all_day(n),
                notebook: folders.get(&n.parent_id).cloned().unwrap_or_default(),
            })
            .collect(),
        writing: Writing {
            today: written,
            goal: prefs.get("dailyGoal").and_then(|v| v.as_i64()).unwrap_or(0),
            streak,
        },
        diary: Diary {
            chars: diary.as_ref().map_or(0, |d| d.1),
            note_id: diary.map(|d| d.0),
        },
        date: today,
    })
}

/// Writes the snapshot (atomically: the widgets may read it at any time).
pub fn write(dir: &Path, db: &Database) -> CmdResult<()> {
    let json = serde_json::to_vec(&snapshot(db)?).map_err(err)?;
    let tmp = dir.join(format!("{SNAPSHOT_FILE}.tmp"));
    std::fs::write(&tmp, json).map_err(err)?;
    std::fs::rename(&tmp, dir.join(SNAPSHOT_FILE)).map_err(err)
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Action {
    /// A task ticked off in the widget, at `time` (Unix ms).
    Complete { id: String, time: i64 },
}

/// Applies the actions left by the widgets, oldest first, and removes them.
/// Returns how many changed a note.
pub fn apply_actions(dir: &Path, db: &Database) -> CmdResult<usize> {
    let Ok(entries) = std::fs::read_dir(dir.join(ACTIONS_DIR)) else {
        return Ok(0);
    };
    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let mut changed = 0;
    for file in files {
        let action = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice::<Action>(&b).ok());
        match action {
            Some(Action::Complete { id, time }) => {
                if let Some(mut n) = db.load::<Note>(&id).map_err(err)?
                    && n.is_todo != 0
                    && n.todo_completed == 0
                    && n.deleted_time == 0
                {
                    n.todo_completed = if time > 0 { time } else { now_ms() };
                    db.save(&mut n).map_err(err)?;
                    changed += 1;
                }
            }
            None => tracing::warn!("ignoring an unreadable widget action: {}", file.display()),
        }
        let _ = std::fs::remove_file(&file);
    }
    Ok(changed)
}

/// Rewrites the widgets' snapshot (when the app goes to the background, and
/// after changes the widgets show). Does nothing where there are no widgets.
#[tauri::command]
pub fn refresh_widgets(state: State<'_, AppState>) -> CmdResult<()> {
    if !cfg!(target_os = "android") {
        return Ok(());
    }
    write(&state.data_dir, &state.db())
}

/// Applies what was done in the widgets; returns how many notes changed.
#[tauri::command]
pub fn apply_widget_actions(state: State<'_, AppState>) -> CmdResult<usize> {
    if !cfg!(target_os = "android") {
        return Ok(0);
    }
    let db = state.db();
    let changed = apply_actions(&state.data_dir, &db)?;
    write(&state.data_dir, &db)?;
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(db: &Database, parent: &str, title: &str, due: i64) -> Note {
        let mut n = Note::new(parent, title, "");
        n.is_todo = 1;
        n.todo_due = due;
        db.save(&mut n).unwrap();
        n
    }

    #[test]
    fn lists_open_tasks_by_due_date_and_applies_completions() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path().join("db.sqlite")).unwrap();
        let mut folder = Folder::new("", "Inbox");
        db.save(&mut folder).unwrap();
        let later = task(&db, &folder.id, "later", 2_000);
        let undated = task(&db, &folder.id, "undated", 0);
        let soon = task(&db, &folder.id, "soon", 1_000);
        let mut done = task(&db, &folder.id, "done", 500);
        done.todo_completed = 10;
        db.save(&mut done).unwrap();

        let snap = snapshot(&db).unwrap();
        let titles: Vec<&str> = snap.tasks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["soon", "later", "undated"]);
        assert_eq!(snap.open_tasks, 3);
        assert_eq!(snap.tasks[0].notebook, "Inbox");

        let actions = dir.path().join(ACTIONS_DIR);
        std::fs::create_dir_all(&actions).unwrap();
        std::fs::write(
            actions.join("1.json"),
            format!(r#"{{"action":"complete","id":"{}","time":1234}}"#, soon.id),
        )
        .unwrap();
        std::fs::write(actions.join("2.json"), "not json").unwrap();
        assert_eq!(apply_actions(dir.path(), &db).unwrap(), 1);
        assert_eq!(
            db.load::<Note>(&soon.id).unwrap().unwrap().todo_completed,
            1234
        );
        assert_eq!(std::fs::read_dir(&actions).unwrap().count(), 0);

        write(dir.path(), &db).unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join(SNAPSHOT_FILE)).unwrap())
                .unwrap();
        assert_eq!(json["openTasks"], 2);
        assert_eq!(json["tasks"][0]["id"], later.id);
        assert_eq!(json["tasks"][1]["id"], undated.id);
    }
}
