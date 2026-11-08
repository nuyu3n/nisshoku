//! Reading and writing the files the user picks in a file dialog. On Android
//! the dialogs hand out `content://` URIs instead of paths, which only the
//! system's content resolver can open (through the fs plugin).

use std::io::{Read, Write};

use tauri::{AppHandle, Runtime};

fn is_content_uri(path: &str) -> bool {
    path.starts_with("content://")
}

fn open<R: Runtime>(app: &AppHandle<R>, uri: &str, write: bool) -> std::io::Result<std::fs::File> {
    use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
    let path: FilePath = uri.parse().map_err(std::io::Error::other)?;
    let mut opts = OpenOptions::new();
    opts.read(!write).write(write).create(write).truncate(write);
    app.fs().open(path, opts)
}

pub fn read<R: Runtime>(app: &AppHandle<R>, path: &str) -> std::io::Result<Vec<u8>> {
    if !is_content_uri(path) {
        return std::fs::read(path);
    }
    let mut data = Vec::new();
    open(app, path, false)?.read_to_end(&mut data)?;
    Ok(data)
}

pub fn write<R: Runtime>(app: &AppHandle<R>, path: &str, data: &[u8]) -> std::io::Result<()> {
    if !is_content_uri(path) {
        return std::fs::write(path, data);
    }
    let mut file = open(app, path, true)?;
    file.write_all(data)?;
    file.flush()
}

/// The extension of common file types, from their first bytes (content
/// URIs carry no file name).
pub fn sniff_extension(data: &[u8]) -> Option<&'static str> {
    Some(match data {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xFF, 0xD8, 0xFF, ..] => "jpg",
        [b'G', b'I', b'F', b'8', ..] => "gif",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "webp",
        [b'B', b'M', ..] => "bmp",
        [b'%', b'P', b'D', b'F', ..] => "pdf",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_images() {
        assert_eq!(sniff_extension(b"\x89PNG\r\n\x1a\n"), Some("png"));
        assert_eq!(sniff_extension(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(sniff_extension(b"%PDF-1.7"), Some("pdf"));
        assert_eq!(sniff_extension(b"hello"), None);
    }
}
