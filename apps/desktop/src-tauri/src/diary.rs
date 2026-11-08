//! The diary: entries with their mood and weather, "on this day" in earlier
//! years, statistics, and exports by month or year.

use std::collections::BTreeMap;

use nisshoku_core::{Database, Note};
use serde::Serialize;
use serde_json::json;
use tauri::State;

use crate::notes::{CmdResult, diary_date, diary_folders, err};
use crate::state::AppState;
use crate::text::{count_chars, note_excerpt};
use crate::userdata;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiaryEntry {
    note_id: String,
    date: String,
    title: String,
    excerpt: String,
    char_count: usize,
    mood: Option<String>,
    weather: Option<String>,
    events: Vec<DiaryEvent>,
}

/// A short note of something that happened, with its time (`HH:MM`).
#[derive(Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct DiaryEvent {
    time: String,
    text: String,
}

fn events_of(n: &Note) -> Vec<DiaryEvent> {
    userdata::get(&n.user_data, "events")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn meta(n: &Note, key: &str) -> Option<String> {
    userdata::get(&n.user_data, key).and_then(|v| v.as_str().map(str::to_string))
}

/// The entry of `date` (YYYY-MM-DD): its note and length.
pub(crate) fn entry_on(db: &Database, date: &str) -> Option<(String, usize)> {
    entries(db)
        .ok()?
        .into_iter()
        .find(|(e, _)| e.date == date)
        .map(|(e, _)| (e.note_id, e.char_count))
}

/// All diary entries (with a date), newest first.
fn entries(db: &Database) -> CmdResult<Vec<(DiaryEntry, String)>> {
    let mut out = Vec::new();
    for f in diary_folders(db)? {
        for n in db
            .query::<Note, _>(
                "parent_id = ?1 AND deleted_time = 0 AND is_conflict = 0",
                [&f.id],
            )
            .map_err(err)?
        {
            let Some(date) = diary_date(&n) else { continue };
            out.push((
                DiaryEntry {
                    note_id: n.id.clone(),
                    date,
                    title: n.title.clone(),
                    excerpt: note_excerpt(&n.title, &n.body, 70),
                    char_count: count_chars(&n.body),
                    mood: meta(&n, "mood"),
                    weather: meta(&n, "weather"),
                    events: events_of(&n),
                },
                n.body,
            ));
        }
    }
    out.sort_by(|a, b| b.0.date.cmp(&a.0.date));
    Ok(out)
}

/// Diary entries, newest first; with `query`, those containing every word.
#[tauri::command]
pub fn diary_entries(
    state: State<'_, AppState>,
    query: Option<String>,
) -> CmdResult<Vec<DiaryEntry>> {
    let all = entries(&state.db())?;
    let query = query.unwrap_or_default();
    Ok(all
        .into_iter()
        .filter(|(e, body)| {
            let events: Vec<&str> = e.events.iter().map(|x| x.text.as_str()).collect();
            crate::text::matches_query(&format!("{}\n{}\n{}", e.title, body, events.join("\n")), &query)
        })
        .map(|(e, _)| e)
        .collect())
}

/// Entries of the same day in earlier years (`date` is `YYYY-MM-DD`).
#[tauri::command]
pub fn on_this_day(state: State<'_, AppState>, date: String) -> CmdResult<Vec<DiaryEntry>> {
    let (year, month_day) = date.split_once('-').ok_or("bad date")?;
    Ok(entries(&state.db())?
        .into_iter()
        .map(|(e, _)| e)
        .filter(|e| e.date.get(5..) == Some(month_day) && e.date.get(..4).is_some_and(|y| y < year))
        .collect())
}

#[tauri::command]
pub fn set_diary_meta(
    state: State<'_, AppState>,
    note_id: String,
    mood: Option<String>,
    weather: Option<String>,
) -> CmdResult<()> {
    let db = state.db();
    let mut n = db
        .load::<Note>(&note_id)
        .map_err(err)?
        .ok_or_else(|| crate::i18n::tr("ノートが見つかりません", "Note not found").to_string())?;
    let value = |v: Option<String>| v.filter(|s| !s.is_empty()).map(|s| json!(s));
    n.user_data = userdata::set(&n.user_data, "mood", value(mood));
    n.user_data = userdata::set(&n.user_data, "weather", value(weather));
    db.save_meta(&mut n).map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryStats {
    streak: usize,
    longest_streak: usize,
    month_days: usize,
    total_days: usize,
    total_chars: usize,
    /// Days per mood, most frequent first.
    moods: Vec<(String, usize)>,
}

#[tauri::command]
pub fn diary_stats(state: State<'_, AppState>, today: String) -> CmdResult<DiaryStats> {
    let all = entries(&state.db())?;
    let dates: std::collections::BTreeSet<String> =
        all.iter().map(|(e, _)| e.date.clone()).collect();
    let parse = |d: &str| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok();

    // Current streak: up to today, or yesterday if today is not written yet.
    let mut streak = 0;
    if let Some(mut day) = parse(&today) {
        let key = |d: chrono::NaiveDate| d.format("%Y-%m-%d").to_string();
        if !dates.contains(&key(day)) {
            day = day.pred_opt().expect("date");
        }
        while dates.contains(&key(day)) {
            streak += 1;
            day = day.pred_opt().expect("date");
        }
    }
    let mut longest = 0;
    let mut run = 0;
    let mut prev: Option<chrono::NaiveDate> = None;
    for d in dates.iter().filter_map(|d| parse(d)) {
        run = if prev.and_then(|p| p.succ_opt()) == Some(d) {
            run + 1
        } else {
            1
        };
        longest = longest.max(run);
        prev = Some(d);
    }
    let month = today.get(..7).unwrap_or("");
    let mut moods: BTreeMap<String, usize> = BTreeMap::new();
    for (e, _) in &all {
        if let Some(m) = &e.mood {
            *moods.entry(m.clone()).or_default() += 1;
        }
    }
    let mut moods: Vec<(String, usize)> = moods.into_iter().collect();
    moods.sort_by_key(|m| std::cmp::Reverse(m.1));
    Ok(DiaryStats {
        streak,
        longest_streak: longest,
        month_days: dates.iter().filter(|d| d.starts_with(month)).count(),
        total_days: dates.len(),
        total_chars: all.iter().map(|(e, _)| e.char_count).sum(),
        moods,
    })
}

/// The entries of a period (`YYYY` or `YYYY-MM`) as one text, oldest first.
#[tauri::command]
pub fn diary_export<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    period: String,
    path: String,
) -> CmdResult<usize> {
    let mut list: Vec<(DiaryEntry, String)> = entries(&state.db())?
        .into_iter()
        .filter(|(e, _)| e.date.starts_with(&period))
        .collect();
    list.reverse();
    let mut text = String::new();
    for (e, body) in &list {
        let marks: Vec<&str> = [e.mood.as_deref(), e.weather.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        text.push_str(&format!("■ {}", e.title));
        if !marks.is_empty() {
            text.push_str(&format!("　{}", marks.join(" ")));
        }
        text.push_str("\n\n");
        for ev in &e.events {
            text.push_str(&format!("・{} {}\n", ev.time, ev.text));
        }
        if !e.events.is_empty() {
            text.push('\n');
        }
        text.push_str(body.trim_end());
        text.push_str("\n\n\n");
    }
    crate::files::write(&app, &path, text.as_bytes()).map_err(err)?;
    Ok(list.len())
}

#[tauri::command]
pub fn set_diary_events(
    state: State<'_, AppState>,
    note_id: String,
    events: Vec<DiaryEvent>,
) -> CmdResult<()> {
    let db = state.db();
    let mut n = db
        .load::<Note>(&note_id)
        .map_err(err)?
        .ok_or_else(|| crate::i18n::tr("ノートが見つかりません", "Note not found").to_string())?;
    let mut events: Vec<DiaryEvent> = events
        .into_iter()
        .filter(|e| !e.text.trim().is_empty())
        .collect();
    events.sort_by(|a, b| a.time.cmp(&b.time));
    let value = (!events.is_empty()).then(|| serde_json::to_value(&events).expect("serializable"));
    n.user_data = userdata::set(&n.user_data, "events", value);
    db.save_meta(&mut n).map_err(err)
}

// -------------------------------------------------------------------------
// The memo: things to remember, kept as a checklist note in the diary.

#[derive(Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct MemoItem {
    text: String,
    done: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Memo {
    note_id: String,
    items: Vec<MemoItem>,
}

fn memo_note(db: &Database) -> CmdResult<Note> {
    for f in diary_folders(db)? {
        if let Some(n) = db
            .query::<Note, _>(
                "parent_id = ?1 AND deleted_time = 0 AND is_conflict = 0",
                [&f.id],
            )
            .map_err(err)?
            .into_iter()
            .find(|n| userdata::get(&n.user_data, "memo").and_then(|v| v.as_bool()) == Some(true))
        {
            return Ok(n);
        }
    }
    let folder = match diary_folders(db)?.into_iter().next() {
        Some(f) => f,
        None => crate::notes::ensure_diary_folder(db)?,
    };
    let mut n = Note::new(&folder.id, crate::i18n::tr("備忘録", "Memo"), "");
    n.user_data = userdata::set("", "memo", Some(json!(true)));
    db.save(&mut n).map_err(err)?;
    Ok(n)
}

/// A list line of the memo: its indentation and item (text may be empty).
fn memo_line(line: &str) -> Option<(&str, MemoItem)> {
    let rest = line.trim_start();
    let indent = &line[..line.len() - rest.len()];
    let rest = rest.strip_prefix("- ").or_else(|| rest.strip_prefix("* "))?;
    let (done, text) = if let Some(t) = rest.strip_prefix("[ ]") {
        (false, t)
    } else if let Some(t) = rest.strip_prefix("[x]").or_else(|| rest.strip_prefix("[X]")) {
        (true, t)
    } else {
        (false, rest)
    };
    Some((
        indent,
        MemoItem {
            text: text.trim().to_string(),
            done,
        },
    ))
}

fn parse_memo(body: &str) -> Vec<MemoItem> {
    body.lines()
        .filter_map(|l| memo_line(l).map(|(_, i)| i))
        .filter(|i| !i.text.is_empty())
        .collect()
}

/// The memo's body with its items replaced by `items`, in place: other lines
/// (headings, paragraphs written in the editor) and indentation stay.
fn write_memo(body: &str, items: &[MemoItem]) -> String {
    let format = |indent: &str, i: &MemoItem| {
        format!("{indent}- [{}] {}", if i.done { "x" } else { " " }, i.text.trim().replace('\n', " "))
    };
    let items: Vec<&MemoItem> = items.iter().filter(|i| !i.text.trim().is_empty()).collect();
    let mut next = items.iter();
    let mut out: Vec<String> = Vec::new();
    let mut last_item = None;
    for line in body.lines() {
        match memo_line(line) {
            Some((_, old)) if old.text.is_empty() => {} // an empty item: dropped
            Some((indent, _)) => {
                if let Some(i) = next.next() {
                    out.push(format(indent, i));
                    last_item = Some(out.len());
                }
            }
            None => out.push(line.to_string()),
        }
    }
    let rest: Vec<String> = next.map(|i| format("", i)).collect();
    let at = last_item.unwrap_or(out.len());
    out.splice(at..at, rest);
    out.join("\n")
}

#[tauri::command]
pub fn get_diary_memo(state: State<'_, AppState>) -> CmdResult<Memo> {
    let db = state.db();
    let n = memo_note(&db)?;
    Ok(Memo {
        items: parse_memo(&n.body),
        note_id: n.id,
    })
}

#[tauri::command]
pub fn set_diary_memo(state: State<'_, AppState>, items: Vec<MemoItem>) -> CmdResult<()> {
    let db = state.db();
    let mut n = memo_note(&db)?;
    n.body = write_memo(&n.body, &items);
    db.save(&mut n).map_err(err)
}

// -------------------------------------------------------------------------
// Duplicates. Two devices that each open the same day before syncing (or each
// create a diary notebook) end up with two entries of that day after the
// sync. They are merged here: the oldest entry (by creation time, then id —
// the same choice on every device) keeps the text of the others.

/// Merges diary notebooks and entries of the same day. Returns whether
/// anything changed.
pub(crate) fn merge_duplicates(db: &Database) -> CmdResult<bool> {
    let mut changed = false;
    let folders = diary_folders(db)?;
    let Some(main) = folders.first() else {
        return Ok(false);
    };

    // Extra diary notebooks: move their notes into the first one.
    for extra in &folders[1..] {
        for mut n in db
            .query::<Note, _>("parent_id = ?1", [&extra.id])
            .map_err(err)?
        {
            n.parent_id = main.id.clone();
            db.save_meta(&mut n).map_err(err)?;
            changed = true;
        }
        let children = db
            .query::<nisshoku_core::Folder, _>("parent_id = ?1", [&extra.id])
            .map_err(err)?;
        if children.is_empty() {
            db.delete(nisshoku_core::ItemType::Folder, &extra.id)
                .map_err(err)?;
            changed = true;
        }
        // (A notebook with notebooks inside is kept, and not counted as a
        // change every time: that would sync again after every sync.)
    }

    let template = crate::notes::preferences(db)
        .get("diaryTemplate")
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let blank = |body: &str| body.trim().is_empty() || body.trim() == template.trim();

    let notes = db
        .query::<Note, _>(
            "parent_id = ?1 AND deleted_time = 0 AND is_conflict = 0",
            [&main.id],
        )
        .map_err(err)?;
    let is_memo =
        |n: &Note| userdata::get(&n.user_data, "memo").and_then(|v| v.as_bool()) == Some(true);
    let mut groups: BTreeMap<String, Vec<Note>> = BTreeMap::new();
    for n in notes {
        // Only entries the app made for a day (with their recorded date):
        // notes whose title merely starts with a date are the user's own.
        let key = if is_memo(&n) {
            Some("memo".to_string())
        } else {
            userdata::get(&n.user_data, "diaryDate").and_then(|v| v.as_str().map(str::to_string))
        };
        if let Some(key) = key {
            groups.entry(key).or_default().push(n);
        }
    }

    for (key, mut group) in groups {
        if group.len() < 2 {
            continue;
        }
        group.sort_by(|a, b| (a.created_time, &a.id).cmp(&(b.created_time, &b.id)));
        let mut rest = group.split_off(1);
        let mut keep = group.pop().expect("one note");
        for other in &mut rest {
            if key == "memo" {
                let lines: Vec<String> = keep.body.lines().map(str::to_string).collect();
                for l in other.body.lines() {
                    if !l.trim().is_empty() && !lines.iter().any(|k| k == l) {
                        if !keep.body.is_empty() && !keep.body.ends_with('\n') {
                            keep.body.push('\n');
                        }
                        keep.body.push_str(l);
                    }
                }
            } else {
                merge_entry(&mut keep, other, &blank);
            }
        }
        db.save(&mut keep).map_err(err)?;
        for other in rest {
            if blank(&other.body) && events_of(&other).is_empty() {
                db.delete(nisshoku_core::ItemType::Note, &other.id)
                    .map_err(err)?;
            } else {
                // Its text is in the kept entry now; the trash keeps the
                // original just in case.
                let mut other = other;
                other.deleted_time = nisshoku_core::time::now_ms();
                db.save(&mut other).map_err(err)?;
            }
        }
        changed = true;
    }
    Ok(changed)
}

/// Adds the text, mood, weather and events of `other` to `keep`.
fn merge_entry(keep: &mut Note, other: &Note, blank: &dyn Fn(&str) -> bool) {
    if !blank(&other.body) && other.body.trim() != keep.body.trim() {
        if blank(&keep.body) {
            keep.body = other.body.clone();
        } else if !keep.body.contains(other.body.trim()) {
            keep.body = format!("{}\n\n{}", keep.body.trim_end(), other.body.trim_start());
        }
    }
    for key in ["mood", "weather"] {
        if meta(keep, key).is_none()
            && let Some(v) = meta(other, key)
        {
            keep.user_data = userdata::set(&keep.user_data, key, Some(json!(v)));
        }
    }
    let mut events = events_of(keep);
    for e in events_of(other) {
        if !events.contains(&e) {
            events.push(e);
        }
    }
    events.sort_by(|a, b| a.time.cmp(&b.time));
    if !events.is_empty() {
        keep.user_data = userdata::set(
            &keep.user_data,
            "events",
            Some(serde_json::to_value(&events).expect("serializable")),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(db: &Database, folder: &str, date: &str, body: &str, t: i64) -> Note {
        let mut n = Note::new(folder, date, body);
        n.created_time = t;
        n.user_data = userdata::set("", "diaryDate", Some(json!(date)));
        db.save(&mut n).unwrap();
        n
    }

    #[test]
    fn the_memo_keeps_what_else_was_written() {
        let body = "# 買うもの

- [ ] 牛乳
  - [x] 卵
- [ ]

メモ書き";
        assert_eq!(parse_memo(body).len(), 2);
        let items = vec![
            MemoItem { text: "牛乳".into(), done: true },
            MemoItem { text: "卵".into(), done: true },
            MemoItem { text: "パン".into(), done: false },
        ];
        assert_eq!(
            write_memo(body, &items),
            "# 買うもの

- [x] 牛乳
  - [x] 卵
- [ ] パン

メモ書き"
        );
        assert_eq!(write_memo("", &items[..1]), "- [x] 牛乳");
    }

    #[test]
    fn duplicates_are_merged() {
        let db = Database::open_in_memory().unwrap();
        let a = crate::notes::ensure_diary_folder(&db).unwrap();
        let mut b = nisshoku_core::Folder::new("", "Diary");
        b.user_data = userdata::set("", "diary", Some(json!(true)));
        b.created_time = a.created_time + 1;
        db.save(&mut b).unwrap();

        let first = entry(&db, &a.id, "2026-11-08", "朝の記録", 1_000);
        entry(&db, &b.id, "2026-11-08", "夜の記録", 2_000);
        entry(&db, &b.id, "2026-11-08", "", 3_000);
        entry(&db, &b.id, "2026-11-07", "昨日", 4_000);

        assert!(merge_duplicates(&db).unwrap());
        assert_eq!(diary_folders(&db).unwrap().len(), 1);
        let all = entries(&db).unwrap();
        assert_eq!(all.len(), 2);
        let today = all.iter().find(|(e, _)| e.date == "2026-11-08").unwrap();
        assert_eq!(today.0.note_id, first.id);
        assert_eq!(today.1, "朝の記録\n\n夜の記録");
        // Nothing left to merge.
        assert!(!merge_duplicates(&db).unwrap());
    }
}
