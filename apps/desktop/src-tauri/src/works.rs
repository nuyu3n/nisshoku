//! Works: a notebook marked as a work (a novel, a collection of stories)
//! holds its manuscript, its characters, its plot board and its notes.
//!
//! Everything lives in ordinary notes and notebooks, with the extra data in
//! their `user_data` (which Joplin keeps and syncs):
//!
//! - the work's notebook: `work` (logline, synopsis, target length, deadline);
//! - a note's `kind`: `character`, `plot` or `material` (reference notes);
//!   notes without a kind are the manuscript, chapter by chapter, in the
//!   work's notebook and its sub-notebooks (parts);
//! - a chapter's `chapter` (status, synopsis, point-of-view character);
//! - a character's `character` (reading, role, traits, aliases, relations);
//!   the note's title is the name and its body free notes;
//! - the plot note's `plot` (columns and cards); its body is written from
//!   the board, so that the plot reads well in Joplin too.

use std::collections::HashMap;

use nisshoku_core::{Database, Folder, Note};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::State;

use crate::i18n::tr;
use crate::notes::{CmdResult, NoteDto, NoteSummary, dto, err, load_folder, load_note, summary};
use crate::state::AppState;
use crate::text::note_excerpt;
use crate::userdata;

const WORK_KEY: &str = "work";
const KIND_KEY: &str = "kind";
const CHAPTER_KEY: &str = "chapter";
const CHARACTER_KEY: &str = "character";
const PLOT_KEY: &str = "plot";

pub const KIND_CHARACTER: &str = "character";
pub const KIND_PLOT: &str = "plot";
pub const KIND_MATERIAL: &str = "material";

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkMeta {
    /// One sentence: what the story is.
    pub logline: String,
    pub synopsis: String,
    pub genre: String,
    /// Planned length of the manuscript, in characters.
    pub target_chars: Option<i64>,
    /// `YYYY-MM-DD`.
    pub deadline: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ChapterMeta {
    /// `idea`, `draft`, `revise` or `done`; empty when not set.
    pub status: String,
    pub synopsis: String,
    /// The point-of-view character (a character note's id).
    pub pov: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Relation {
    /// The other character's note.
    pub target: String,
    pub label: String,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Character {
    /// How the name is read (ふりがな).
    pub ruby: String,
    /// `protagonist`, `antagonist`, `main`, `supporting` or `minor`.
    pub role: String,
    pub age: String,
    pub gender: String,
    pub appearance: String,
    pub personality: String,
    pub background: String,
    /// What they want, and what stands in the way.
    pub motivation: String,
    /// Other names the text calls them by (nicknames, family name…).
    pub aliases: Vec<String>,
    /// A colour of their own, `#rrggbb`.
    pub color: String,
    pub relations: Vec<Relation>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PlotColumn {
    pub id: String,
    pub title: String,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PlotCard {
    pub id: String,
    /// The column it is in.
    pub column: String,
    pub title: String,
    pub text: String,
    /// The chapter (note id) it is told in.
    pub chapter: Option<String>,
    pub done: bool,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Plot {
    /// `kishotenketsu`, `threeAct`, `heroJourney` or `free`.
    pub template: String,
    pub columns: Vec<PlotColumn>,
    pub cards: Vec<PlotCard>,
}

// -------------------------------------------------------------------------
// Reading the data

pub(crate) fn work_meta(f: &Folder) -> Option<WorkMeta> {
    let v = userdata::get(&f.user_data, WORK_KEY)?;
    Some(serde_json::from_value(v).unwrap_or_default())
}

pub(crate) fn is_work(f: &Folder) -> bool {
    work_meta(f).is_some()
}

pub(crate) fn kind_of(n: &Note) -> Option<String> {
    userdata::get(&n.user_data, KIND_KEY).and_then(|v| v.as_str().map(str::to_string))
}

fn chapter_meta(n: &Note) -> ChapterMeta {
    userdata::get(&n.user_data, CHAPTER_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn character_of(n: &Note) -> Character {
    userdata::get(&n.user_data, CHARACTER_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn plot_of(n: &Note) -> Plot {
    userdata::get(&n.user_data, PLOT_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// The notebooks of the work rooted at `root`, depth-first and by title,
/// with their depth below the root.
fn work_tree(db: &Database, root: &Folder) -> CmdResult<Vec<(Folder, usize)>> {
    let folders: Vec<Folder> = db.query::<Folder, _>("deleted_time = 0", []).map_err(err)?;
    let mut children: HashMap<String, Vec<Folder>> = HashMap::new();
    for f in folders {
        children.entry(f.parent_id.clone()).or_default().push(f);
    }
    for list in children.values_mut() {
        list.sort_by(|a, b| a.title.cmp(&b.title));
    }
    let mut out = Vec::new();
    fn walk(f: &Folder, depth: usize, children: &HashMap<String, Vec<Folder>>, out: &mut Vec<(Folder, usize)>) {
        out.push((f.clone(), depth));
        if depth > 20 {
            return; // a cycle in damaged data
        }
        for c in children.get(&f.id).into_iter().flatten() {
            walk(c, depth + 1, children, out);
        }
    }
    walk(root, 0, &children, &mut out);
    Ok(out)
}

/// The work `folder_id` belongs to: its own notebook or the nearest marked
/// one above it.
pub(crate) fn work_root(db: &Database, folder_id: &str) -> Option<Folder> {
    let mut id = folder_id.to_string();
    for _ in 0..32 {
        let f = db.load::<Folder>(&id).ok()??;
        if f.deleted_time == 0 && is_work(&f) {
            return Some(f);
        }
        if f.parent_id.is_empty() {
            return None;
        }
        id = f.parent_id;
    }
    None
}

/// Notes of a notebook, in the order the manuscript has: the order set by
/// dragging (higher first), then oldest first.
fn notes_in(db: &Database, folder_id: &str) -> CmdResult<Vec<Note>> {
    let mut notes = db
        .query::<Note, _>(
            "parent_id = ?1 AND deleted_time = 0 AND is_conflict = 0",
            [folder_id],
        )
        .map_err(err)?;
    notes.sort_by(|a, b| {
        b.order
            .total_cmp(&a.order)
            .then(a.user_created_time.max(a.created_time).cmp(&b.user_created_time.max(b.created_time)))
    });
    Ok(notes)
}

/// How many times the text mentions a character by any of `names`
/// (longest name first, matches not overlapping).
fn count_mentions(text: &str, names: &[String]) -> usize {
    let mut names: Vec<&str> = names.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if names.is_empty() {
        return 0;
    }
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let mut count = 0;
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        if let Some(n) = names.iter().find(|n| text[i..].starts_with(**n)) {
            count += 1;
            i += n.len();
        } else {
            // Next character boundary.
            i += 1;
            while i < bytes.len() && !text.is_char_boundary(i) {
                i += 1;
            }
        }
    }
    count
}

fn names_of(n: &Note, c: &Character) -> Vec<String> {
    let mut names = vec![n.title.trim().to_string()];
    names.extend(c.aliases.iter().map(|a| a.trim().to_string()));
    names.retain(|s| !s.is_empty());
    names
}

// -------------------------------------------------------------------------
// The overview of a work

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInfo {
    #[serde(flatten)]
    note: NoteSummary,
    status: String,
    synopsis: String,
    pov: Option<String>,
    /// The characters mentioned, with how often: `[id, count]`, most first.
    characters: Vec<(String, usize)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    folder_id: String,
    title: String,
    depth: usize,
    chapters: Vec<ChapterInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterInfo {
    id: String,
    name: String,
    ruby: String,
    role: String,
    color: String,
    aliases: Vec<String>,
    excerpt: String,
    /// Mentions in the whole manuscript.
    mentions: usize,
    /// The chapters they appear in, in manuscript order.
    chapters: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkOverview {
    id: String,
    title: String,
    meta: WorkMeta,
    sections: Vec<Section>,
    characters: Vec<CharacterInfo>,
    plot_id: Option<String>,
    materials: Vec<NoteSummary>,
    total_chars: usize,
    chapter_count: usize,
    /// Chapters by status (`idea`, `draft`, `revise`, `done`, or `` unset).
    status_counts: HashMap<String, usize>,
}

const ROLE_ORDER: [&str; 5] = ["protagonist", "antagonist", "main", "supporting", "minor"];

fn overview(db: &Database, root: &Folder) -> CmdResult<WorkOverview> {
    let tree = work_tree(db, root)?;
    let mut chapters_by_folder: Vec<(Folder, usize, Vec<Note>)> = Vec::new();
    let mut characters: Vec<(Note, Character)> = Vec::new();
    let mut plot_id = None;
    let mut materials = Vec::new();
    for (folder, depth) in tree {
        let mut chapters = Vec::new();
        for n in notes_in(db, &folder.id)? {
            match kind_of(&n).as_deref() {
                Some(KIND_CHARACTER) => {
                    let c = character_of(&n);
                    characters.push((n, c));
                }
                Some(KIND_PLOT) => {
                    if plot_id.is_none() {
                        plot_id = Some(n.id.clone());
                    }
                }
                Some(KIND_MATERIAL) => materials.push(summary(&n)),
                _ if crate::notes::is_template(&n) => {}
                _ => chapters.push(n),
            }
        }
        chapters_by_folder.push((folder, depth, chapters));
    }
    characters.sort_by_key(|(n, c)| {
        (
            ROLE_ORDER.iter().position(|r| *r == c.role).unwrap_or(ROLE_ORDER.len()),
            n.user_created_time.max(n.created_time),
        )
    });
    let names: Vec<Vec<String>> = characters.iter().map(|(n, c)| names_of(n, c)).collect();
    let mut mentions = vec![0usize; characters.len()];
    let mut appears: Vec<Vec<String>> = vec![Vec::new(); characters.len()];
    let mut status_counts: HashMap<String, usize> = HashMap::new();
    let mut total_chars = 0;
    let mut chapter_count = 0;
    let mut sections = Vec::new();
    for (folder, depth, notes) in chapters_by_folder {
        let mut chapters = Vec::new();
        for n in notes {
            let meta = chapter_meta(&n);
            let text = format!("{}\n{}", n.title, n.body);
            let mut cast: Vec<(String, usize)> = Vec::new();
            for (i, (c, _)) in characters.iter().enumerate() {
                let k = count_mentions(&text, &names[i]);
                if k > 0 {
                    mentions[i] += k;
                    appears[i].push(n.id.clone());
                    cast.push((c.id.clone(), k));
                }
            }
            cast.sort_by(|a, b| b.1.cmp(&a.1));
            *status_counts.entry(meta.status.clone()).or_default() += 1;
            let s = summary(&n);
            total_chars += s.char_count;
            chapter_count += 1;
            chapters.push(ChapterInfo {
                note: s,
                status: meta.status,
                synopsis: meta.synopsis,
                pov: meta.pov,
                characters: cast,
            });
        }
        sections.push(Section {
            folder_id: folder.id.clone(),
            title: folder.title.clone(),
            depth,
            chapters,
        });
    }
    let characters = characters
        .into_iter()
        .enumerate()
        .map(|(i, (n, c))| CharacterInfo {
            id: n.id.clone(),
            name: n.title.clone(),
            ruby: c.ruby,
            role: c.role,
            color: c.color,
            aliases: c.aliases,
            excerpt: note_excerpt(&n.title, &n.body, 60),
            mentions: mentions[i],
            chapters: std::mem::take(&mut appears[i]),
        })
        .collect();
    Ok(WorkOverview {
        id: root.id.clone(),
        title: root.title.clone(),
        meta: work_meta(root).unwrap_or_default(),
        sections,
        characters,
        plot_id,
        materials,
        total_chars,
        chapter_count,
        status_counts,
    })
}

fn load_work(db: &Database, work_id: &str) -> CmdResult<Folder> {
    let f = load_folder(db, work_id)?;
    if is_work(&f) {
        Ok(f)
    } else {
        work_root(db, work_id).ok_or_else(|| tr("作品のノートブックではありません", "Not a work's notebook").to_string())
    }
}

// -------------------------------------------------------------------------
// Commands

/// Marks a notebook as a work (with its details), or unmarks it (`None`).
#[tauri::command]
pub fn set_work(state: State<'_, AppState>, folder_id: String, meta: Option<WorkMeta>) -> CmdResult<()> {
    let db = state.db();
    let mut f = load_folder(&db, &folder_id)?;
    f.user_data = userdata::set(&f.user_data, WORK_KEY, meta.map(|m| json!(m)));
    db.save_meta(&mut f).map_err(err)
}

#[tauri::command]
pub fn work_overview(state: State<'_, AppState>, work_id: String) -> CmdResult<WorkOverview> {
    let db = state.db();
    let root = load_work(&db, &work_id)?;
    overview(&db, &root)
}

/// Sets what a note is in its work: `character`, `plot`, `material`, or
/// `None` for a chapter of the manuscript.
#[tauri::command]
pub fn set_note_kind(state: State<'_, AppState>, note_id: String, kind: Option<String>) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &note_id)?;
    let kind = kind.filter(|k| [KIND_CHARACTER, KIND_PLOT, KIND_MATERIAL].contains(&k.as_str()));
    n.user_data = userdata::set(&n.user_data, KIND_KEY, kind.map(|k| json!(k)));
    db.save_meta(&mut n).map_err(err)
}

#[tauri::command]
pub fn set_chapter_meta(state: State<'_, AppState>, note_id: String, meta: ChapterMeta) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &note_id)?;
    let empty = meta == ChapterMeta::default();
    n.user_data = userdata::set(&n.user_data, CHAPTER_KEY, (!empty).then(|| json!(meta)));
    db.save_meta(&mut n).map_err(err)
}

/// A new note of `kind` in the work's own notebook.
#[tauri::command]
pub fn create_work_note(
    state: State<'_, AppState>,
    work_id: String,
    kind: String,
    title: String,
) -> CmdResult<NoteDto> {
    let db = state.db();
    let root = load_work(&db, &work_id)?;
    let mut n = Note::new(&root.id, &title, "");
    if [KIND_CHARACTER, KIND_MATERIAL].contains(&kind.as_str()) {
        n.user_data = userdata::set("", KIND_KEY, Some(json!(kind)));
    }
    db.save(&mut n).map_err(err)?;
    Ok(dto(&n))
}

#[tauri::command]
pub fn get_character(state: State<'_, AppState>, note_id: String) -> CmdResult<Character> {
    let db = state.db();
    Ok(character_of(&load_note(&db, &note_id)?))
}

#[tauri::command]
pub fn set_character(state: State<'_, AppState>, note_id: String, character: Character) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &note_id)?;
    n.user_data = userdata::set(&n.user_data, KIND_KEY, Some(json!(KIND_CHARACTER)));
    n.user_data = userdata::set(&n.user_data, CHARACTER_KEY, Some(json!(character)));
    db.save(&mut n).map_err(err)
}

/// The columns a new plot board starts with.
fn template_columns(template: &str) -> Vec<PlotColumn> {
    let titles: Vec<&str> = match template {
        "threeAct" => vec![
            tr("第一幕　発端", "Act I: Setup"),
            tr("第二幕　対立", "Act II: Confrontation"),
            tr("第三幕　解決", "Act III: Resolution"),
        ],
        "heroJourney" => vec![
            tr("日常の世界", "The ordinary world"),
            tr("冒険への誘い", "The call to adventure"),
            tr("試練", "Tests and ordeals"),
            tr("最大の危機", "The ordeal"),
            tr("帰還", "The return"),
        ],
        "free" => vec![tr("プロット", "Plot")],
        _ => vec![tr("起", "Beginning"), tr("承", "Development"), tr("転", "Twist"), tr("結", "Conclusion")],
    };
    titles
        .into_iter()
        .enumerate()
        .map(|(i, t)| PlotColumn {
            id: format!("c{}", i + 1),
            title: t.to_string(),
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotDoc {
    note_id: String,
    plot: Plot,
}

/// The work's plot board, made (with `template`'s columns) if it has none.
#[tauri::command]
pub fn open_plot(state: State<'_, AppState>, work_id: String, template: Option<String>) -> CmdResult<PlotDoc> {
    let db = state.db();
    let root = load_work(&db, &work_id)?;
    if let Some(id) = overview(&db, &root)?.plot_id {
        let n = load_note(&db, &id)?;
        return Ok(PlotDoc { note_id: id, plot: plot_of(&n) });
    }
    let template = template.unwrap_or_else(|| "kishotenketsu".into());
    let plot = Plot {
        columns: template_columns(&template),
        template,
        cards: Vec::new(),
    };
    let mut n = Note::new(&root.id, tr("プロット", "Plot"), "");
    n.user_data = userdata::set("", KIND_KEY, Some(json!(KIND_PLOT)));
    n.user_data = userdata::set(&n.user_data, PLOT_KEY, Some(json!(plot)));
    n.body = plot_markdown(&db, &plot);
    db.save(&mut n).map_err(err)?;
    Ok(PlotDoc { note_id: n.id, plot })
}

#[tauri::command]
pub fn set_plot(state: State<'_, AppState>, note_id: String, plot: Plot) -> CmdResult<()> {
    let db = state.db();
    let mut n = load_note(&db, &note_id)?;
    n.user_data = userdata::set(&n.user_data, KIND_KEY, Some(json!(KIND_PLOT)));
    n.user_data = userdata::set(&n.user_data, PLOT_KEY, Some(json!(plot)));
    n.body = plot_markdown(&db, &plot);
    db.save(&mut n).map_err(err)
}

/// The plot as Markdown, for reading it elsewhere (Joplin).
fn plot_markdown(db: &Database, plot: &Plot) -> String {
    let mut out = format!(
        "> {}\n",
        tr(
            "このノートは日色のプロットボードです。カードの編集は日色で行ってください。",
            "This note is a plot board of Nisshoku; edit its cards in Nisshoku."
        )
    );
    for col in &plot.columns {
        out.push_str(&format!("\n## {}\n\n", col.title));
        let cards: Vec<&PlotCard> = plot.cards.iter().filter(|c| c.column == col.id).collect();
        if cards.is_empty() {
            out.push_str("-\n");
        }
        for c in cards {
            let mark = if c.done { "x" } else { " " };
            let mut line = format!("- [{mark}] **{}**", c.title.trim());
            if let Some(ch) = c.chapter.as_deref().and_then(|id| db.load::<Note>(id).ok().flatten()) {
                line.push_str(&format!(" （{}）", ch.title));
            }
            out.push_str(&line);
            out.push('\n');
            for l in c.text.lines().filter(|l| !l.trim().is_empty()) {
                out.push_str(&format!("  {}\n", l.trim()));
            }
        }
    }
    out
}

/// The chapters of a work in manuscript order, with the part (sub-notebook)
/// each begins, for compiling the manuscript: `(part title or "", note)`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManuscriptPart {
    title: String,
    depth: usize,
    notes: Vec<NoteDto>,
}

/// The manuscript of a work in order: (part title, its chapters), the
/// work's own notebook without a part title.
pub(crate) fn manuscript_notes(db: &Database, root: &Folder) -> CmdResult<Vec<(Option<String>, Vec<Note>)>> {
    let mut out = Vec::new();
    for (folder, depth) in work_tree(db, root)? {
        let notes: Vec<Note> = notes_in(db, &folder.id)?
            .into_iter()
            .filter(|n| kind_of(n).is_none() && !crate::notes::is_template(n))
            .collect();
        if !notes.is_empty() {
            out.push(((depth > 0).then(|| folder.title.clone()), notes));
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn work_manuscript(state: State<'_, AppState>, work_id: String) -> CmdResult<Vec<ManuscriptPart>> {
    let db = state.db();
    let root = load_work(&db, &work_id)?;
    let mut parts = Vec::new();
    for (folder, depth) in work_tree(&db, &root)? {
        let notes: Vec<NoteDto> = notes_in(&db, &folder.id)?
            .iter()
            .filter(|n| kind_of(n).is_none() && !crate::notes::is_template(n))
            .map(dto)
            .collect();
        if notes.is_empty() && depth > 0 {
            continue;
        }
        parts.push(ManuscriptPart {
            title: if depth == 0 { String::new() } else { folder.title.clone() },
            depth,
            notes,
        });
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions_prefer_the_longest_name() {
        let names = vec!["澪".to_string(), "澪ちゃん".to_string()];
        assert_eq!(count_mentions("澪ちゃんと澪が、澪ちゃんを", &names), 3);
        assert_eq!(count_mentions("誰もいない", &names), 0);
        assert_eq!(count_mentions("Mio met mio", &["Mio".to_string()]), 1);
    }

    fn db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn overview_sorts_notes_by_kind_and_counts_characters() {
        let db = db();
        let mut root = Folder::new("", "長編");
        root.user_data = userdata::set("", WORK_KEY, Some(json!(WorkMeta { target_chars: Some(1000), ..Default::default() })));
        db.save(&mut root).unwrap();
        let mut part = Folder::new(&root.id, "第一部");
        db.save(&mut part).unwrap();

        let mut mio = Note::new(&root.id, "澪", "主人公");
        mio.user_data = userdata::set("", KIND_KEY, Some(json!(KIND_CHARACTER)));
        mio.user_data = userdata::set(
            &mio.user_data,
            CHARACTER_KEY,
            Some(json!(Character { role: "protagonist".into(), aliases: vec!["みお".into()], ..Default::default() })),
        );
        db.save(&mut mio).unwrap();
        let mut memo = Note::new(&root.id, "資料", "港の地図");
        memo.user_data = userdata::set("", KIND_KEY, Some(json!(KIND_MATERIAL)));
        db.save(&mut memo).unwrap();
        let mut ch1 = Note::new(&part.id, "第一章", "澪は港へ。みおと呼ばれた。");
        ch1.user_data = userdata::set("", CHAPTER_KEY, Some(json!(ChapterMeta { status: "draft".into(), ..Default::default() })));
        db.save(&mut ch1).unwrap();
        let mut ch2 = Note::new(&part.id, "第二章", "誰もいない。");
        db.save(&mut ch2).unwrap();

        let o = overview(&db, &root).unwrap();
        assert_eq!(o.meta.target_chars, Some(1000));
        assert_eq!(o.chapter_count, 2);
        assert_eq!(o.materials.len(), 1);
        assert_eq!(o.characters.len(), 1);
        assert_eq!(o.characters[0].mentions, 2);
        assert_eq!(o.characters[0].chapters, vec![ch1.id.clone()]);
        let part_section = o.sections.iter().find(|s| s.folder_id == part.id).unwrap();
        assert_eq!(part_section.depth, 1);
        assert_eq!(part_section.chapters[0].status, "draft");
        assert_eq!(part_section.chapters[0].characters, vec![(mio.id.clone(), 2)]);
        assert_eq!(o.status_counts.get("draft"), Some(&1));
        assert_eq!(work_root(&db, &part.id).map(|f| f.id), Some(root.id.clone()));

        // The shape the front end reads (src/api.ts: WorkOverview).
        let v = serde_json::to_value(&o).unwrap();
        for k in ["id", "title", "meta", "sections", "characters", "plotId", "materials", "totalChars", "chapterCount", "statusCounts"] {
            assert!(v.get(k).is_some(), "{k}");
        }
        assert_eq!(v["meta"]["targetChars"], 1000);
        let ch = &v["sections"][1]["chapters"][0];
        for k in ["id", "title", "charCount", "status", "synopsis", "pov", "characters", "kind"] {
            assert!(ch.get(k).is_some(), "chapter {k}");
        }
        assert_eq!(ch["characters"][0][1], 2);
        let c = &v["characters"][0];
        for k in ["id", "name", "ruby", "role", "color", "aliases", "excerpt", "mentions", "chapters"] {
            assert!(c.get(k).is_some(), "character {k}");
        }
        // And what it sends back.
        let meta: ChapterMeta = serde_json::from_value(json!({"status": "done", "synopsis": "s", "pov": null})).unwrap();
        assert_eq!(meta.status, "done");
        let w: WorkMeta = serde_json::from_value(json!({"logline": "", "synopsis": "", "genre": "", "targetChars": null, "deadline": "2026-12-01"})).unwrap();
        assert_eq!(w.deadline.as_deref(), Some("2026-12-01"));
    }

    #[test]
    fn plot_reads_as_markdown() {
        let db = db();
        let plot = Plot {
            template: "free".into(),
            columns: vec![PlotColumn { id: "c1".into(), title: "起".into() }],
            cards: vec![PlotCard { id: "a".into(), column: "c1".into(), title: "出会い".into(), text: "港で\n\n灯台".into(), chapter: None, done: true }],
        };
        let md = plot_markdown(&db, &plot);
        assert!(md.contains("## 起"));
        assert!(md.contains("- [x] **出会い**\n  港で\n  灯台\n"));
    }
}
