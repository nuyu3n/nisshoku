//! The text format items are stored in on a sync target.
//!
//! ```text
//! <title>
//!
//! <body (notes only, may span many lines)>
//!
//! id: 0123456789abcdef0123456789abcdef
//! parent_id: ...
//! updated_time: 2024-01-02T03:04:05.678Z
//! ...
//! type_: 1
//! ```
//!
//! Properties form the last block of the file, one `key: value` per line, with
//! newlines in values escaped as `\n`. Everything above the last blank line is
//! the title (first line) and, for notes, the body (from the third line on).
//! Items without a title (such as note-tag links) consist of the property block
//! only.

use crate::error::{Error, Result};
use crate::item_type::ItemType;

#[derive(Debug, Clone, PartialEq)]
pub struct RawItem {
    /// Numeric item type. Kept as a raw number so that items of types this
    /// version does not know about can still be parsed and skipped.
    pub item_type: i64,
    pub title: Option<String>,
    pub body: Option<String>,
    /// Properties in file order, excluding `type_`. Values are as written in the
    /// file (still escaped).
    pub props: Vec<(String, String)>,
}

impl RawItem {
    pub fn new(item_type: ItemType) -> Self {
        Self {
            item_type: item_type.as_i64(),
            title: None,
            body: None,
            props: Vec::new(),
        }
    }

    pub fn known_type(&self) -> Option<ItemType> {
        ItemType::from_i64(self.item_type)
    }

    pub fn prop(&self, key: &str) -> Option<&str> {
        self.props
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn to_text(&self) -> String {
        let mut parts: Vec<&str> = Vec::with_capacity(3);
        if let Some(title) = &self.title {
            parts.push(title);
        }
        if let Some(body) = &self.body
            && !body.is_empty()
        {
            parts.push(body);
        }
        let mut props: Vec<String> = self
            .props
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        props.push(format!("type_: {}", self.item_type));
        let props = props.join("\n");
        parts.push(&props);
        parts.join("\n\n")
    }

    pub fn parse(content: &str) -> Result<Self> {
        // Be lenient about trailing newlines added by editors or other tools.
        let content = content.trim_end_matches(['\n', '\r']);
        let lines: Vec<&str> = content.split('\n').collect();

        let mut props_rev = Vec::new();
        let mut separator = None;
        for (i, line) in lines.iter().enumerate().rev() {
            let line = line.trim();
            if line.is_empty() {
                separator = Some(i);
                break;
            }
            let colon = line
                .find(':')
                .ok_or_else(|| Error::Parse(format!("invalid property line: {line:?}")))?;
            props_rev.push((
                line[..colon].trim().to_string(),
                line[colon + 1..].trim().to_string(),
            ));
        }

        let mut item_type = None;
        let mut props = Vec::with_capacity(props_rev.len());
        for (k, v) in props_rev.into_iter().rev() {
            if k == "type_" {
                item_type = Some(
                    v.parse::<i64>()
                        .map_err(|_| Error::Parse(format!("invalid type_: {v:?}")))?,
                );
            } else {
                props.push((k, v));
            }
        }
        let item_type = item_type.ok_or_else(|| Error::Parse("missing type_ property".into()))?;

        let text_lines = separator.map_or(&lines[..0], |sep| &lines[..sep]);
        let mut title = None;
        let mut body = None;
        if let Some(first) = text_lines.first() {
            title = Some(first.to_string());
            body = Some(
                text_lines
                    .get(2..)
                    .map(|l| l.join("\n"))
                    .unwrap_or_default(),
            );
        }
        if item_type != ItemType::Note.as_i64() {
            body = None;
        } else if body.is_none() {
            body = Some(String::new());
        }

        Ok(Self {
            item_type,
            title,
            body,
            props,
        })
    }
}

/// Escapes a property value so that it fits on one line.
pub fn escape_prop(s: &str) -> String {
    s.replace('\n', "\\n").replace('\r', "\\r")
}

pub fn unescape_prop(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\r", "\r")
}

/// Revision diffs (JSON text) are written as a JSON string literal in which
/// the backslash of every `\n` / `\r` escape is doubled once more, as Joplin does.
pub fn encode_diff_prop(diff: &str) -> String {
    if diff.is_empty() {
        return String::new();
    }
    let quoted = serde_json::to_string(diff).expect("serializable");
    quoted.replace("\\n", "\\\\n").replace("\\r", "\\\\r")
}

pub fn decode_diff_prop(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    let unescaped = value.replace("\\\\n", "\\n").replace("\\\\r", "\\r");
    serde_json::from_str::<String>(&unescaped).unwrap_or_else(|_| value.to_string())
}

pub fn format_float(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

pub fn parse_int(s: &str) -> i64 {
    let s = s.trim();
    s.parse::<i64>()
        .ok()
        .or_else(|| s.parse::<f64>().ok().map(|f| f as i64))
        .unwrap_or(0)
}

pub fn parse_float(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_round_trip() {
        let mut raw = RawItem::new(ItemType::Note);
        raw.title = Some("Title".into());
        raw.body = Some("line 1\n\nline 3\n".into());
        raw.props.push(("id".into(), "abc".into()));
        let text = raw.to_text();
        assert_eq!(text, "Title\n\nline 1\n\nline 3\n\n\nid: abc\ntype_: 1");
        assert_eq!(RawItem::parse(&text).unwrap(), raw);
    }

    #[test]
    fn empty_title_and_body() {
        let mut raw = RawItem::new(ItemType::Note);
        raw.title = Some(String::new());
        raw.body = Some(String::new());
        raw.props.push(("id".into(), "abc".into()));
        let text = raw.to_text();
        assert_eq!(text, "\n\nid: abc\ntype_: 1");
        assert_eq!(RawItem::parse(&text).unwrap(), raw);
    }

    #[test]
    fn item_without_title() {
        let mut raw = RawItem::new(ItemType::NoteTag);
        raw.props.push(("id".into(), "abc".into()));
        raw.props.push(("note_id".into(), "def".into()));
        let text = raw.to_text();
        assert_eq!(text, "id: abc\nnote_id: def\ntype_: 6");
        assert_eq!(RawItem::parse(&text).unwrap(), raw);
    }

    #[test]
    fn body_that_looks_like_props() {
        let mut raw = RawItem::new(ItemType::Note);
        raw.title = Some("T".into());
        raw.body = Some("key: value\nother: thing".into());
        raw.props.push(("id".into(), "abc".into()));
        let parsed = RawItem::parse(&raw.to_text()).unwrap();
        assert_eq!(parsed.body.as_deref(), Some("key: value\nother: thing"));
        assert_eq!(parsed.props.len(), 1);
    }

    #[test]
    fn missing_type_is_an_error() {
        assert!(RawItem::parse("Title\n\nid: abc").is_err());
    }
}
