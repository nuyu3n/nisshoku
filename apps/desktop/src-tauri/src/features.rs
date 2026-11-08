//! Tags, pins, to-dos, manual order, links between notes, templates and
//! images. Everything is stored the way Joplin stores it (tags and note-tag
//! links, `is_todo`, `order`, `:/id` links and resources), or in `user_data`.

use std::collections::HashMap;

use nisshoku_core::{Database, Folder, ItemType, Note, NoteTag, Resource, Tag, time::now_ms};
use serde::Serialize;
use serde_json::json;
use tauri::State;

use crate::i18n::tr;
use crate::notes::{CmdResult, err, is_template};
use crate::state::AppState;
use crate::userdata;

// -------------------------------------------------------------------------
// Tags

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    id: String,
    title: String,
    note_count: usize,
}

/// Tag titles of each note (only notes that have tags).
pub fn tag_titles_by_note(db: &Database) -> CmdResult<HashMap<String, Vec<String>>> {
    let tags: HashMap<String, String> = db
        .all::<Tag>()
        .map_err(err)?
        .into_iter()
        .map(|t| (t.id, t.title))
        .collect();
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for nt in db.all::<NoteTag>().map_err(err)? {
        if let Some(title) = tags.get(&nt.tag_id) {
            out.entry(nt.note_id).or_default().push(title.clone());
        }
    }
    for titles in out.values_mut() {
        titles.sort();
        titles.dedup();
    }
    Ok(out)
}

#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> CmdResult<Vec<TagDto>> {
    let db = state.db();
    let live: std::collections::HashSet<String> = db
        .query::<Note, _>("deleted_time = 0 AND is_conflict = 0", [])
        .map_err(err)?
        .into_iter()
        .map(|n| n.id)
        .collect();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for nt in db.all::<NoteTag>().map_err(err)? {
        if live.contains(&nt.note_id) {
            *counts.entry(nt.tag_id).or_default() += 1;
        }
    }
    let mut tags: Vec<TagDto> = db
        .all::<Tag>()
        .map_err(err)?
        .into_iter()
        .map(|t| TagDto {
            note_count: counts.get(&t.id).copied().unwrap_or(0),
            id: t.id,
            title: t.title,
        })
        .collect();
    tags.sort_by_key(|t| t.title.to_lowercase());
    Ok(tags)
}

/// Sets the tags of a note by title, creating tags that don't exist
/// (matched without regard to case, as Joplin does).
#[tauri::command]
pub fn set_note_tags(
    state: State<'_, AppState>,
    note_id: String,
    titles: Vec<String>,
) -> CmdResult<()> {
    let db = state.db();
    let mut by_title: HashMap<String, Tag> = db
        .all::<Tag>()
        .map_err(err)?
        .into_iter()
        .map(|t| (t.title.to_lowercase(), t))
        .collect();
    let mut wanted = std::collections::HashSet::new();
    for title in titles.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let key = title.to_lowercase();
        let tag = match by_title.get(&key) {
            Some(t) => t.clone(),
            None => {
                let mut t = Tag {
                    title: title.to_string(),
                    ..Default::default()
                };
                db.save(&mut t).map_err(err)?;
                by_title.insert(key, t.clone());
                t
            }
        };
        wanted.insert(tag.id);
    }
    let existing = db
        .query::<NoteTag, _>("note_id = ?1", [&note_id])
        .map_err(err)?;
    for nt in &existing {
        if !wanted.contains(&nt.tag_id) {
            db.delete(ItemType::NoteTag, &nt.id).map_err(err)?;
        }
    }
    for tag_id in wanted {
        if !existing.iter().any(|nt| nt.tag_id == tag_id) {
            let mut nt = NoteTag {
                note_id: note_id.clone(),
                tag_id,
                ..Default::default()
            };
            db.save(&mut nt).map_err(err)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn rename_tag(state: State<'_, AppState>, id: String, title: String) -> CmdResult<()> {
    let db = state.db();
    let mut t = db
        .load::<Tag>(&id)
        .map_err(err)?
        .ok_or_else(|| tr("タグが見つかりません", "Tag not found").to_string())?;
    t.title = title.trim().to_string();
    db.save(&mut t).map_err(err)
}

#[tauri::command]
pub fn delete_tag(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.db().delete(ItemType::Tag, &id).map_err(err)
}

// -------------------------------------------------------------------------
// Pins, to-dos, order

fn load_note(db: &Database, id: &str) -> CmdResult<Note> {
    db.load::<Note>(id)
        .map_err(err)?
        .ok_or_else(|| tr("ノートが見つかりません", "Note not found").to_string())
}

#[tauri::command]
pub fn set_note_pinned(state: State<'_, AppState>, id: String, pinned: bool) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &id)?;
    n.user_data = userdata::set(&n.user_data, "pinned", pinned.then_some(json!(true)));
    db.save(&mut n).map_err(err)
}

#[tauri::command]
pub fn set_note_todo(state: State<'_, AppState>, id: String, todo: bool) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &id)?;
    n.is_todo = todo as i64;
    if !todo {
        n.todo_completed = 0;
    }
    db.save(&mut n).map_err(err)
}

#[tauri::command]
pub fn set_todo_completed(
    state: State<'_, AppState>,
    id: String,
    completed: bool,
) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &id)?;
    n.is_todo = 1;
    n.todo_completed = if completed { now_ms() } else { 0 };
    db.save(&mut n).map_err(err)
}

/// Gives notes the manual order of `ids` (first = top). Joplin shows custom
/// order by descending `order`.
#[tauri::command]
pub fn set_notes_order(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<()> {
    let db = state.db();
    let base = now_ms() as f64;
    for (i, id) in ids.iter().enumerate() {
        let mut n = load_note(&db, id)?;
        let order = base - i as f64;
        if n.order != order {
            n.order = order;
            db.save(&mut n).map_err(err)?;
        }
    }
    Ok(())
}

// -------------------------------------------------------------------------
// Links between notes

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedNote {
    id: String,
    title: String,
}

/// Notes that link to `note_id` (with Joplin's `:/id` links).
#[tauri::command]
pub fn list_backlinks(state: State<'_, AppState>, note_id: String) -> CmdResult<Vec<LinkedNote>> {
    let db = state.db();
    let needle = format!(":/{note_id}");
    Ok(db
        .query::<Note, _>(
            "deleted_time = 0 AND is_conflict = 0 AND body LIKE ?1",
            [format!("%{needle}%")],
        )
        .map_err(err)?
        .into_iter()
        .filter(|n| n.id != note_id)
        .map(|n| LinkedNote {
            id: n.id,
            title: n.title,
        })
        .collect())
}

// -------------------------------------------------------------------------
// Templates: notes flagged in `user_data`, in a notebook of their own.

fn templates_folder(db: &Database) -> CmdResult<Folder> {
    let existing = db
        .query::<Folder, _>("deleted_time = 0", [])
        .map_err(err)?
        .into_iter()
        .find(|f| userdata::get(&f.user_data, "templates").and_then(|v| v.as_bool()) == Some(true));
    if let Some(f) = existing {
        return Ok(f);
    }
    let mut f = Folder::new("", "Templates");
    f.user_data = userdata::set("", "templates", Some(json!(true)));
    db.save(&mut f).map_err(err)?;
    Ok(f)
}

#[tauri::command]
pub fn save_as_template(state: State<'_, AppState>, note_id: String) -> CmdResult<String> {
    let db = state.db();
    let src = load_note(&db, &note_id)?;
    let folder = templates_folder(&db)?;
    let mut n = Note::new(&folder.id, &src.title, &src.body);
    n.user_data = userdata::set("", "template", Some(json!(true)));
    db.save(&mut n).map_err(err)?;
    Ok(n.id)
}

#[tauri::command]
pub fn list_templates(state: State<'_, AppState>) -> CmdResult<Vec<LinkedNote>> {
    let db = state.db();
    let mut out: Vec<LinkedNote> = db
        .query::<Note, _>("deleted_time = 0", [])
        .map_err(err)?
        .into_iter()
        .filter(is_template)
        .map(|n| LinkedNote {
            id: n.id,
            title: n.title,
        })
        .collect();
    out.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(out)
}

// -------------------------------------------------------------------------
// Images: Joplin resources (metadata item + file named `<id>.<ext>`).

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceFile {
    id: String,
    path: String,
    mime: String,
    title: String,
    size: i64,
}

#[tauri::command]
pub fn save_resource(
    state: State<'_, AppState>,
    name: String,
    mime: String,
    data: Vec<u8>,
) -> CmdResult<ResourceFile> {
    if data.len() > 200 * 1024 * 1024 {
        return Err(tr(
            "ファイルが大きすぎます（200MBまで）",
            "The file is too large (200 MB at most)",
        )
        .into());
    }
    let ext = std::path::Path::new(&name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .filter(|e| e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| mime.rsplit('/').next().unwrap_or("bin").to_string());
    let mut r = Resource {
        id: nisshoku_core::ids::new_id(),
        title: name.clone(),
        filename: name,
        mime: mime.clone(),
        file_extension: ext,
        size: data.len() as i64,
        blob_updated_time: now_ms(),
        ..Default::default()
    };
    let path = state.resource_dir.join(r.file_name());
    std::fs::write(&path, &data).map_err(err)?;
    state.db().save(&mut r).map_err(err)?;
    Ok(ResourceFile {
        id: r.id,
        path: path.to_string_lossy().into_owned(),
        mime,
        title: r.title,
        size: r.size,
    })
}

/// Adds a file from disk as a resource (for "insert image").
#[tauri::command]
pub fn import_resource<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    path: String,
) -> CmdResult<ResourceFile> {
    let data = crate::files::read(&app, &path).map_err(err)?;
    let (name, ext) = if path.starts_with("content://") {
        let ext = crate::files::sniff_extension(&data).unwrap_or("");
        let name = if ext.is_empty() {
            "file".into()
        } else {
            format!("file.{ext}")
        };
        (name, ext.to_string())
    } else {
        let p = std::path::Path::new(&path);
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("file");
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        (name.to_string(), ext.to_lowercase())
    };
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain",
        "doc" | "docx" => "application/msword",
        "xls" | "xlsx" => "application/vnd.ms-excel",
        "zip" => "application/zip",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    };
    save_resource(state, name, mime.to_string(), data)
}

/// The local files of all resources, so that `:/id` images can be shown.
#[tauri::command]
pub fn resource_files(state: State<'_, AppState>) -> CmdResult<Vec<ResourceFile>> {
    let db = state.db();
    Ok(db
        .all::<Resource>()
        .map_err(err)?
        .into_iter()
        .map(|r| ResourceFile {
            path: state
                .resource_dir
                .join(r.file_name())
                .to_string_lossy()
                .into_owned(),
            mime: r.mime,
            title: if r.title.is_empty() {
                r.filename
            } else {
                r.title
            },
            size: r.size,
            id: r.id,
        })
        .collect())
}

/// Opens an attachment with the default app of the system.
#[tauri::command]
pub fn open_resource<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let r = state
        .db()
        .load::<Resource>(&id)
        .map_err(err)?
        .ok_or_else(|| tr("添付ファイルが見つかりません", "Attachment not found").to_string())?;
    let path = state.resource_dir.join(r.file_name());
    if !path.exists() {
        return Err(tr(
            "添付ファイルがまだこの端末にありません（同期を待ってください）",
            "The file is not on this device yet (wait for the sync)",
        )
        .into());
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(err)
}

// -------------------------------------------------------------------------
// Web clips

/// Downloads a web page and saves its main text as a note (Markdown).
#[tauri::command]
pub async fn clip_url(
    state: State<'_, AppState>,
    url: String,
    parent_id: Option<String>,
) -> CmdResult<crate::notes::NoteDto> {
    let url = reqwest::Url::parse(url.trim())
        .map_err(|_| tr("URL が正しくありません", "The URL is not valid").to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(tr(
            "http か https の URL を入力してください",
            "Enter an http or https URL",
        )
        .into());
    }
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "Mozilla/5.0 (compatible; Nisshoku/",
            env!("CARGO_PKG_VERSION"),
            ")"
        ))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(err)?;
    let resp = client.get(url.clone()).send().await.map_err(|e| {
        format!(
            "{}: {e}",
            tr("ページを取得できませんでした", "Could not fetch the page")
        )
    })?;
    if !resp.status().is_success() {
        return Err(format!(
            "{} (HTTP {})",
            tr("ページを取得できませんでした", "Could not fetch the page"),
            resp.status()
        ));
    }
    let final_url = resp.url().clone();
    let bytes = resp.bytes().await.map_err(err)?;
    if bytes.len() > 20 * 1024 * 1024 {
        return Err(tr("ページが大きすぎます", "The page is too large").into());
    }
    let html = String::from_utf8_lossy(&bytes).into_owned();
    let (title, body) = crate::clip::html_to_markdown(&html, &final_url);
    let title = if title.is_empty() {
        final_url.to_string()
    } else {
        title
    };
    let note_body = format!(
        "> {}: [{}]({})\n\n{}",
        tr("出典", "Source"),
        final_url,
        final_url,
        body
    );

    let db = state.db();
    let parent = match parent_id.filter(|p| !p.is_empty()) {
        Some(p) => p,
        None => db
            .query::<Folder, _>("deleted_time = 0", [])
            .map_err(err)?
            .into_iter()
            .min_by_key(|f| f.created_time)
            .map(|f| f.id)
            .unwrap_or_default(),
    };
    let mut n = Note::new(&parent, &title, &note_body);
    n.source_url = final_url.to_string();
    if parent.is_empty() {
        let mut f = Folder::new("", "Notes");
        db.save(&mut f).map_err(err)?;
        n.parent_id = f.id;
    }
    db.save(&mut n).map_err(err)?;
    Ok(crate::notes::dto(&n))
}
