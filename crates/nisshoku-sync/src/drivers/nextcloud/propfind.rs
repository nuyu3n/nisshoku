//! Parsing of WebDAV `PROPFIND` (multistatus) responses.

use quick_xml::Reader;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::Event;

pub const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop><d:getlastmodified/><d:getetag/><d:resourcetype/></d:prop>
</d:propfind>"#;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Entry {
    /// The `href` as sent by the server (percent-encoded).
    pub href: String,
    pub last_modified: Option<i64>,
    pub etag: Option<String>,
    pub is_dir: bool,
}

#[derive(Default)]
struct PropStat {
    last_modified: Option<i64>,
    etag: Option<String>,
    is_dir: bool,
    ok: bool,
}

pub fn parse_multistatus(xml: &str) -> Result<Vec<Entry>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut entry: Option<Entry> = None;
    let mut propstat: Option<PropStat> = None;
    let mut text = String::new();

    loop {
        let event = reader.read_event().map_err(|e| e.to_string())?;
        match event {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_string();
                match name.as_str() {
                    "response" => entry = Some(Entry::default()),
                    "propstat" => propstat = Some(PropStat::default()),
                    "collection" => mark_dir(&stack, &mut propstat),
                    _ => {}
                }
                stack.push(name);
                text.clear();
            }
            Event::Empty(e) => {
                if e.local_name().as_ref() == "collection" {
                    mark_dir(&stack, &mut propstat);
                }
            }
            Event::Text(e) => text.push_str(&e.into_inner()),
            Event::CData(e) => text.push_str(&e.into_inner()),
            Event::GeneralRef(e) => {
                if e.is_char_ref() {
                    if let Ok(Some(c)) = e.resolve_char_ref() {
                        text.push(c);
                    }
                } else if let Some(s) = resolve_predefined_entity(&e.into_inner()) {
                    text.push_str(s);
                }
            }
            Event::End(e) => {
                let name = e.local_name().as_ref().to_string();
                stack.pop();
                let value = std::mem::take(&mut text);
                let value = value.trim();
                match name.as_str() {
                    "href" if stack.last().is_some_and(|p| p == "response") => {
                        if let Some(entry) = &mut entry {
                            entry.href = value.to_string();
                        }
                    }
                    "getlastmodified" => {
                        if let Some(ps) = &mut propstat {
                            ps.last_modified = httpdate::parse_http_date(value)
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_millis() as i64);
                        }
                    }
                    "getetag" => {
                        if let Some(ps) = &mut propstat {
                            ps.etag = (!value.is_empty()).then(|| value.to_string());
                        }
                    }
                    "status" => {
                        if let Some(ps) = &mut propstat {
                            ps.ok = value.split_whitespace().nth(1) == Some("200");
                        }
                    }
                    "propstat" => {
                        if let (Some(ps), Some(entry)) = (propstat.take(), &mut entry)
                            && ps.ok
                        {
                            entry.last_modified = ps.last_modified.or(entry.last_modified);
                            entry.etag = ps.etag.or(entry.etag.take());
                            entry.is_dir |= ps.is_dir;
                        }
                    }
                    "response" => {
                        if let Some(entry) = entry.take() {
                            out.push(entry);
                        }
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

fn mark_dir(stack: &[String], propstat: &mut Option<PropStat>) {
    if stack.last().is_some_and(|p| p == "resourcetype")
        && let Some(ps) = propstat
    {
        ps.is_dir = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape of a Nextcloud response, including a 404 propstat for a property
    /// that folders do not have.
    const SAMPLE: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns">
 <d:response>
  <d:href>/remote.php/dav/files/alice/My%20Notes/</d:href>
  <d:propstat>
   <d:prop>
    <d:getlastmodified>Tue, 05 Nov 2024 10:00:00 GMT</d:getlastmodified>
    <d:resourcetype><d:collection/></d:resourcetype>
    <d:getetag>&quot;6729eaf0a1b2c&quot;</d:getetag>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:response>
  <d:href>/remote.php/dav/files/alice/My%20Notes/0b8a7e1a3c4d4e5f8a9b0c1d2e3f4a5b.md</d:href>
  <d:propstat>
   <d:prop>
    <d:getlastmodified>Tue, 05 Nov 2024 10:00:01 GMT</d:getlastmodified>
    <d:getetag>"abc&amp;def"</d:getetag>
    <d:resourcetype/>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
  <d:propstat>
   <d:prop><d:getcontentlength/></d:prop>
   <d:status>HTTP/1.1 404 Not Found</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;

    #[test]
    fn parses_nextcloud_response() {
        let entries = parse_multistatus(SAMPLE).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].href, "/remote.php/dav/files/alice/My%20Notes/");
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].etag.as_deref(), Some("\"6729eaf0a1b2c\""));
        assert_eq!(entries[0].last_modified, Some(1_730_800_800_000));
        assert!(!entries[1].is_dir);
        assert_eq!(entries[1].etag.as_deref(), Some("\"abc&def\""));
        assert_eq!(entries[1].last_modified, Some(1_730_800_801_000));
    }
}
