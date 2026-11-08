//! Web page to Markdown for web clips: the main content (an `article`,
//! `main`, or the body), with headings, paragraphs, lists, quotes, code,
//! links, images and emphasis. Navigation, scripts and forms are left out.

use reqwest::Url;
use scraper::{ElementRef, Html, Node, Selector};

const SKIP: &[&str] = &[
    "script", "style", "noscript", "nav", "footer", "header", "aside", "form", "button", "svg",
    "iframe", "template", "select", "input", "textarea", "canvas",
];

/// The page's title and its main content as Markdown.
pub fn html_to_markdown(html: &str, base: &Url) -> (String, String) {
    let doc = Html::parse_document(html);
    let pick = |css: &str| {
        Selector::parse(css)
            .ok()
            .and_then(|s| doc.select(&s).next())
    };
    let title = pick("meta[property='og:title']")
        .and_then(|m| m.value().attr("content").map(str::to_string))
        .or_else(|| pick("title").map(|t| t.text().collect::<String>()))
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let root = pick("article")
        .or_else(|| pick("main"))
        .or_else(|| pick("[role='main']"))
        .or_else(|| pick("body"));
    let mut out = String::new();
    if let Some(root) = root {
        block(root, base, &mut out, 0);
    }
    (title, tidy(&out))
}

fn tidy(s: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in s.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank += 1;
            if blank == 1 && !out.is_empty() {
                out.push('\n');
            }
        } else {
            blank = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

fn absolute(base: &Url, href: &str) -> String {
    base.join(href)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| href.to_string())
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
}

/// Inline content of an element.
fn inline(el: ElementRef, base: &Url) -> String {
    let mut out = String::new();
    for child in el.children() {
        match child.value() {
            Node::Text(t) => {
                let text = t.text.replace(['\n', '\t', '\r'], " ");
                out.push_str(&escape(&text));
            }
            Node::Element(e) => {
                let Some(c) = ElementRef::wrap(child) else {
                    continue;
                };
                let name = e.name();
                if SKIP.contains(&name) {
                    continue;
                }
                let inner = || inline(c, base).trim().to_string();
                match name {
                    "strong" | "b" => {
                        let t = inner();
                        if !t.is_empty() {
                            out.push_str(&format!("**{t}**"));
                        }
                    }
                    "em" | "i" => {
                        let t = inner();
                        if !t.is_empty() {
                            out.push_str(&format!("*{t}*"));
                        }
                    }
                    "code" => out.push_str(&format!("`{}`", c.text().collect::<String>())),
                    "br" => out.push_str("  \n"),
                    "a" => {
                        let t = inner();
                        match e.attr("href") {
                            Some(h)
                                if !t.is_empty()
                                    && !h.starts_with("javascript:")
                                    && !h.starts_with('#') =>
                            {
                                out.push_str(&format!("[{t}]({})", absolute(base, h)))
                            }
                            _ => out.push_str(&t),
                        }
                    }
                    "img" => {
                        if let Some(src) = e.attr("src").or_else(|| e.attr("data-src")) {
                            let alt = e.attr("alt").unwrap_or("");
                            out.push_str(&format!("![{}]({})", escape(alt), absolute(base, src)));
                        }
                    }
                    "ruby" => {
                        // 漢字《かんじ》 notation.
                        let base_text: String = c
                            .children()
                            .filter_map(|n| match n.value() {
                                Node::Text(t) => Some(t.text.to_string()),
                                _ => None,
                            })
                            .collect();
                        let rt: String = Selector::parse("rt")
                            .ok()
                            .map(|s| c.select(&s).flat_map(|r| r.text()).collect())
                            .unwrap_or_default();
                        if rt.is_empty() {
                            out.push_str(&base_text);
                        } else {
                            out.push_str(&format!("|{}《{}》", base_text.trim(), rt.trim()));
                        }
                    }
                    "rt" | "rp" => {}
                    _ => out.push_str(&inline(c, base)),
                }
            }
            _ => {}
        }
    }
    out.split(' ')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn has_block_children(el: ElementRef) -> bool {
    el.children().any(|c| {
        c.value().as_element().is_some_and(|e| {
            matches!(
                e.name(),
                "p" | "div"
                    | "section"
                    | "ul"
                    | "ol"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "blockquote"
                    | "pre"
                    | "table"
                    | "figure"
                    | "article"
                    | "main"
            )
        })
    })
}

fn block(el: ElementRef, base: &Url, out: &mut String, depth: usize) {
    if depth > 40 {
        return;
    }
    for child in el.children() {
        match child.value() {
            Node::Text(t) => {
                let text = t.text.trim();
                if !text.is_empty() {
                    out.push_str(&escape(text));
                    out.push('\n');
                }
            }
            Node::Element(e) => {
                let Some(c) = ElementRef::wrap(child) else {
                    continue;
                };
                let name = e.name();
                if SKIP.contains(&name) {
                    continue;
                }
                match name {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let level = name[1..].parse::<usize>().unwrap_or(2).clamp(1, 4);
                        let t = inline(c, base);
                        if !t.trim().is_empty() {
                            out.push_str(&format!("\n{} {}\n\n", "#".repeat(level), t.trim()));
                        }
                    }
                    "p" => {
                        let t = inline(c, base);
                        if !t.trim().is_empty() {
                            out.push_str(&format!("\n{}\n\n", t.trim()));
                        }
                    }
                    "ul" | "ol" => {
                        out.push('\n');
                        let mut i = 1;
                        for li in c.children().filter_map(ElementRef::wrap) {
                            if li.value().name() != "li" {
                                continue;
                            }
                            let marker = if name == "ol" {
                                format!("{i}.")
                            } else {
                                "-".into()
                            };
                            out.push_str(&format!(
                                "{}{marker} {}\n",
                                "  ".repeat(depth.min(3) / 2),
                                inline(li, base).trim()
                            ));
                            i += 1;
                        }
                        out.push('\n');
                    }
                    "blockquote" => {
                        let mut inner = String::new();
                        block(c, base, &mut inner, depth + 1);
                        out.push('\n');
                        for line in tidy(&inner).lines() {
                            out.push_str(&format!("> {line}\n"));
                        }
                        out.push('\n');
                    }
                    "pre" => {
                        out.push_str(&format!(
                            "\n```\n{}\n```\n\n",
                            c.text().collect::<String>().trim_end()
                        ));
                    }
                    "hr" => out.push_str("\n---\n\n"),
                    "img" => out.push_str(&format!("\n{}\n\n", inline(el, base).trim())),
                    "table" => {
                        let rows: Vec<Vec<String>> = Selector::parse("tr")
                            .ok()
                            .map(|s| {
                                c.select(&s)
                                    .map(|tr| {
                                        tr.children()
                                            .filter_map(ElementRef::wrap)
                                            .filter(|td| matches!(td.value().name(), "td" | "th"))
                                            .map(|td| {
                                                inline(td, base)
                                                    .replace('|', "\\|")
                                                    .trim()
                                                    .to_string()
                                            })
                                            .collect()
                                    })
                                    .filter(|r: &Vec<String>| !r.is_empty())
                                    .collect()
                            })
                            .unwrap_or_default();
                        if let Some(first) = rows.first() {
                            let cols = rows.iter().map(|r| r.len()).max().unwrap_or(first.len());
                            let line = |r: &Vec<String>| {
                                let mut cells = r.clone();
                                cells.resize(cols, String::new());
                                format!("| {} |", cells.join(" | "))
                            };
                            out.push('\n');
                            out.push_str(&line(first));
                            out.push('\n');
                            out.push_str(&format!("|{}\n", " --- |".repeat(cols)));
                            for r in &rows[1..] {
                                out.push_str(&line(r));
                                out.push('\n');
                            }
                            out.push('\n');
                        }
                    }
                    _ if has_block_children(c) => block(c, base, out, depth + 1),
                    _ => {
                        let t = inline(c, base);
                        if !t.trim().is_empty() {
                            out.push_str(&format!("\n{}\n\n", t.trim()));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_the_main_content() {
        let html = r#"<html><head><title>記事の題</title></head><body>
            <nav><a href="/">ホーム</a></nav>
            <article><h1>見出し</h1><p>本文の<strong>大事</strong>な所と<a href="/x">リンク</a>。</p>
            <ul><li>一</li><li>二</li></ul><blockquote><p>引用</p></blockquote>
            <p><ruby>漢字<rt>かんじ</rt></ruby>と<img src="a.png" alt="図"></p>
            <table><tr><th>名</th><th>年</th></tr><tr><td>澪</td><td>17</td></tr></table>
            <script>alert(1)</script></article></body></html>"#;
        let base = Url::parse("https://example.com/post/1").unwrap();
        let (title, md) = html_to_markdown(html, &base);
        assert_eq!(title, "記事の題");
        assert!(!md.contains("ホーム") && !md.contains("alert"), "{md}");
        assert!(md.contains("# 見出し"), "{md}");
        assert!(
            md.contains("本文の**大事**な所と[リンク](https://example.com/x)。"),
            "{md}"
        );
        assert!(md.contains("- 一\n- 二"), "{md}");
        assert!(md.contains("> 引用"), "{md}");
        assert!(
            md.contains("|漢字《かんじ》と![図](https://example.com/post/a.png)"),
            "{md}"
        );
        assert!(
            md.contains("| 名 | 年 |\n| --- | --- |\n| 澪 | 17 |"),
            "{md}"
        );
    }
}
