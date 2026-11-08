//! Development CLI: a profile directory synced with Nextcloud, OneDrive,
//! Google Drive or a local folder.
//!
//! ```text
//! cargo run --example nisshoku -- <profile-dir> <command> [args]
//!
//! Target (stored in the profile on first use):
//!   NISSHOKU_NEXTCLOUD_URL / _USER / _PASSWORD / _FOLDER
//!   NISSHOKU_ONEDRIVE_CLIENT_ID / _REFRESH_TOKEN / _FOLDER
//!   NISSHOKU_GDRIVE_CLIENT_ID / _CLIENT_SECRET / _REFRESH_TOKEN / _FOLDER
//!   NISSHOKU_TARGET_DIR
//!   (refresh tokens: `cargo run --example login`)
//!
//! Commands:
//!   sync                              sync with the target
//!   ls                                list folders and notes
//!   cat <id-prefix>                   print a note
//!   mkbook <title>                    create a folder
//!   mknote <folder-prefix> <title> <body>
//!   edit <id-prefix> <body>           replace a note's body (`@file` reads it from a file)
//!   rm <id-prefix>                    delete a note or folder
//!   attach <note-prefix> <file>       add a file to a note
//!   rev <note-prefix>                 record a revision of a note now
//!   history <note-prefix>             list a note's revisions and their versions
//! ```

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use nisshoku_core::{AnyItem, Database, Folder, ItemType, Note, NoteTag, Resource, Tag};
use nisshoku_sync::drivers::{
    FileSystemStorage, GoogleDriveConfig, GoogleDriveStorage, NextcloudConfig, NextcloudStorage,
    OAuthTokens, OneDriveConfig, OneDriveStorage,
};
use nisshoku_sync::{RemoteStorage, SyncOptions, Synchronizer};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn find(db: &Database, prefix: &str) -> Res<AnyItem> {
    let mut found = Vec::new();
    for f in db.all::<Folder>()? {
        if f.id.starts_with(prefix) {
            found.push(AnyItem::Folder(f));
        }
    }
    for n in db.all::<Note>()? {
        if n.id.starts_with(prefix) {
            found.push(AnyItem::Note(n));
        }
    }
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(format!("no item with id {prefix}").into()),
        _ => Err(format!("{prefix} is ambiguous").into()),
    }
}

async fn storage() -> Res<Arc<dyn RemoteStorage>> {
    let var = |name: &str| std::env::var(name);
    let folder = |name: &str| var(name).unwrap_or_else(|_| "Nisshoku".into());
    let tokens = |refresh_token: String| OAuthTokens {
        refresh_token,
        ..Default::default()
    };
    if let Ok(dir) = var("NISSHOKU_TARGET_DIR") {
        return Ok(Arc::new(FileSystemStorage::new(dir)));
    }
    if let Ok(client_id) = var("NISSHOKU_ONEDRIVE_CLIENT_ID") {
        let config = OneDriveConfig {
            client_id,
            folder: folder("NISSHOKU_ONEDRIVE_FOLDER"),
            tokens: tokens(var("NISSHOKU_ONEDRIVE_REFRESH_TOKEN")?),
            endpoints: Default::default(),
        };
        return Ok(Arc::new(OneDriveStorage::connect(&config, None).await?));
    }
    if let Ok(client_id) = var("NISSHOKU_GDRIVE_CLIENT_ID") {
        let config = GoogleDriveConfig {
            client_id,
            client_secret: var("NISSHOKU_GDRIVE_CLIENT_SECRET")?,
            folder: folder("NISSHOKU_GDRIVE_FOLDER"),
            tokens: tokens(var("NISSHOKU_GDRIVE_REFRESH_TOKEN")?),
            endpoints: Default::default(),
        };
        return Ok(Arc::new(GoogleDriveStorage::connect(&config, None).await?));
    }
    let config = NextcloudConfig {
        server_url: std::env::var("NISSHOKU_NEXTCLOUD_URL")?,
        username: std::env::var("NISSHOKU_NEXTCLOUD_USER")?,
        password: std::env::var("NISSHOKU_NEXTCLOUD_PASSWORD")?,
        folder: std::env::var("NISSHOKU_NEXTCLOUD_FOLDER").unwrap_or_else(|_| "Nisshoku".into()),
    };
    Ok(Arc::new(NextcloudStorage::connect(&config).await?))
}

#[tokio::main]
async fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (profile, command) = match args.as_slice() {
        [p, c, ..] => (PathBuf::from(p), c.as_str()),
        _ => return Err("usage: nisshoku <profile-dir> <command> [args]".into()),
    };
    let rest = &args[2..];
    std::fs::create_dir_all(profile.join("resources"))?;
    let db = Arc::new(Mutex::new(Database::open(profile.join("database.sqlite"))?));

    match command {
        "sync" => {
            let sync = Synchronizer::new(
                db.clone(),
                storage().await?,
                SyncOptions {
                    target_id: 1,
                    resource_dir: profile.join("resources"),
                    client_type: "desktop".into(),
                    master_password: std::env::var("NISSHOKU_MASTER_PASSWORD").ok(),
                    ..Default::default()
                },
            );
            println!("{:#?}", sync.sync().await?);
        }
        "ls" => {
            let db = db.lock().unwrap();
            let notes = db.all::<Note>()?;
            let tags = db.all::<Tag>()?;
            let links = db.all::<NoteTag>()?;
            for f in db.all::<Folder>()? {
                println!("[{}] {}", &f.id[..8], f.title);
                for n in notes.iter().filter(|n| n.parent_id == f.id) {
                    let note_tags: Vec<&str> = links
                        .iter()
                        .filter(|l| l.note_id == n.id)
                        .filter_map(|l| tags.iter().find(|t| t.id == l.tag_id))
                        .map(|t| t.title.as_str())
                        .collect();
                    let conflict = if n.is_conflict == 1 {
                        " (conflict)"
                    } else {
                        ""
                    };
                    println!("    {} {}{conflict} {note_tags:?}", &n.id[..8], n.title);
                }
            }
            for n in notes.iter().filter(|n| n.is_conflict == 1) {
                println!(
                    "conflict: {} {} <- {}",
                    &n.id[..8],
                    n.title,
                    n.conflict_original_id
                );
            }
            for r in db.all::<Resource>()? {
                let size = std::fs::metadata(profile.join("resources").join(r.file_name()))
                    .map(|m| m.len());
                println!("resource {} {} {:?}", &r.id[..8], r.title, size.ok());
            }
        }
        "cat" => {
            let db = db.lock().unwrap();
            let AnyItem::Note(n) = find(&db, &rest[0])? else {
                return Err("not a note".into());
            };
            println!("{}\n\n{}", n.title, n.body);
        }
        "mkbook" => {
            let mut f = Folder::new("", &rest[0]);
            db.lock().unwrap().save(&mut f)?;
            println!("{}", f.id);
        }
        "mknote" => {
            let db = db.lock().unwrap();
            let AnyItem::Folder(f) = find(&db, &rest[0])? else {
                return Err("not a folder".into());
            };
            let mut n = Note::new(&f.id, &rest[1], &rest[2]);
            db.save(&mut n)?;
            println!("{}", n.id);
        }
        "edit" => {
            let db = db.lock().unwrap();
            let AnyItem::Note(mut n) = find(&db, &rest[0])? else {
                return Err("not a note".into());
            };
            n.body = match rest[1].strip_prefix('@') {
                Some(file) => std::fs::read_to_string(file)?,
                None => rest[1].clone(),
            };
            db.save(&mut n)?;
        }
        "rm" => {
            let db = db.lock().unwrap();
            let item = find(&db, &rest[0])?;
            db.delete(item.item_type(), item.id())?;
        }
        "attach" => {
            let db = db.lock().unwrap();
            let AnyItem::Note(mut n) = find(&db, &rest[0])? else {
                return Err("not a note".into());
            };
            let src = PathBuf::from(&rest[1]);
            let name = src.file_name().unwrap().to_string_lossy().into_owned();
            let ext = src
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut r = Resource {
                id: nisshoku_core::ids::new_id(),
                title: name.clone(),
                filename: name.clone(),
                file_extension: ext,
                mime: "application/octet-stream".into(),
                size: std::fs::metadata(&src)?.len() as i64,
                blob_updated_time: nisshoku_core::time::now_ms(),
                ..Default::default()
            };
            std::fs::copy(&src, profile.join("resources").join(r.file_name()))?;
            db.save(&mut r)?;
            n.body.push_str(&format!("\n\n[{name}](:/{})", r.id));
            db.save(&mut n)?;
            let _ = ItemType::Resource;
            println!("{}", r.id);
        }
        "rev" => {
            let db = db.lock().unwrap();
            let AnyItem::Note(n) = find(&db, &rest[0])? else {
                return Err("not a note".into());
            };
            match db.create_revision(&n)? {
                Some(r) => println!("{}", r.id),
                None => println!("unchanged"),
            }
        }
        "history" => {
            let db = db.lock().unwrap();
            let AnyItem::Note(n) = find(&db, &rest[0])? else {
                return Err("not a note".into());
            };
            for r in db.revisions(&n.id)? {
                let v = db.note_at_revision(&n.id, &r.id)?;
                println!("{} {:?} {:?}", &r.id[..8], v.title, v.body);
            }
        }
        other => return Err(format!("unknown command {other}").into()),
    }
    Ok(())
}
