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
        // (`snake_case` and "10~20" are text, not emphasis or strike-through.)
        let word = |c: Option<&char>| c.is_some_and(|c| c.is_alphanumeric());
        match c {
            '_' if i > 0 && word(chars.get(i - 1)) && word(chars.get(i + 1)) => out.push(c),
            '~' if chars.get(i + 1) != Some(&'~') && (i == 0 || chars[i - 1] != '~') => out.push(c),
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
    excerpt(&strip_math(&strip_ruby(body)), max_chars)
}

/// For a search result: the text around the first word of `query` found
/// (folded as [`search_fold`]), or the start when only the title matched.
pub fn search_excerpt(title: &str, markdown: &str, query: &str, max_chars: usize) -> String {
    let full = note_excerpt(title, markdown, usize::MAX);
    let chars: Vec<char> = full.chars().collect();
    // The folded text, with where each of its characters came from.
    let mut folded = String::new();
    let mut origin = Vec::new();
    for (i, c) in chars.iter().enumerate() {
        for f in search_fold(&c.to_string()).chars() {
            folded.push(f);
            origin.push(i);
        }
    }
    let folded: Vec<char> = folded.chars().collect();
    let found = search_fold(query).split_whitespace().find_map(|word| {
        let word: Vec<char> = word.chars().collect();
        (0..folded.len().saturating_sub(word.len() - 1)).find(|&i| folded[i..].starts_with(&word))
    });
    let start = match found {
        // A little of what comes before, for context.
        Some(at) if origin[at] > 12 => origin[at] - 12,
        _ => return note_excerpt(title, markdown, max_chars),
    };
    let mut out: String = "…".to_string();
    out.extend(chars[start..].iter().take(max_chars));
    if chars.len() - start > max_chars {
        out.push('…');
    }
    out
}

/// TeX (`$…$`, `$$…$$`) shown as a mark in excerpts: its source reads as noise.
fn strip_math(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('$') {
        let display = rest[i..].starts_with("$$");
        let open = if display { 2 } else { 1 };
        let after = &rest[i + open..];
        let close = if display { after.find("$$") } else { after.find('$') };
        match close {
            // (A lone `$`, as in a price, stays.)
            Some(end) if end > 0 && (display || !after[..end].contains('\n')) => {
                out.push_str(&rest[..i]);
                out.push_str(" ［数式］ ");
                rest = &after[end + open..];
            }
            _ => {
                out.push_str(&rest[..i + 1]);
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    out
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
    fn math_in_excerpts() {
        assert_eq!(note_excerpt("", "距離は $d \\approx 3.57$ で", 40), "距離は ［数式］ で");
        assert_eq!(note_excerpt("", "$$\nx^2\n$$\n後", 40), "［数式］ 後");
        assert_eq!(note_excerpt("", "100$ です", 40), "100$ です");
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
        let long = format!("{}見つけたい言葉のあと", "前置き".repeat(20));
        let found = search_excerpt("", &long, "ツケタイ", 20);
        assert!(found.starts_with('…') && found.contains("見つけたい") && found.ends_with('…'), "{found}");
        assert_eq!(search_excerpt("", "短い本文", "本文", 12), "短い本文");
        assert_eq!(excerpt("edited_on_pc _em_ 10~20 ~~old~~", 40), "edited_on_pc em 10~20 old");
    }
}
