//! Commands for notebooks, notes, the trash, the diary, note history and
//! writing statistics.

use std::collections::{BTreeMap, HashMap};

use nisshoku_core::{Database, Folder, ItemType, Note, time::now_ms};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::State;

use crate::i18n::tr;
use crate::state::AppState;
use crate::text::{count_chars, note_excerpt};
use crate::userdata;

pub type CmdResult<T> = Result<T, String>;

pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

const DIARY_KEY: &str = "diary";
const DIARY_DATE_KEY: &str = "diaryDate";
const GOAL_KEY: &str = "goal";
const STATS_SETTING: &str = "app.stats.daily";
const PREFERENCES_SETTING: &str = "app.preferences";

// -------------------------------------------------------------------------
// Data sent to the window

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderDto {
    id: String,
    title: String,
    parent_id: String,
    icon: String,
    is_diary: bool,
    is_work: bool,
    note_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    id: String,
    parent_id: String,
    title: String,
    excerpt: String,
    pub(crate) char_count: usize,
    created_time: i64,
    updated_time: i64,
    is_conflict: bool,
    deleted_time: i64,
    diary_date: Option<String>,
    goal: Option<i64>,
    pinned: bool,
    is_todo: bool,
    todo_completed: bool,
    /// When a to-do is due (Unix ms), 0 if it has no due date.
    todo_due: i64,
    /// The due date is a day, without a time (see `tasks`).
    due_all_day: bool,
    /// When a to-do was completed (Unix ms), 0 if open.
    completed_time: i64,
    order: f64,
    /// Titles of the note's tags (filled in by lists).
    tags: Vec<String>,
    /// In a work: `character`, `plot`, `material`, or none (manuscript).
    kind: Option<String>,
    /// Set by `save_note` when the note had been changed elsewhere since the
    /// editor loaded it: that version was kept as this conflict copy.
    #[serde(skip_serializing_if = "Option::is_none")]
    conflict_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    id: String,
    parent_id: String,
    title: String,
    body: String,
    created_time: i64,
    updated_time: i64,
    is_conflict: bool,
    /// For a conflict copy, the note it is a copy of.
    conflict_original_id: String,
    deleted_time: i64,
    diary_date: Option<String>,
    goal: Option<i64>,
    pinned: bool,
    is_todo: bool,
    todo_completed: bool,
    todo_due: i64,
    due_all_day: bool,
    kind: Option<String>,
}

/// The day of a diary entry: recorded by this app, or read from a title
/// like `2026-11-04`, `2026/11/4` or `2026年11月4日` (entries written elsewhere).
pub(crate) fn diary_date(n: &Note) -> Option<String> {
    userdata::get(&n.user_data, DIARY_DATE_KEY)
        .and_then(|v| v.as_str().map(str::to_string))
        .or_else(|| date_in_title(&n.title))
}

fn leading_number(s: &str) -> Option<(u32, &str)> {
    let n = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if n == 0 {
        return None;
    }
    Some((s[..n].parse().ok()?, &s[n..]))
}

fn date_in_title(title: &str) -> Option<String> {
    let (y, rest) = leading_number(title.trim_start())?;
    let (m, rest) = leading_number(rest.strip_prefix(['-', '/', '.', '年'])?)?;
    let (d, _) = leading_number(rest.strip_prefix(['-', '/', '.', '月'])?)?;
    if !(1900..=2200).contains(&y) {
        return None;
    }
    chrono::NaiveDate::from_ymd_opt(y as i32, m, d).map(|d| d.format("%Y-%m-%d").to_string())
}

fn goal(n: &Note) -> Option<i64> {
    userdata::get(&n.user_data, GOAL_KEY).and_then(|v| v.as_i64())
}

pub(crate) fn summary(n: &Note) -> NoteSummary {
    NoteSummary {
        id: n.id.clone(),
        parent_id: n.parent_id.clone(),
        title: n.title.clone(),
        excerpt: note_excerpt(&n.title, &n.body, 90),
        char_count: count_chars(&n.body),
        created_time: n.user_created_time.max(n.created_time),
        updated_time: n.user_updated_time.max(n.updated_time),
        is_conflict: n.is_conflict != 0,
        deleted_time: n.deleted_time,
        diary_date: diary_date(n),
        goal: goal(n),
        pinned: is_pinned(n),
        is_todo: n.is_todo != 0,
        todo_completed: n.todo_completed != 0,
        todo_due: n.todo_due,
        due_all_day: crate::tasks::due_all_day(n),
        completed_time: n.todo_completed,
        order: n.order,
        tags: Vec::new(),
        kind: crate::works::kind_of(n),
        conflict_id: None,
    }
}

pub fn is_pinned(n: &Note) -> bool {
    userdata::get(&n.user_data, "pinned").and_then(|v| v.as_bool()) == Some(true)
}

pub fn is_template(n: &Note) -> bool {
    userdata::get(&n.user_data, "template").and_then(|v| v.as_bool()) == Some(true)
}

pub(crate) fn dto(n: &Note) -> NoteDto {
    NoteDto {
        id: n.id.clone(),
        parent_id: n.parent_id.clone(),
        title: n.title.clone(),
        body: n.body.clone(),
        created_time: n.user_created_time.max(n.created_time),
        updated_time: n.user_updated_time.max(n.updated_time),
        is_conflict: n.is_conflict != 0,
        conflict_original_id: n.conflict_original_id.clone(),
        deleted_time: n.deleted_time,
        diary_date: diary_date(n),
        goal: goal(n),
        pinned: is_pinned(n),
        is_todo: n.is_todo != 0,
        todo_completed: n.todo_completed != 0,
        todo_due: n.todo_due,
        due_all_day: crate::tasks::due_all_day(n),
        kind: crate::works::kind_of(n),
    }
}

/// The emoji of a notebook icon, stored as Joplin does:
/// `{"emoji":"📘","name":"","dataUrl":"","type":1}`.
fn icon_emoji(icon: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(icon).ok()?;
    v.get("emoji")?
        .as_str()
        .filter(|e| !e.is_empty())
        .map(str::to_string)
}

fn is_diary(f: &Folder) -> bool {
    userdata::get(&f.user_data, DIARY_KEY).and_then(|v| v.as_bool()) == Some(true)
}

pub(crate) fn load_note(db: &Database, id: &str) -> CmdResult<Note> {
    db.load::<Note>(id)
        .map_err(err)?
        .ok_or_else(|| tr("ノートが見つかりません", "Note not found").to_string())
}

pub(crate) fn load_folder(db: &Database, id: &str) -> CmdResult<Folder> {
    db.load::<Folder>(id)
        .map_err(err)?
        .ok_or_else(|| tr("ノートブックが見つかりません", "Notebook not found").to_string())
}

// -------------------------------------------------------------------------
// Notebooks

#[tauri::command]
pub fn list_folders(state: State<'_, AppState>) -> CmdResult<Vec<FolderDto>> {
    let db = state.db();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for n in db
        .query::<Note, _>("deleted_time = 0 AND is_conflict = 0", [])
        .map_err(err)?
    {
        *counts.entry(n.parent_id).or_default() += 1;
    }
    let mut folders: Vec<Folder> = db.query::<Folder, _>("deleted_time = 0", []).map_err(err)?;
    folders.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(folders
        .iter()
        .map(|f| FolderDto {
            id: f.id.clone(),
            title: f.title.clone(),
            parent_id: f.parent_id.clone(),
            icon: icon_emoji(&f.icon).unwrap_or_default(),
            is_diary: is_diary(f),
            is_work: crate::works::is_work(f),
            note_count: counts.get(&f.id).copied().unwrap_or(0),
        })
        .collect())
}

#[tauri::command]
pub fn create_folder(
    state: State<'_, AppState>,
    title: String,
    parent_id: Option<String>,
) -> CmdResult<String> {
    let db = state.db();
    let mut f = Folder::new(parent_id.as_deref().unwrap_or(""), title.trim());
    db.save(&mut f).map_err(err)?;
    Ok(f.id)
}

#[tauri::command]
pub fn rename_folder(state: State<'_, AppState>, id: String, title: String) -> CmdResult<()> {
    let db = state.db();
    let mut f = load_folder(&db, &id)?;
    f.title = title.trim().to_string();
    db.save(&mut f).map_err(err)
}

#[tauri::command]
pub fn set_folder_icon(
    state: State<'_, AppState>,
    id: String,
    emoji: Option<String>,
) -> CmdResult<()> {
    let db = state.db();
    let mut f = load_folder(&db, &id)?;
    f.icon = match emoji.filter(|e| !e.trim().is_empty()) {
        Some(e) => json!({ "emoji": e.trim(), "name": "", "dataUrl": "", "type": 1 }).to_string(),
        None => String::new(),
    };
    db.save_meta(&mut f).map_err(err)
}

#[tauri::command]
pub fn move_folder(state: State<'_, AppState>, id: String, parent_id: String) -> CmdResult<()> {
    let db = state.db();
    // A folder cannot go inside itself.
    if !parent_id.is_empty() {
        load_folder(&db, &parent_id)?;
    }
    let mut p = parent_id.clone();
    let mut seen = std::collections::HashSet::new();
    // (`seen` ends a loop already in the data, which would otherwise hang here.)
    while !p.is_empty() && seen.insert(p.clone()) {
        if p == id {
            return Err(tr(
                "ノートブックをその中のノートブックには移動できません",
                "A notebook cannot be moved into itself",
            )
            .into());
        }
        p = db
            .load::<Folder>(&p)
            .map_err(err)?
            .map(|f| f.parent_id)
            .unwrap_or_default();
    }
    let mut f = load_folder(&db, &id)?;
    f.parent_id = parent_id;
    db.save_meta(&mut f).map_err(err)
}

/// Moves a notebook, its sub-notebooks and their notes to the trash.
#[tauri::command]
pub fn trash_folder(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = state.db();
    let now = now_ms();
    let mut pending = vec![id];
    while let Some(fid) = pending.pop() {
        for n in db
            .query::<Note, _>("parent_id = ?1 AND deleted_time = 0", [&fid])
            .map_err(err)?
        {
            let mut n = n;
            n.deleted_time = now;
            db.save_meta(&mut n).map_err(err)?;
        }
        for f in db
            .query::<Folder, _>("parent_id = ?1 AND deleted_time = 0", [&fid])
            .map_err(err)?
        {
            pending.push(f.id);
        }
        let mut f = load_folder(&db, &fid)?;
        f.deleted_time = now;
        db.save_meta(&mut f).map_err(err)?;
    }
    Ok(())
}

// -------------------------------------------------------------------------
// Notes

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NoteScope {
    All,
    Folder { id: String },
    Diary,
    Trash,
    Conflicts,
    Search { query: String },
    Tag { id: String },
    Pinned,
    Todo,
}

#[tauri::command]
pub fn list_notes(state: State<'_, AppState>, scope: NoteScope) -> CmdResult<Vec<NoteSummary>> {
    let db = state.db();
    let mut notes = match &scope {
        NoteScope::All => {
            // Diary entries have their own place.
            let diary: std::collections::HashSet<String> =
                diary_folders(&db)?.into_iter().map(|f| f.id).collect();
            db.query::<Note, _>("deleted_time = 0 AND is_conflict = 0", [])
                .map(|v| {
                    v.into_iter()
                        .filter(|n| !is_template(n) && !diary.contains(&n.parent_id))
                        .collect()
                })
        }
        NoteScope::Tag { id } => {
            let ids: std::collections::HashSet<String> = db
                .query::<nisshoku_core::NoteTag, _>("tag_id = ?1", [id])
                .map_err(err)?
                .into_iter()
                .map(|nt| nt.note_id)
                .collect();
            db.query::<Note, _>("deleted_time = 0 AND is_conflict = 0", [])
                .map(|v| v.into_iter().filter(|n| ids.contains(&n.id) && !is_template(n)).collect())
        }
        NoteScope::Pinned => db
            .query::<Note, _>("deleted_time = 0 AND is_conflict = 0", [])
            .map(|v| v.into_iter().filter(|n| is_pinned(n) && !is_template(n)).collect()),
        NoteScope::Todo => {
            db.query::<Note, _>("deleted_time = 0 AND is_conflict = 0 AND is_todo = 1", [])
        }
        NoteScope::Folder { id } => db.query::<Note, _>(
            "parent_id = ?1 AND deleted_time = 0 AND is_conflict = 0",
            [id],
        ),
        NoteScope::Diary => {
            let ids: Vec<String> = diary_folders(&db)?.into_iter().map(|f| f.id).collect();
            let mut out = Vec::new();
            for id in ids {
                out.extend(
                    db.query::<Note, _>(
                        "parent_id = ?1 AND deleted_time = 0 AND is_conflict = 0",
                        [&id],
                    )
                    .map_err(err)?,
                );
            }
            Ok(out)
        }
        NoteScope::Trash => db.query::<Note, _>("deleted_time > 0", []),
        NoteScope::Conflicts => db.query::<Note, _>("is_conflict = 1 AND deleted_time = 0", []),
        NoteScope::Search { query } => {
            // `tag:名前` and `in:ノートブック` narrow down; other words are searched for.
            use crate::text::{matches_query, search_fold};
            let mut words = Vec::new();
            let (mut tag_terms, mut folder_terms) = (Vec::new(), Vec::new());
            for w in query.split_whitespace() {
                let lower = w.to_lowercase();
                if let Some(t) = lower.strip_prefix("tag:").or_else(|| w.strip_prefix("タグ:")) {
                    tag_terms.push(search_fold(t));
                } else if let Some(f) = lower.strip_prefix("in:").or_else(|| w.strip_prefix("場所:")) {
                    folder_terms.push(search_fold(f));
                } else {
                    words.push(w);
                }
            }
            let words = words.join(" ");
            if words.trim().is_empty() && tag_terms.is_empty() && folder_terms.is_empty() {
                return Ok(Vec::new());
            }
            let tags = crate::features::tag_titles_by_note(&db)?;
            let folders: HashMap<String, String> = db
                .all::<Folder>()
                .map_err(err)?
                .into_iter()
                .map(|f| (f.id, search_fold(&f.title)))
                .collect();
            let all = db.query::<Note, _>("deleted_time = 0", []).map_err(err)?;
            Ok(all
                .into_iter()
                .filter(|n| {
                    let note_tags: Vec<String> = tags.get(&n.id).map(|v| v.iter().map(|t| search_fold(t)).collect()).unwrap_or_default();
                    tag_terms.iter().all(|t| note_tags.iter().any(|x| x.contains(t.as_str())))
                        && folder_terms.iter().all(|f| folders.get(&n.parent_id).is_some_and(|x| x.contains(f.as_str())))
                        && matches_query(&format!("{}\n{}", n.title, n.body), &words)
                })
                .collect())
        }
    }
    .map_err(err)?;
    let tags = crate::features::tag_titles_by_note(&db)?;
    let mut out: Vec<NoteSummary> = notes
        .iter_mut()
        .map(|n| {
            let mut s = summary(n);
            s.tags = tags.get(&n.id).cloned().unwrap_or_default();
            s
        })
        .collect();
    match scope {
        NoteScope::Diary => out.sort_by(|a, b| {
            (b.diary_date.clone(), b.created_time).cmp(&(a.diary_date.clone(), a.created_time))
        }),
        NoteScope::Trash => out.sort_by_key(|n| -n.deleted_time),
        _ => out.sort_by_key(|n| -n.updated_time),
    }
    Ok(out)
}

#[tauri::command]
pub fn get_note(state: State<'_, AppState>, id: String) -> CmdResult<NoteDto> {
    Ok(dto(&load_note(&state.db(), &id)?))
}

/// The notebook new notes go to when none is chosen.
pub(crate) fn default_folder(db: &Database) -> CmdResult<String> {
    let mut folders: Vec<Folder> = db
        .query::<Folder, _>("deleted_time = 0", [])
        .map_err(err)?
        .into_iter()
        .filter(|f| !is_diary(f))
        .collect();
    // The notebook of the default name if there is one (not whichever was
    // made first: new notes would land in the first novel), else the oldest.
    let name = preference_str(db, "defaultNotebookName").unwrap_or_else(|| "Notes".into());
    folders.sort_by_key(|f| (f.title.trim() != name.trim(), f.created_time));
    if let Some(f) = folders.first() {
        return Ok(f.id.clone());
    }
    let mut f = Folder::new("", &name);
    db.save(&mut f).map_err(err)?;
    Ok(f.id)
}

#[tauri::command]
pub fn create_note(
    state: State<'_, AppState>,
    parent_id: Option<String>,
    title: Option<String>,
    body: Option<String>,
) -> CmdResult<NoteDto> {
    let db = state.db();
    let parent = match parent_id.filter(|p| !p.is_empty()) {
        Some(p) => p,
        None => default_folder(&db)?,
    };
    let mut n = Note::new(
        &parent,
        title.as_deref().unwrap_or(""),
        body.as_deref().unwrap_or(""),
    );
    db.save(&mut n).map_err(err)?;
    Ok(dto(&n))
}

/// Saves the title and body of a note (with the history rules), and counts
/// what was written toward today's statistics.
///
/// `base_title` / `base_body` are the text the editor started from. If the
/// note now holds another text (a sync brought an edit made on another
/// device meanwhile), that version is kept as a conflict copy rather than
/// overwritten unseen.
#[tauri::command]
pub fn save_note(
    state: State<'_, AppState>,
    id: String,
    title: String,
    body: String,
    base_title: Option<String>,
    base_body: Option<String>,
) -> CmdResult<NoteSummary> {
    let db = state.db();
    let mut n = load_note(&db, &id)?;
    if n.title == title && n.body == body {
        return Ok(summary(&n));
    }
    let changed_elsewhere = match (&base_title, &base_body) {
        (Some(t), Some(b)) => (&n.title, &n.body) != (t, b),
        _ => false,
    };
    let conflict_id = if changed_elsewhere && n.deleted_time == 0 {
        let mut copy = n.clone();
        copy.id = String::new();
        copy.is_conflict = 1;
        copy.conflict_original_id = n.id.clone();
        db.save(&mut copy).map_err(err)?;
        Some(copy.id)
    } else {
        None
    };
    let before = count_chars(&n.body);
    n.title = title;
    n.body = body;
    db.save_note(&mut n).map_err(err)?;
    let after = count_chars(&n.body);
    if after > before {
        record_written(&db, (after - before) as i64)?;
    }
    let mut out = summary(&n);
    out.conflict_id = conflict_id;
    Ok(out)
}

#[tauri::command]
pub fn move_note(state: State<'_, AppState>, id: String, parent_id: String) -> CmdResult<()> {
    let db = state.db();
    load_folder(&db, &parent_id)?;
    let mut n = load_note(&db, &id)?;
    n.parent_id = parent_id;
    db.save_meta(&mut n).map_err(err)
}

#[tauri::command]
pub fn set_note_goal(state: State<'_, AppState>, id: String, goal: Option<i64>) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &id)?;
    n.user_data = userdata::set(
        &n.user_data,
        GOAL_KEY,
        goal.filter(|g| *g > 0).map(|g| json!(g)),
    );
    db.save_meta(&mut n).map_err(err)
}

#[tauri::command]
pub fn duplicate_note(state: State<'_, AppState>, id: String) -> CmdResult<NoteDto> {
    let db = state.db();
    let src = load_note(&db, &id)?;
    let mut n = Note::new(
        &src.parent_id,
        &format!("{}{}", src.title, tr("（コピー）", " (copy)")),
        &src.body,
    );
    // What the note is (a to-do, a chapter or character of a work, its goal…)
    // comes along; comments on the original and its day in the diary do not.
    n.is_todo = src.is_todo;
    n.todo_due = src.todo_due;
    let mut user_data = src.user_data.clone();
    for key in [COMMENTS_KEY, DIARY_DATE_KEY, "pinned", "memo"] {
        if userdata::get(&user_data, key).is_some() {
            user_data = userdata::set(&user_data, key, None);
        }
    }
    n.user_data = user_data;
    db.save(&mut n).map_err(err)?;
    for nt in db
        .query::<nisshoku_core::NoteTag, _>("note_id = ?1", [&src.id])
        .map_err(err)?
    {
        let mut link = nisshoku_core::NoteTag {
            note_id: n.id.clone(),
            tag_id: nt.tag_id,
            ..Default::default()
        };
        db.save(&mut link).map_err(err)?;
    }
    Ok(dto(&n))
}

// -------------------------------------------------------------------------
// Trash

#[tauri::command]
pub fn trash_note(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &id)?;
    n.deleted_time = now_ms();
    db.save_meta(&mut n).map_err(err)
}

#[tauri::command]
pub fn restore_note(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &id)?;
    // Bring back the notebooks it was in, or put it in the default one.
    let mut parent = n.parent_id.clone();
    while !parent.is_empty() {
        match db.load::<Folder>(&parent).map_err(err)? {
            Some(mut f) => {
                if f.deleted_time != 0 {
                    f.deleted_time = 0;
                    db.save_meta(&mut f).map_err(err)?;
                }
                parent = f.parent_id.clone();
            }
            None => {
                n.parent_id = default_folder(&db)?;
                break;
            }
        }
    }
    n.deleted_time = 0;
    db.save_meta(&mut n).map_err(err)
}

#[tauri::command]
pub fn delete_note_forever(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.db().delete(ItemType::Note, &id).map_err(err)
}

#[tauri::command]
pub fn empty_trash(state: State<'_, AppState>) -> CmdResult<usize> {
    let db = state.db();
    let notes = db.query::<Note, _>("deleted_time > 0", []).map_err(err)?;
    for n in &notes {
        db.delete(ItemType::Note, &n.id).map_err(err)?;
    }
    // Deepest notebooks first.
    let mut folders = db.query::<Folder, _>("deleted_time > 0", []).map_err(err)?;
    folders.sort_by_key(|f| std::cmp::Reverse(db.folder_depth(&f.id).unwrap_or(0)));
    for f in &folders {
        if !db.folder_has_children(&f.id).map_err(err)? {
            db.delete(ItemType::Folder, &f.id).map_err(err)?;
        }
    }
    Ok(notes.len())
}

/// Deletes what has been in the trash for more than `days` days.
pub fn purge_trash(db: &Database, days: i64) -> CmdResult<usize> {
    if days <= 0 {
        return Ok(0);
    }
    let cutoff = now_ms() - days * 86_400_000;
    let notes = db
        .query::<Note, _>("deleted_time > 0 AND deleted_time < ?1", [cutoff])
        .map_err(err)?;
    for n in &notes {
        db.delete(ItemType::Note, &n.id).map_err(err)?;
    }
    let mut folders = db
        .query::<Folder, _>("deleted_time > 0 AND deleted_time < ?1", [cutoff])
        .map_err(err)?;
    folders.sort_by_key(|f| std::cmp::Reverse(db.folder_depth(&f.id).unwrap_or(0)));
    for f in &folders {
        if !db.folder_has_children(&f.id).map_err(err)? {
            db.delete(ItemType::Folder, &f.id).map_err(err)?;
        }
    }
    Ok(notes.len())
}

/// A file or folder name made of a title.
fn file_name(title: &str, fallback: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').to_string();
    let short: String = cleaned.chars().take(80).collect();
    let short = short.trim_end_matches([' ', '.']).to_string();
    if short.is_empty() {
        return fallback.to_string();
    }
    // Names Windows keeps for devices, with or without an extension.
    let stem = short.split('.').next().unwrap_or("").trim().to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if reserved { format!("{short}_") } else { short }
}

/// Writes every note as a Markdown file, in folders named after the
/// notebooks. Returns the number of notes written.
#[tauri::command]
pub fn export_all(state: State<'_, AppState>, dir: String) -> CmdResult<usize> {
    let state = &*state;
    let db = state.db();
    let folders: HashMap<String, Folder> = db
        .query::<Folder, _>("deleted_time = 0", [])
        .map_err(err)?
        .into_iter()
        .map(|f| (f.id.clone(), f))
        .collect();
    let folder_path = |id: &str| -> std::path::PathBuf {
        let mut parts = Vec::new();
        let mut cur = folders.get(id);
        while let Some(f) = cur {
            parts.push(file_name(&f.title, "Untitled"));
            if parts.len() > 32 {
                break;
            }
            cur = folders.get(&f.parent_id);
        }
        parts.iter().rev().collect()
    };
    let root = std::path::PathBuf::from(dir);
    let mut used = std::collections::HashSet::new();
    let notes = db
        .query::<Note, _>("deleted_time = 0 AND is_conflict = 0", [])
        .map_err(err)?;
    let resources: HashMap<String, nisshoku_core::Resource> = db
        .all::<nisshoku_core::Resource>()
        .map_err(err)?
        .into_iter()
        .map(|r| (r.id.clone(), r))
        .collect();
    let link = regex_lite_links();
    let mut written = 0;
    let mut failed = Vec::new();
    let mut copied = std::collections::HashSet::new();
    for n in &notes {
        let result = (|| -> std::io::Result<()> {
            let rel = folder_path(&n.parent_id);
            let folder = root.join(&rel);
            std::fs::create_dir_all(&folder)?;
            let base = file_name(&n.title, "Untitled");
            let mut path = folder.join(format!("{base}.md"));
            let mut i = 2;
            while !used.insert(path.clone()) || path.exists() {
                path = folder.join(format!("{base} ({i}).md"));
                i += 1;
            }
            // Attachments go to `_resources`, and their `:/id` links point there.
            let up = "../".repeat(rel.components().count());
            let body = link(&n.body, &mut |id| {
                let r = resources.get(id)?;
                let src = state.resource_dir.join(r.file_name());
                if !src.exists() {
                    return None;
                }
                let name = r.file_name();
                if copied.insert(id.to_string()) {
                    let dir = root.join("_resources");
                    std::fs::create_dir_all(&dir).ok()?;
                    std::fs::copy(&src, dir.join(&name)).ok()?;
                }
                Some(format!("{up}_resources/{name}"))
            });
            let text = if n.title.is_empty() {
                body
            } else {
                format!("# {}\n\n{}\n", n.title, body)
            };
            std::fs::write(&path, text)
        })();
        match result {
            Ok(()) => written += 1,
            Err(e) => failed.push(format!("{}: {e}", if n.title.is_empty() { &n.id } else { &n.title })),
        }
    }
    if !failed.is_empty() {
        tracing::warn!("could not export: {failed:?}");
        if written == 0 {
            return Err(failed.join("\n"));
        }
    }
    Ok(written)
}

/// Replaces the targets of `:/<id>` links and images where `f` gives one.
fn regex_lite_links() -> impl Fn(&str, &mut dyn FnMut(&str) -> Option<String>) -> String {
    |text, f| {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(i) = rest.find("(:/") {
            let after = &rest[i + 3..];
            let id_len = after.bytes().take_while(|b| b.is_ascii_hexdigit()).count();
            out.push_str(&rest[..i + 1]);
            if id_len == 32 && let Some(target) = f(&after[..32]) {
                out.push_str(&target);
                rest = &after[32..];
            } else {
                out.push_str(":/");
                rest = after;
            }
        }
        out.push_str(rest);
        out
    }
}

// -------------------------------------------------------------------------
// Diary

pub(crate) fn diary_folders(db: &Database) -> CmdResult<Vec<Folder>> {
    let mut folders: Vec<Folder> = db
        .query::<Folder, _>("deleted_time = 0", [])
        .map_err(err)?
        .into_iter()
        .filter(is_diary)
        .collect();
    // The same order on every device, so that merging duplicates (see
    // diary::merge_duplicates) keeps the same notebook everywhere.
    folders.sort_by(|a, b| (a.created_time, &a.id).cmp(&(b.created_time, &b.id)));
    Ok(folders)
}

pub(crate) fn ensure_diary_folder(db: &Database) -> CmdResult<Folder> {
    if let Some(f) = diary_folders(db)?.into_iter().next() {
        return Ok(f);
    }
    let name = preference_str(db, "diaryNotebookName").unwrap_or_else(|| "Diary".into());
    let mut f = Folder::new("", &name);
    f.user_data = userdata::set("", DIARY_KEY, Some(json!(true)));
    db.save(&mut f).map_err(err)?;
    Ok(f)
}

/// The title of a new diary entry, in the chosen format: `iso`
/// (2026-11-04), `isoWeekday` (2026-11-04 Wed) or `ja` (2026年11月4日（水）).
fn diary_title(date: chrono::NaiveDate, format: &str) -> String {
    use chrono::Datelike;
    const WEEKDAYS: [&str; 7] = ["月", "火", "水", "木", "金", "土", "日"];
    let weekday = date.weekday().num_days_from_monday() as usize;
    match format {
        "ja" => format!(
            "{}年{}月{}日（{}）",
            date.year(),
            date.month(),
            date.day(),
            WEEKDAYS[weekday]
        ),
        "isoWeekday" => date.format("%Y-%m-%d %a").to_string(),
        _ => date.format("%Y-%m-%d").to_string(),
    }
}

/// The diary entry of a day (`YYYY-MM-DD`), created if there is none.
#[tauri::command]
pub fn open_diary(state: State<'_, AppState>, date: String) -> CmdResult<NoteDto> {
    let day = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(err)?;
    let db = state.db();
    for f in diary_folders(&db)? {
        let notes = db
            .query::<Note, _>(
                "parent_id = ?1 AND deleted_time = 0 AND is_conflict = 0",
                [&f.id],
            )
            .map_err(err)?;
        if let Some(n) = notes
            .iter()
            .find(|n| diary_date(n).as_deref() == Some(date.as_str()))
        {
            return Ok(dto(n));
        }
    }
    let folder = ensure_diary_folder(&db)?;
    // Not trimmed: the template's blank lines are part of it.
    let template = preferences(&db)
        .get("diaryTemplate")
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let format = preference_str(&db, "diaryTitleFormat").unwrap_or_else(|| "iso".into());
    let mut n = Note::new(&folder.id, &diary_title(day, &format), &template);
    n.user_data = userdata::set("", DIARY_DATE_KEY, Some(json!(date)));
    db.save(&mut n).map_err(err)?;
    Ok(dto(&n))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryDay {
    date: String,
    note_id: String,
    char_count: usize,
    mood: Option<String>,
}

/// Days of a month (`YYYY-MM`) that have a diary entry.
#[tauri::command]
pub fn diary_month(state: State<'_, AppState>, month: String) -> CmdResult<Vec<DiaryDay>> {
    let db = state.db();
    let mut out = Vec::new();
    for f in diary_folders(&db)? {
        for n in db
            .query::<Note, _>(
                "parent_id = ?1 AND deleted_time = 0 AND is_conflict = 0",
                [&f.id],
            )
            .map_err(err)?
        {
            if let Some(d) = diary_date(&n).filter(|d| d.starts_with(&month)) {
                out.push(DiaryDay {
                    date: d,
                    note_id: n.id.clone(),
                    char_count: count_chars(&n.body),
                    mood: userdata::get(&n.user_data, "mood")
                        .and_then(|v| v.as_str().map(str::to_string)),
                });
            }
        }
    }
    out.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(out)
}

/// Consecutive days with a diary entry, up to today (or yesterday).
#[tauri::command]
pub fn diary_streak(state: State<'_, AppState>, today: String) -> CmdResult<usize> {
    let db = state.db();
    let mut dates = std::collections::HashSet::new();
    for f in diary_folders(&db)? {
        for n in db
            .query::<Note, _>("parent_id = ?1 AND deleted_time = 0 AND is_conflict = 0", [&f.id])
            .map_err(err)?
        {
            if let Some(d) = diary_date(&n) {
                dates.insert(d);
            }
        }
    }
    Ok(streak(&dates, &today))
}

fn streak(dates: &std::collections::HashSet<String>, today: &str) -> usize {
    let Ok(mut day) = chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d") else {
        return 0;
    };
    let key = |d: chrono::NaiveDate| d.format("%Y-%m-%d").to_string();
    if !dates.contains(&key(day)) {
        day = day.pred_opt().expect("date");
    }
    let mut n = 0;
    while dates.contains(&key(day)) {
        n += 1;
        day = day.pred_opt().expect("date");
    }
    n
}

// -------------------------------------------------------------------------
// History

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionDto {
    id: String,
    time: i64,
}

#[tauri::command]
pub fn list_revisions(state: State<'_, AppState>, note_id: String) -> CmdResult<Vec<RevisionDto>> {
    let db = state.db();
    let mut revs: Vec<RevisionDto> = db
        .revisions(&note_id)
        .map_err(err)?
        .into_iter()
        .map(|r| RevisionDto {
            id: r.id,
            time: r.item_updated_time,
        })
        .collect();
    revs.sort_by_key(|r| -r.time);
    Ok(revs)
}

#[derive(Serialize)]
pub struct RevisionContent {
    title: String,
    body: String,
}

#[tauri::command]
pub fn revision_content(
    state: State<'_, AppState>,
    note_id: String,
    revision_id: String,
) -> CmdResult<RevisionContent> {
    let n = state
        .db()
        .note_at_revision(&note_id, &revision_id)
        .map_err(err)?;
    Ok(RevisionContent {
        title: n.title,
        body: n.body,
    })
}

#[tauri::command]
pub fn restore_revision(
    state: State<'_, AppState>,
    note_id: String,
    revision_id: String,
) -> CmdResult<NoteDto> {
    let db = state.db();
    // (The current text goes into the history first.)
    let mut n = db.restore_revision(&note_id, &revision_id).map_err(err)?;
    if n.parent_id.is_empty() {
        n.parent_id = default_folder(&db)?;
        db.save(&mut n).map_err(err)?;
    }
    Ok(dto(&n))
}

/// Settles a conflict copy: `keep` = `copy` puts its text into the original
/// note (re-creating it if it is gone), `original` drops the copy. Returns
/// the note that remains.
#[tauri::command]
pub fn resolve_conflict(state: State<'_, AppState>, id: String, keep: String) -> CmdResult<String> {
    let db = state.db();
    let copy = load_note(&db, &id)?;
    if copy.is_conflict == 0 {
        return Err(tr("競合コピーではありません", "This is not a conflict copy").into());
    }
    let original = db
        .load::<Note>(&copy.conflict_original_id)
        .map_err(err)?
        .filter(|n| n.deleted_time == 0);
    match (keep.as_str(), original) {
        ("copy", Some(mut o)) => {
            o.title = copy.title.clone();
            o.body = copy.body.clone();
            db.save_note(&mut o).map_err(err)?;
            db.delete(ItemType::Note, &copy.id).map_err(err)?;
            Ok(o.id)
        }
        // The original is gone: the copy becomes an ordinary note.
        ("copy", None) | ("original", None) => {
            let mut n = copy;
            n.is_conflict = 0;
            n.conflict_original_id = String::new();
            if db.load::<Folder>(&n.parent_id).map_err(err)?.is_none() {
                n.parent_id = default_folder(&db)?;
            }
            db.save(&mut n).map_err(err)?;
            Ok(n.id)
        }
        ("original", Some(o)) => {
            db.delete(ItemType::Note, &copy.id).map_err(err)?;
            Ok(o.id)
        }
        _ => Err("keep: copy or original".into()),
    }
}

/// Removes a note's history (after locking it, its earlier text would stay
/// readable there).
#[tauri::command]
pub fn clear_history(state: State<'_, AppState>, note_id: String) -> CmdResult<usize> {
    state.db().delete_revisions(&note_id).map_err(err)
}

/// Keeps the history in the background: a version of each changed note every
/// ten minutes or so, and versions older than `historyDays` (preference,
/// 90 by default; 0 keeps them forever) removed at start.
pub async fn keep_history<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    use tauri::Manager;
    let state = app.state::<AppState>();
    {
        let db = state.db();
        let days = preferences(&db).get("historyDays").and_then(|v| v.as_i64()).unwrap_or(90);
        if days > 0
            && let Err(e) = db.delete_old_revisions(days * 86_400_000)
        {
            tracing::warn!("could not remove old history: {e}");
        }
    }
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
        let ids = match state.db().notes_needing_revision() {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!("could not look for notes to record: {e}");
                continue;
            }
        };
        for id in ids {
            // One note at a time: the window's commands get the database in between.
            {
                let db = state.db();
                if let Ok(Some(n)) = db.load::<Note>(&id)
                    && let Err(e) = db.create_revision(&n)
                {
                    tracing::warn!("could not record a version of {id}: {e}");
                }
            }
            tokio::task::yield_now().await;
        }
    }
}

/// Records the current text in the history now ("save a version").
#[tauri::command]
pub fn save_snapshot(state: State<'_, AppState>, note_id: String) -> CmdResult<bool> {
    let db = state.db();
    let n = load_note(&db, &note_id)?;
    Ok(db.create_revision(&n).map_err(err)?.is_some())
}

// -------------------------------------------------------------------------
// Statistics

/// Characters written today and the run of days with writing.
pub(crate) fn today_and_streak(db: &Database) -> (i64, usize) {
    let stats = daily_stats(db);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let written = stats
        .iter()
        .filter(|(_, v)| **v > 0)
        .map(|(k, _)| k.clone())
        .collect();
    (
        stats.get(&today).copied().unwrap_or(0),
        streak(&written, &today),
    )
}

fn daily_stats(db: &Database) -> BTreeMap<String, i64> {
    db.setting(STATS_SETTING)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn record_written(db: &Database, chars: i64) -> CmdResult<()> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut stats = daily_stats(db);
    *stats.entry(today).or_default() += chars;
    // Keep about two years.
    while stats.len() > 800 {
        let first = stats.keys().next().cloned().expect("not empty");
        stats.remove(&first);
    }
    db.set_setting(STATS_SETTING, &serde_json::to_string(&stats).map_err(err)?)
        .map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    today: i64,
    streak: usize,
    /// Characters written per day, oldest first (the last 30 days).
    days: Vec<(String, i64)>,
    /// Every recorded day (about two years), oldest first.
    history: Vec<(String, i64)>,
    total_notes: usize,
    total_chars: usize,
}

#[tauri::command]
pub fn get_stats(state: State<'_, AppState>) -> CmdResult<Stats> {
    let db = state.db();
    let stats = daily_stats(&db);
    let today = chrono::Local::now().date_naive();
    let key = |d: chrono::NaiveDate| d.format("%Y-%m-%d").to_string();
    let days: Vec<(String, i64)> = (0..30)
        .rev()
        .map(|i| {
            let d = key(today - chrono::Duration::days(i));
            let v = stats.get(&d).copied().unwrap_or(0);
            (d, v)
        })
        .collect();
    let written: std::collections::HashSet<String> = stats
        .iter()
        .filter(|(_, v)| **v > 0)
        .map(|(k, _)| k.clone())
        .collect();
    let notes = db
        .query::<Note, _>("deleted_time = 0 AND is_conflict = 0", [])
        .map_err(err)?;
    Ok(Stats {
        today: stats.get(&key(today)).copied().unwrap_or(0),
        streak: streak(&written, &key(today)),
        history: stats.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        days,
        total_notes: notes.len(),
        total_chars: notes.iter().map(|n| count_chars(&n.body)).sum(),
    })
}

// -------------------------------------------------------------------------
// Comments
//
// Notes on passages of a note (for revising), kept in the note's `user_data`
// so that they sync without changing the text. A comment finds its passage
// by the quoted text and what precedes it.

const COMMENTS_KEY: &str = "comments";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    id: String,
    /// The passage commented on.
    quote: String,
    /// A few characters before the passage, to tell repeated text apart.
    #[serde(default)]
    before: String,
    text: String,
    created_time: i64,
    #[serde(default)]
    updated_time: i64,
    #[serde(default)]
    resolved: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteComment {
    note_id: String,
    note_title: String,
    #[serde(flatten)]
    comment: Comment,
}

fn comments_of(n: &Note) -> Vec<Comment> {
    userdata::get(&n.user_data, COMMENTS_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn save_comments(db: &Database, n: &mut Note, comments: &[Comment]) -> CmdResult<()> {
    let value =
        (!comments.is_empty()).then(|| serde_json::to_value(comments).expect("serializable"));
    n.user_data = userdata::set(&n.user_data, COMMENTS_KEY, value);
    db.save_meta(n).map_err(err)
}

/// Comments of one note, or of all notes (not in the trash), newest first.
#[tauri::command]
pub fn list_comments(
    state: State<'_, AppState>,
    note_id: Option<String>,
) -> CmdResult<Vec<NoteComment>> {
    let db = state.db();
    let notes = match note_id {
        Some(id) => vec![load_note(&db, &id)?],
        None => db
            .query::<Note, _>("deleted_time = 0 AND user_data LIKE '%\"comments\"%'", [])
            .map_err(err)?,
    };
    let mut out: Vec<NoteComment> = notes
        .iter()
        .flat_map(|n| {
            comments_of(n).into_iter().map(|c| NoteComment {
                note_id: n.id.clone(),
                note_title: n.title.clone(),
                comment: c,
            })
        })
        .collect();
    out.sort_by_key(|c| -c.comment.created_time);
    Ok(out)
}

#[tauri::command]
pub fn add_comment(
    state: State<'_, AppState>,
    note_id: String,
    quote: String,
    before: String,
    text: String,
) -> CmdResult<Comment> {
    let db = state.db();
    let mut n = load_note(&db, &note_id)?;
    let mut comments = comments_of(&n);
    let now = now_ms();
    let comment = Comment {
        id: nisshoku_core::ids::new_id(),
        quote,
        before,
        text: text.trim().to_string(),
        created_time: now,
        updated_time: now,
        resolved: false,
    };
    comments.push(comment.clone());
    save_comments(&db, &mut n, &comments)?;
    Ok(comment)
}

#[tauri::command]
pub fn update_comment(
    state: State<'_, AppState>,
    note_id: String,
    comment_id: String,
    text: Option<String>,
    resolved: Option<bool>,
) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &note_id)?;
    let mut comments = comments_of(&n);
    let c = comments
        .iter_mut()
        .find(|c| c.id == comment_id)
        .ok_or_else(|| tr("コメントが見つかりません", "Comment not found").to_string())?;
    if let Some(t) = text {
        c.text = t.trim().to_string();
    }
    if let Some(r) = resolved {
        c.resolved = r;
    }
    c.updated_time = now_ms();
    save_comments(&db, &mut n, &comments)
}

#[tauri::command]
pub fn delete_comment(
    state: State<'_, AppState>,
    note_id: String,
    comment_id: String,
) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &note_id)?;
    let comments: Vec<Comment> = comments_of(&n)
        .into_iter()
        .filter(|c| c.id != comment_id)
        .collect();
    save_comments(&db, &mut n, &comments)
}

// -------------------------------------------------------------------------
// Preferences and files

pub fn preferences(db: &Database) -> serde_json::Map<String, serde_json::Value> {
    db.setting(PREFERENCES_SETTING)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// A non-empty text preference.
pub(crate) fn preference_str(db: &Database, key: &str) -> Option<String> {
    preferences(db)
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Applies the preferences this side uses directly.
pub fn apply_preferences(prefs: &serde_json::Map<String, serde_json::Value>) {
    crate::i18n::set_english(prefs.get("language").and_then(|v| v.as_str()) == Some("en"));
}

#[tauri::command]
pub fn get_preferences(state: State<'_, AppState>) -> CmdResult<serde_json::Value> {
    Ok(serde_json::Value::Object(preferences(&state.db())))
}

#[tauri::command]
pub fn set_preferences(
    state: State<'_, AppState>,
    preferences: serde_json::Value,
) -> CmdResult<()> {
    if let Some(map) = preferences.as_object() {
        apply_preferences(map);
    }
    state
        .db()
        .set_setting(PREFERENCES_SETTING, &preferences.to_string())
        .map_err(err)
}

#[tauri::command]
pub fn write_text_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
    contents: String,
) -> CmdResult<()> {
    crate::files::write(&app, &path, contents.as_bytes()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diary_titles() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
        assert_eq!(diary_title(d, "iso"), "2026-11-04");
        assert_eq!(diary_title(d, "isoWeekday"), "2026-11-04 Wed");
        assert_eq!(diary_title(d, "ja"), "2026年11月4日（水）");
    }

    #[test]
    fn dates_in_titles() {
        assert_eq!(date_in_title("2026-11-04"), Some("2026-11-04".into()));
        assert_eq!(date_in_title("2026/11/4 雨"), Some("2026-11-04".into()));
        assert_eq!(
            date_in_title("2026年11月4日（水）"),
            Some("2026-11-04".into())
        );
        assert_eq!(date_in_title("2026-13-01"), None);
        assert_eq!(date_in_title("第3章 2026-11-04"), None);
        assert_eq!(date_in_title("1234"), None);
    }

    #[test]
    fn export_names_and_links() {
        assert_eq!(file_name("CON", "x"), "CON_");
        assert_eq!(file_name("nul.txt", "x"), "nul.txt_");
        assert_eq!(file_name("com1", "x"), "com1_");
        assert_eq!(file_name("Console", "x"), "Console");
        assert_eq!(file_name("a/b: c?", "x"), "a_b_ c_");
        assert_eq!(file_name(" . ", "x"), "x");
        let id = "0123456789abcdef0123456789abcdef";
        let text = format!("![絵](:/{id}) [他](:/{})", "f".repeat(32));
        let out = regex_lite_links()(&text, &mut |i| (i == id).then(|| "../_resources/a.png".to_string()));
        assert_eq!(out, format!("![絵](../_resources/a.png) [他](:/{})", "f".repeat(32)));
    }

    #[test]
    fn streaks() {
        let dates: std::collections::HashSet<String> = ["2026-11-01", "2026-11-02", "2026-11-03"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(streak(&dates, "2026-11-03"), 3);
        assert_eq!(streak(&dates, "2026-11-04"), 3, "today not written yet");
        assert_eq!(streak(&dates, "2026-11-05"), 0);
    }
}
