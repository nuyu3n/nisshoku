//! Text measures for writing: character counts that ignore Markdown syntax,
//! excerpts for lists.

/// Text folded for searching: full- and half-width forms alike (NFKC),
/// katakana as hiragana, and lower case, so that `ﾆｯｼｮｸ`, `ニッショク` and
/// `にっしょく` find each other.
pub fn search_fold(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    text.nfkc()
        .map(|c| match c {
            '\u{30A1}'..='\u{30F6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
            c => c,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

/// Whether `text` contains every word of `query` (folded as above).
pub fn matches_query(text: &str, query: &str) -> bool {
    let hay = search_fold(text);
    search_fold(query).split_whitespace().all(|t| hay.contains(t))
}

/// Characters of prose in a Markdown body: whitespace, Markdown markers,
/// link targets and ruby notation markers are not counted.
pub fn count_chars(markdown: &str) -> usize {
    plain_text(markdown)
        .chars()
        .filter(|c| !c.is_whitespace())
        .count()
}

/// The body as plain text, roughly as a reader sees it.
pub fn plain_text(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut in_code_fence = false;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if trimmed.chars().all(|c| matches!(c, '-' | '*' | '_' | ' ')) && trimmed.len() >= 3 {
            continue; // horizontal rule
        }
        // Table delimiter rows (| --- | :-: |).
        if trimmed.starts_with('|') && trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
            continue;
        }
        let mut rest = trimmed;
        // Block markers.
        loop {
            let before = rest;
            rest = rest.trim_start_matches('>').trim_start();
            if let Some(r) = rest
                .strip_prefix("- [ ] ")
                .or_else(|| rest.strip_prefix("- [x] "))
            {
                rest = r;
            }
            for marker in ["- ", "* ", "+ "] {
                if let Some(r) = rest.strip_prefix(marker) {
                    rest = r;
                }
            }
            let hashes = rest.chars().take_while(|c| *c == '#').count();
            if hashes > 0 && rest[hashes..].starts_with(' ') {
                rest = &rest[hashes + 1..];
            }
            let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 && rest[digits..].starts_with(". ") {
                rest = &rest[digits + 2..];
            }
            if rest == before {
                break;
            }
        }
        out.push_str(&inline_text(rest));
        out.push('\n');
    }
    out
}

/// Removes inline Markdown: emphasis markers, code ticks, link and image
/// targets, HTML tags and the ruby notation `|漢字《かんじ》` (keeping both).
fn inline_text(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '*' | '_' | '`' | '~' | '|' | '｜' | '《' | '》' => {}
            '\\' if i + 1 < chars.len() => {
                out.push(chars[i + 1]);
                i += 1;
            }
            '!' if chars.get(i + 1) == Some(&'[') => {}
            ']' if chars.get(i + 1) == Some(&'(') => {
                // Skip the link target.
                let mut j = i + 2;
                while j < chars.len() && chars[j] != ')' {
                    j += 1;
                }
                i = j;
            }
            '[' => {}
            '<' => {
                let mut j = i + 1;
                while j < chars.len() && chars[j] != '>' {
                    j += 1;
                }
                if j < chars.len() {
                    i = j;
                } else {
                    out.push(c);
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    out
}

/// The start of the body as plain text, for note lists.
/// The ruby of `|漢字《かんじ》` (or `漢字《かんじ》`) reduced to its base text.
fn strip_ruby(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // The bar only marks where the base starts.
            '|' | '｜'
                if chars
                    .clone()
                    .take_while(|c| !c.is_whitespace())
                    .any(|c| c == '《') => {}
            '《' => {
                let rest: String = chars.clone().collect();
                match rest.find('》') {
                    Some(end) => {
                        for _ in 0..rest[..end].chars().count() + 1 {
                            chars.next();
                        }
                    }
                    None => out.push(c),
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// The start of a note's text for lists: without a first heading that only
/// repeats the title, and with ruby reduced to its base text.
pub fn note_excerpt(title: &str, markdown: &str, max_chars: usize) -> String {
    let mut body = markdown.trim_start();
    if let Some(line) = body.lines().next() {
        let heading = line.trim_start_matches('#');
        if heading.len() < line.len() && !title.trim().is_empty() && heading.trim() == title.trim()
        {
            body = &body[line.len()..];
        }
    }
    excerpt(&strip_ruby(body), max_chars)
}

pub fn excerpt(markdown: &str, max_chars: usize) -> String {
    let text = plain_text(markdown);
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = joined.chars().take(max_chars).collect();
    if joined.chars().count() > max_chars {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_prose_only() {
        assert_eq!(count_chars("# 第一章\n\n吾輩は**猫**である。"), 3 + 8);
        assert_eq!(count_chars("- [x] 買い物\n> 引用 です"), 3 + 4);
        assert_eq!(
            count_chars("[リンク](https://example.com) と ![絵](a.png)"),
            3 + 1 + 1
        );
        assert_eq!(count_chars("|漢字《かんじ》"), 5);
        assert_eq!(count_chars("---\n\n1. 一\n2. 二"), 2);
    }

    #[test]
    fn search_ignores_width_and_kana() {
        assert!(matches_query("日食のﾉｰﾄ", "のーと"));
        assert!(matches_query("ニッショク", "にっしょく"));
        assert!(matches_query("ＡＢＣ 123", "abc １２３"));
        assert!(!matches_query("日食", "月食"));
    }

    #[test]
    fn excerpts() {
        assert_eq!(excerpt("# 題\n\n本文の\n始まり", 5), "題 本文の…");
        assert_eq!(note_excerpt("題", "# 題\n\n本文", 10), "本文");
        assert_eq!(note_excerpt("別", "# 題\n\n本文", 10), "題 本文");
        assert_eq!(
            note_excerpt("", "|祖父《そふ》は｜灯台《とうだい》守", 20),
            "祖父は灯台守"
        );
        assert_eq!(note_excerpt("", "二人《ふたり》 | 《未完》", 20), "二人");
    }
}
