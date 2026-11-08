//! A work written out as an EPUB 3 book, set vertically (縦書き, right to
//! left) or horizontally: a chapter per manuscript note, the parts (its
//! sub-notebooks) as headings, ruby (`|漢字《かんじ》`) as ruby.

use std::io::Write;

use nisshoku_core::{Database, Folder};
use tauri::State;
use zip::write::SimpleFileOptions;

use crate::notes::{CmdResult, err};
use crate::state::AppState;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn is_kanji(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '々' | '〆' | 'ヵ' | 'ヶ')
}

/// One line of prose as XHTML: escaped, with ruby and emphasis markers
/// (`|漢字《かんじ》`, `漢字《かんじ》`, `《《傍点》》`) turned into markup and
/// other Markdown inline markers dropped.
pub fn inline(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // 《《emphasis dots》》
        if c == '《' && chars.get(i + 1) == Some(&'《') {
            let rest: String = chars[i + 2..].iter().collect();
            if let Some(end) = rest.find("》》") {
                let text = &rest[..end];
                out.push_str(&format!("<em class=\"sesame\">{}</em>", esc(text)));
                i += 2 + text.chars().count() + 2;
                continue;
            }
        }
        // |base《ruby》
        if c == '|' || c == '｜' {
            let rest: String = chars[i + 1..].iter().collect();
            if let (Some(open), Some(close)) = (rest.find('《'), rest.find('》'))
                && open < close
                && !rest[..open].contains(['|', '｜', '\n'])
            {
                let base = &rest[..open];
                let ruby = &rest[open + '《'.len_utf8()..close];
                out.push_str(&format!("<ruby>{}<rt>{}</rt></ruby>", esc(base), esc(ruby)));
                i += 1 + rest[..close].chars().count() + 1;
                continue;
            }
        }
        // 漢字《ruby》: the run of kanji before it is the base.
        if c == '《' {
            let rest: String = chars[i + 1..].iter().collect();
            if let Some(close) = rest.find('》') {
                let base_len = out_kanji_tail(&out);
                if base_len > 0 {
                    let cut = out.len() - base_len;
                    let base = out[cut..].to_string();
                    out.truncate(cut);
                    out.push_str(&format!("<ruby>{base}<rt>{}</rt></ruby>", esc(&rest[..close])));
                    i += 1 + rest[..close].chars().count() + 1;
                    continue;
                }
            }
        }
        match c {
            '*' | '_' | '`' => {}
            '\\' if i + 1 < chars.len() => {
                out.push_str(&esc(&chars[i + 1].to_string()));
                i += 1;
            }
            c => out.push_str(&esc(&c.to_string())),
        }
        i += 1;
    }
    out
}

/// Bytes at the end of `s` that are kanji (the base of a ruby without a bar).
fn out_kanji_tail(s: &str) -> usize {
    s.chars().rev().take_while(|c| is_kanji(*c)).map(char::len_utf8).sum()
}

/// A note's Markdown body as XHTML paragraphs.
pub fn body_xhtml(markdown: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for line in markdown.lines() {
        let t = line.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            out.push_str(&format!("<p>{}</p>\n", esc(line)));
            continue;
        }
        if t.is_empty() {
            continue;
        }
        if t.chars().all(|c| matches!(c, '-' | '*' | '_' | ' ')) && t.len() >= 3 {
            out.push_str("<p class=\"break\">＊</p>\n");
            continue;
        }
        // Images and attachments are not part of the book.
        if t.starts_with("![") {
            continue;
        }
        let hashes = t.chars().take_while(|c| *c == '#').count();
        if hashes > 0 && t[hashes..].starts_with(' ') {
            let level = (hashes + 1).min(4);
            out.push_str(&format!("<h{level}>{}</h{level}>\n", inline(t[hashes..].trim())));
            continue;
        }
        let t = t.trim_start_matches('>').trim_start();
        let t = t
            .strip_prefix("- [ ] ")
            .or_else(|| t.strip_prefix("- [x] "))
            .or_else(|| t.strip_prefix("- "))
            .unwrap_or(t);
        // Links: their text only.
        let mut text = String::new();
        let mut rest = t;
        while let Some(i) = rest.find("](") {
            let Some(end) = rest[i..].find(')') else { break };
            text.push_str(&rest[..i].replacen('[', "", 1));
            rest = &rest[i + end + 1..];
        }
        text.push_str(rest);
        out.push_str(&format!("<p>{}</p>\n", inline(&text)));
    }
    out
}

pub struct Chapter {
    pub title: String,
    pub part: Option<String>,
    pub body: String,
}

fn page(title: &str, body: &str, lang: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{lang}\" lang=\"{lang}\">\n<head><meta charset=\"UTF-8\"/><title>{}</title><link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/></head>\n<body>\n{body}</body>\n</html>\n",
        esc(title)
    )
}

/// The book as EPUB bytes.
pub fn build(title: &str, author: &str, chapters: &[Chapter], vertical: bool, lang: &str) -> std::io::Result<Vec<u8>> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let io = std::io::Error::other;
    // The mimetype comes first, uncompressed (the format requires it).
    zip.start_file("mimetype", stored).map_err(io)?;
    zip.write_all(b"application/epub+zip")?;
    zip.start_file("META-INF/container.xml", deflated).map_err(io)?;
    zip.write_all(
        b"<?xml version=\"1.0\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>\n",
    )?;
    let css = format!(
        "html {{ {} }}\nbody {{ font-family: serif; line-height: 1.8; margin: 0; }}\np {{ margin: 0; text-indent: 1em; }}\np.break {{ text-align: center; text-indent: 0; margin: 1em 0; }}\nh1, h2, h3, h4 {{ font-weight: bold; margin: 2em 0 1em; }}\nem.sesame {{ font-style: normal; text-emphasis-style: sesame; -epub-text-emphasis-style: sesame; }}\n.title {{ text-align: center; margin-top: 30%; }}\n",
        if vertical {
            "writing-mode: vertical-rl; -epub-writing-mode: vertical-rl; -webkit-writing-mode: vertical-rl;"
        } else {
            ""
        }
    );
    zip.start_file("OEBPS/style.css", deflated).map_err(io)?;
    zip.write_all(css.as_bytes())?;

    let mut manifest = String::new();
    let mut spine = String::new();
    let mut nav = String::new();
    let title_page = page(
        title,
        &format!("<h1 class=\"title\">{}</h1>\n<p class=\"title\">{}</p>\n", esc(title), esc(author)),
        lang,
    );
    zip.start_file("OEBPS/title.xhtml", deflated).map_err(io)?;
    zip.write_all(title_page.as_bytes())?;
    manifest.push_str("<item id=\"title\" href=\"title.xhtml\" media-type=\"application/xhtml+xml\"/>\n");
    spine.push_str("<itemref idref=\"title\"/>\n");
    let mut last_part: Option<&str> = None;
    for (i, ch) in chapters.iter().enumerate() {
        let name = format!("c{:04}.xhtml", i + 1);
        let mut body = String::new();
        if ch.part.as_deref() != last_part {
            if let Some(p) = &ch.part {
                body.push_str(&format!("<h1>{}</h1>\n", inline(p)));
            }
            last_part = ch.part.as_deref();
        }
        if !ch.title.is_empty() {
            body.push_str(&format!("<h2>{}</h2>\n", inline(&ch.title)));
        }
        body.push_str(&body_xhtml(&ch.body));
        zip.start_file(format!("OEBPS/{name}"), deflated).map_err(io)?;
        zip.write_all(page(&ch.title, &body, lang).as_bytes())?;
        manifest.push_str(&format!("<item id=\"c{i}\" href=\"{name}\" media-type=\"application/xhtml+xml\"/>\n"));
        spine.push_str(&format!("<itemref idref=\"c{i}\"/>\n"));
        let label = if ch.title.is_empty() { format!("{}", i + 1) } else { ch.title.clone() };
        nav.push_str(&format!("<li><a href=\"{name}\">{}</a></li>\n", esc(&label)));
    }
    let nav_page = page(
        "目次",
        &format!("<nav epub:type=\"toc\" id=\"toc\"><h1>目次</h1><ol>\n{nav}</ol></nav>\n"),
        lang,
    );
    zip.start_file("OEBPS/nav.xhtml", deflated).map_err(io)?;
    zip.write_all(nav_page.as_bytes())?;
    manifest.push_str("<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n<item id=\"css\" href=\"style.css\" media-type=\"text/css\"/>\n");
    let id = nisshoku_core::ids::new_id();
    let modified = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let opf = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"bookid\" xml:lang=\"{lang}\">\n<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n<dc:identifier id=\"bookid\">urn:uuid:{id}</dc:identifier>\n<dc:title>{}</dc:title>\n<dc:creator>{}</dc:creator>\n<dc:language>{lang}</dc:language>\n<meta property=\"dcterms:modified\">{modified}</meta>\n</metadata>\n<manifest>\n{manifest}</manifest>\n<spine{}>\n{spine}</spine>\n</package>\n",
        esc(title),
        esc(author),
        if vertical { " page-progression-direction=\"rtl\"" } else { "" }
    );
    zip.start_file("OEBPS/content.opf", deflated).map_err(io)?;
    zip.write_all(opf.as_bytes())?;
    Ok(zip.finish().map_err(io)?.into_inner())
}

fn chapters_of(db: &Database, work_id: &str) -> CmdResult<(Folder, Vec<Chapter>)> {
    let root = crate::notes::load_folder(db, work_id)?;
    let parts = crate::works::manuscript_notes(db, &root)?;
    let chapters = parts
        .into_iter()
        .flat_map(|(part, notes)| {
            notes.into_iter().map(move |n| Chapter {
                title: n.title,
                part: part.clone(),
                body: n.body,
            })
        })
        .collect();
    Ok((root, chapters))
}

/// Writes the work `work_id` as an EPUB book to `path`. Returns the number of chapters.
#[tauri::command]
pub fn export_epub<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    work_id: String,
    path: String,
    vertical: bool,
    author: Option<String>,
) -> CmdResult<usize> {
    let (root, chapters) = chapters_of(&state.db(), &work_id)?;
    let lang = if crate::i18n::is_english() { "en" } else { "ja" };
    let bytes = build(&root.title, author.as_deref().unwrap_or(""), &chapters, vertical, lang).map_err(err)?;
    crate::files::write(&app, &path, &bytes).map_err(err)?;
    Ok(chapters.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruby_and_emphasis() {
        assert_eq!(inline("|祖父《そふ》は"), "<ruby>祖父<rt>そふ</rt></ruby>は");
        assert_eq!(inline("灯台《とうだい》守"), "<ruby>灯台<rt>とうだい</rt></ruby>守");
        assert_eq!(inline("《《ここ》》だ"), "<em class=\"sesame\">ここ</em>だ");
        assert_eq!(inline("a < b **強く**"), "a &lt; b 強く");
        assert_eq!(inline("《未完》"), "《未完》");
    }

    #[test]
    fn a_book_is_a_zip_with_the_mimetype_first() {
        let chapters = vec![Chapter { title: "第一章".into(), part: None, body: "# 見出し\n\n本文。\n\n---\n\n次。".into() }];
        let bytes = build("題", "著者", &chapters, true, "ja").unwrap();
        assert_eq!(&bytes[30..50], b"mimetypeapplication/");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut ch = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("OEBPS/c0001.xhtml").unwrap(), &mut ch).unwrap();
        assert!(ch.contains("<h2>第一章</h2>") && ch.contains("<p>本文。</p>") && ch.contains("class=\"break\""));
        let mut opf = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("OEBPS/content.opf").unwrap(), &mut opf).unwrap();
        assert!(opf.contains("page-progression-direction=\"rtl\""));
    }
}
