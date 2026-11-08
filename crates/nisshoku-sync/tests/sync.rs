//! Sync scenarios, each run against every backend: a local folder, the mock
//! Nextcloud server and (if configured) a real Nextcloud server.

mod common;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use common::*;
use nisshoku_core::{Folder, ItemType, Note, NoteTag, Resource, Tag};
use nisshoku_sync::storage::{item_path, resource_blob_path};
use nisshoku_sync::{SyncError, SyncReport};

backend_tests!(
    items_propagate_between_clients,
    updates_propagate,
    concurrent_edits_create_a_conflict_copy,
    identical_concurrent_edits_are_not_a_conflict,
    simultaneous_syncs_do_not_lose_edits,
    deletions_propagate,
    editing_a_note_deleted_elsewhere_keeps_a_conflict_copy,
    deleting_a_note_edited_elsewhere_keeps_the_edit,
    deleting_a_folder_that_gained_a_note_elsewhere_keeps_it,
    deleting_a_folder_after_another_client_added_a_note_keeps_it,
    nested_folder_deletions_propagate,
    resources_and_their_data_propagate,
    concurrent_resource_changes_keep_both_versions,
    data_without_hash_is_checked_by_size,
    tags_propagate,
    note_history_syncs,
    content_survives_round_trip,
    many_items,
    picks_up_items_written_by_other_clients,
    a_new_client_gets_everything,
    relinking_a_target_creates_no_conflicts,
    lost_response_after_upload_is_harmless,
    interrupted_syncs_resume,
    cancelled_sync_releases_lock_and_resumes,
    exclusive_lock_blocks_sync,
    stale_exclusive_lock_is_ignored,
    refuses_old_targets,
);

async fn items_propagate_between_clients(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let folder = a.create_folder("", "Novel");
    let note = a.create_note(&folder.id, "Chapter 1", "It was a dark and stormy night.");

    let report = a.sync().await;
    assert_eq!(report.created_remote, 2);

    let s = b.storage().await;
    let info = String::from_utf8(s.get("info.json").await.unwrap().unwrap()).unwrap();
    assert!(info.contains("\"version\": 3"), "{info}");
    assert!(s.stat(&item_path(&note.id)).await.unwrap().is_some());
    assert!(s.stat(".resource").await.unwrap().unwrap().is_dir);
    assert!(s.list("locks").await.unwrap().is_empty(), "lock released");

    let report = c.sync().await;
    assert_eq!(report.created_local, 2);
    assert_eq!(c.db().load::<Folder>(&folder.id).unwrap().unwrap(), folder);
    assert_eq!(c.note(&note.id).unwrap(), note);

    assert_eq!(a.sync().await, SyncReport::default());
    assert_eq!(c.sync().await, SyncReport::default());
}

async fn updates_propagate(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let note = a.create_note(&a.create_folder("", "Novel").id, "Chapter 1", "Draft.");
    a.sync().await;
    c.sync().await;

    c.edit_note(&note.id, "Second draft.");
    assert_eq!(c.sync().await.updated_remote, 1);
    assert_eq!(a.sync().await.updated_local, 1);
    assert_eq!(a.note(&note.id).unwrap().body, "Second draft.");

    // Moving to another folder and to the trash are ordinary updates.
    let other = a.create_folder("", "Drafts");
    let mut n = a.note(&note.id).unwrap();
    n.parent_id = other.id.clone();
    n.deleted_time = nisshoku_core::time::now_ms();
    a.db().save(&mut n).unwrap();
    a.sync().await;
    c.sync().await;
    let on_c = c.note(&note.id).unwrap();
    assert_eq!(on_c.parent_id, other.id);
    assert_eq!(on_c.deleted_time, n.deleted_time);
    assert_converged(&[&a, &c]);
}

async fn concurrent_edits_create_a_conflict_copy(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let note = a.create_note(&a.create_folder("", "Novel").id, "Chapter 1", "Draft.");
    a.sync().await;
    c.sync().await;

    a.edit_note(&note.id, "Edited on A.");
    c.edit_note(&note.id, "Edited on C.");
    a.sync().await;
    assert_eq!(c.sync().await.conflicts, 1);

    let notes = c.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].body, "Edited on A.");
    assert_eq!(
        (notes[1].body.as_str(), notes[1].is_conflict),
        ("Edited on C.", 1)
    );
    assert_eq!(notes[1].conflict_original_id, note.id);

    // Conflict copies stay local.
    a.sync().await;
    assert_eq!(a.notes().len(), 1);
    assert_converged(&[&a, &c]);
}

async fn identical_concurrent_edits_are_not_a_conflict(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let note = a.create_note(&a.create_folder("", "Novel").id, "Chapter 1", "Draft.");
    a.sync().await;
    c.sync().await;

    a.edit_note(&note.id, "Same edit.");
    c.edit_note(&note.id, "Same edit.");
    a.sync().await;
    assert_eq!(c.sync().await.conflicts, 0);
    assert_eq!(c.notes().len(), 1);
}

/// Two clients syncing at the same moment, both having edited the same note:
/// one version wins, the other survives as a conflict copy.
async fn simultaneous_syncs_do_not_lose_edits(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let folder = a.create_folder("", "Novel");
    let notes: Vec<Note> = (0..5)
        .map(|i| a.create_note(&folder.id, &format!("Ch {i}"), "Draft."))
        .collect();
    a.sync().await;
    c.sync().await;

    for n in &notes {
        a.edit_note(&n.id, &format!("A edits {}", n.title));
        c.edit_note(&n.id, &format!("C edits {}", n.title));
    }
    let (ra, rc) = tokio::join!(a.synchronizer.sync(), c.synchronizer.sync());
    ra.unwrap();
    rc.unwrap();
    sync_until_stable(&[&a, &c]).await;
    assert_converged(&[&a, &c]);

    // Every edit is either the current version or a conflict copy somewhere.
    let mut bodies: Vec<String> = a
        .notes()
        .into_iter()
        .chain(c.notes())
        .map(|n| n.body)
        .collect();
    bodies.sort();
    bodies.dedup();
    for n in &notes {
        for who in ["A", "C"] {
            let body = format!("{who} edits {}", n.title);
            assert!(bodies.contains(&body), "lost edit {body:?}");
        }
    }
}

async fn deletions_propagate(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let folder = a.create_folder("", "Novel");
    let note = a.create_note(&folder.id, "Chapter 1", "Draft.");
    a.sync().await;
    c.sync().await;

    a.db().delete(ItemType::Folder, &folder.id).unwrap();
    assert_eq!(a.sync().await.deleted_remote, 2);
    assert!(
        b.storage()
            .await
            .stat(&item_path(&note.id))
            .await
            .unwrap()
            .is_none()
    );

    assert_eq!(c.sync().await.deleted_local, 2);
    assert!(c.notes().is_empty());
    assert!(c.db().all::<Folder>().unwrap().is_empty());
    assert!(c.db().synced_item_ids(TARGET_ID).unwrap().is_empty());
    assert!(a.db().synced_item_ids(TARGET_ID).unwrap().is_empty());
    assert!(a.db().deleted_items(TARGET_ID).unwrap().is_empty());
}

async fn editing_a_note_deleted_elsewhere_keeps_a_conflict_copy(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let note = a.create_note(&a.create_folder("", "Novel").id, "Chapter 1", "Draft.");
    a.sync().await;
    c.sync().await;

    a.db().delete(ItemType::Note, &note.id).unwrap();
    a.sync().await;
    c.edit_note(&note.id, "Precious edits.");
    assert_eq!(c.sync().await.conflicts, 1);

    let notes = c.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(
        (notes[0].body.as_str(), notes[0].is_conflict),
        ("Precious edits.", 1)
    );
    assert_converged(&[&a, &c]);
}

async fn deleting_a_note_edited_elsewhere_keeps_the_edit(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let folder = a.create_folder("", "Novel");
    let note = a.create_note(&folder.id, "Chapter 1", "Draft.");
    a.sync().await;
    c.sync().await;

    c.edit_note(&note.id, "Precious edits.");
    c.sync().await;
    // A deletes the folder without having seen the edit.
    a.db().delete(ItemType::Folder, &folder.id).unwrap();
    let report = a.sync().await;
    assert_eq!(report.kept, 2, "{report:?}");

    assert_eq!(a.note(&note.id).unwrap().body, "Precious edits.");
    assert!(a.db().load::<Folder>(&folder.id).unwrap().is_some());
    sync_until_stable(&[&a, &c]).await;
    assert_converged(&[&a, &c]);
    assert_eq!(c.note(&note.id).unwrap().body, "Precious edits.");
}

async fn deleting_a_folder_that_gained_a_note_elsewhere_keeps_it(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let folder = a.create_folder("", "Novel");
    a.sync().await;
    c.sync().await;

    // C adds a note and syncs first; A deletes the folder without knowing.
    let note = c.create_note(&folder.id, "New chapter", "Fresh text.");
    c.sync().await;
    a.db().delete(ItemType::Folder, &folder.id).unwrap();
    a.sync().await;
    sync_until_stable(&[&a, &c]).await;
    assert_converged(&[&a, &c]);
    assert_eq!(a.note(&note.id).unwrap().parent_id, folder.id);
    assert!(a.db().load::<Folder>(&folder.id).unwrap().is_some());
}

async fn deleting_a_folder_after_another_client_added_a_note_keeps_it(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let folder = a.create_folder("", "Novel");
    a.sync().await;
    c.sync().await;

    // A deletes the folder and syncs first; C added a note meanwhile.
    a.db().delete(ItemType::Folder, &folder.id).unwrap();
    a.sync().await;
    let note = c.create_note(&folder.id, "New chapter", "Fresh text.");
    let report = c.sync().await;
    assert_eq!(report.kept, 1, "{report:?}");
    sync_until_stable(&[&a, &c]).await;
    assert_converged(&[&a, &c]);
    assert_eq!(a.note(&note.id).unwrap().body, "Fresh text.");
}

async fn nested_folder_deletions_propagate(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let top = a.create_folder("", "Series");
    let mut parent = top.id.clone();
    for depth in 0..4 {
        let f = a.create_folder(&parent, &format!("Book {depth}"));
        a.create_note(&f.id, &format!("Chapter {depth}"), "Text.");
        parent = f.id;
    }
    a.sync().await;
    c.sync().await;
    assert_eq!(item_type_count(&c, ItemType::Folder), 5);

    a.db().delete(ItemType::Folder, &top.id).unwrap();
    a.sync().await;
    c.sync().await;
    assert_eq!(item_type_count(&c, ItemType::Folder), 0);
    assert_eq!(item_type_count(&c, ItemType::Note), 0);
    assert_converged(&[&a, &c]);
}

async fn resources_and_their_data_propagate(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let r = a.add_resource("map.png", b"version 1");
    a.sync().await;
    let s = b.storage().await;
    assert_eq!(
        s.get(&resource_blob_path(&r.id)).await.unwrap().as_deref(),
        Some(&b"version 1"[..])
    );

    c.sync().await;
    assert_eq!(c.resource_data(&r).as_deref(), Some(&b"version 1"[..]));

    a.set_resource_data(&r.id, b"version 2");
    a.sync().await;
    c.sync().await;
    assert_eq!(c.resource_data(&r).as_deref(), Some(&b"version 2"[..]));

    a.db().delete(ItemType::Resource, &r.id).unwrap();
    a.sync().await;
    assert!(s.get(&resource_blob_path(&r.id)).await.unwrap().is_none());
    c.sync().await;
    assert!(c.db().all::<Resource>().unwrap().is_empty());
    assert!(c.resource_data(&r).is_none());
}

async fn concurrent_resource_changes_keep_both_versions(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let r = a.add_resource("cover.jpg", b"original");
    a.sync().await;
    c.sync().await;

    a.set_resource_data(&r.id, b"from A");
    c.set_resource_data(&r.id, b"from C");
    a.sync().await;
    assert_eq!(c.sync().await.conflicts, 1);

    let resources = c.db().all::<Resource>().unwrap();
    assert_eq!(resources.len(), 2);
    let mut data: Vec<Vec<u8>> = resources
        .iter()
        .map(|r| c.resource_data(r).unwrap())
        .collect();
    data.sort();
    assert_eq!(data, vec![b"from A".to_vec(), b"from C".to_vec()]);
    // A note pointing at the copy appears among the conflicts.
    let conflict = c.notes().into_iter().find(|n| n.is_conflict == 1).unwrap();
    assert!(conflict.body.contains("](:/"));

    sync_until_stable(&[&a, &c]).await;
    assert_converged(&[&a, &c]);
}

/// Resources written by Joplin carry no hash; their size is checked instead.
async fn data_without_hash_is_checked_by_size(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let r = a.add_resource("joplin.bin", b"12345");
    a.sync().await;

    // Rewrite the target as Joplin would have: no hash, and here data of the
    // wrong size (as if mid-replacement).
    let s = b.storage().await;
    let text = String::from_utf8(s.get(&item_path(&r.id)).await.unwrap().unwrap()).unwrap();
    let text: String = text
        .lines()
        .filter(|l| !l.starts_with("nisshoku_blob_sha256:"))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    s.put(&item_path(&r.id), text.as_bytes()).await.unwrap();
    s.put(&resource_blob_path(&r.id), b"123").await.unwrap();

    let report = c.synchronizer.sync().await.unwrap();
    assert_eq!(report.pending_blobs, 1);
    assert!(
        c.resource_data(&r).is_none(),
        "wrong-size data is not accepted"
    );

    // Correct data arrives: accepted.
    s.put(&resource_blob_path(&r.id), b"54321").await.unwrap();
    assert_eq!(c.synchronizer.sync().await.unwrap().pending_blobs, 0);
    assert_eq!(c.resource_data(&r).as_deref(), Some(&b"54321"[..]));

    // A size field that is simply wrong: accepted after a few attempts.
    let r2 = a.add_resource("bad size", b"abc");
    a.sync().await;
    let text = String::from_utf8(s.get(&item_path(&r2.id)).await.unwrap().unwrap()).unwrap();
    let text: String = text
        .lines()
        .filter(|l| !l.starts_with("nisshoku_blob_sha256:"))
        .map(|l| {
            if l.starts_with("size: ") {
                "size: 999"
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    s.put(&item_path(&r2.id), text.as_bytes()).await.unwrap();
    let c2 = Client::of(b).await;
    let mut accepted = false;
    for _ in 0..4 {
        c2.synchronizer.sync().await.unwrap();
        if c2.resource_data(&r2).is_some() {
            accepted = true;
            break;
        }
    }
    assert!(accepted);
    assert_eq!(c2.resource_data(&r2).as_deref(), Some(&b"abc"[..]));
}

async fn note_history_syncs(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let folder = a.create_folder("", "Novel");
    let mut note = a.create_note(&folder.id, "第一稿", "書き出し。");
    let r1 = a.db().create_revision(&note).unwrap().unwrap();
    note.title = "第二稿".into();
    note.body = "書き出しを直した。🌒
続き。"
        .into();
    a.db().save(&mut note).unwrap();
    let r2 = a.db().create_revision(&note).unwrap().unwrap();
    a.sync().await;
    c.sync().await;
    assert_converged(&[&a, &c]);

    // The other client sees the whole history and can restore from it.
    assert_eq!(c.db().revisions(&note.id).unwrap().len(), 2);
    let v1 = c.db().note_at_revision(&note.id, &r1.id).unwrap();
    assert_eq!(
        (v1.title.as_str(), v1.body.as_str()),
        ("第一稿", "書き出し。")
    );
    let v2 = c.db().note_at_revision(&note.id, &r2.id).unwrap();
    assert_eq!(
        v2.body,
        "書き出しを直した。🌒
続き。"
    );

    // History outlives the note: delete it on A, restore it on C.
    a.db().delete(ItemType::Note, &note.id).unwrap();
    a.sync().await;
    c.sync().await;
    assert!(c.note(&note.id).is_none());
    c.db().restore_revision(&note.id, &r1.id).unwrap();
    c.sync().await;
    a.sync().await;
    assert_eq!(a.note(&note.id).unwrap().body, "書き出し。");
    assert_converged(&[&a, &c]);

    // Expiry propagates and keeps the remaining history usable.
    // Age the oldest revision past the retention period.
    let mut oldest = a.db().revisions(&note.id).unwrap().remove(0);
    oldest.created_time -= nisshoku_core::revisions::DEFAULT_TTL_MS + 1000;
    a.db().write(&oldest).unwrap();
    assert_eq!(
        a.db()
            .delete_old_revisions(nisshoku_core::revisions::DEFAULT_TTL_MS)
            .unwrap(),
        1
    );
    a.sync().await;
    c.sync().await;
    assert_converged(&[&a, &c]);
    let left = c.db().revisions(&note.id).unwrap();
    assert_eq!(left.len(), 1);
    let latest = left.last().unwrap();
    assert!(c.db().note_at_revision(&note.id, &latest.id).is_ok());
}

async fn tags_propagate(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let note = a.create_note(&a.create_folder("", "Novel").id, "Chapter 1", "Draft.");
    let mut tag = Tag {
        title: "first draft".into(),
        ..Default::default()
    };
    a.db().save(&mut tag).unwrap();
    let mut link = NoteTag {
        note_id: note.id.clone(),
        tag_id: tag.id.clone(),
        ..Default::default()
    };
    a.db().save(&mut link).unwrap();
    a.sync().await;
    c.sync().await;

    assert_eq!(
        c.db().load::<Tag>(&tag.id).unwrap().unwrap().title,
        "first draft"
    );
    let links = c
        .db()
        .query::<NoteTag, _>("note_id = ?1", [&note.id])
        .unwrap();
    assert_eq!(links.len(), 1);

    a.db().delete(ItemType::Tag, &tag.id).unwrap();
    a.sync().await;
    c.sync().await;
    assert_eq!(item_type_count(&c, ItemType::Tag), 0);
    assert_eq!(item_type_count(&c, ItemType::NoteTag), 0);
}

async fn content_survives_round_trip(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let folder = a.create_folder("", "長編小説「日食」");
    let bodies = [
        "第一章\r\n\r\n　吾輩は猫である。名前はまだ無い。\r\n",
        "key: value\nother: thing\n\nid: fake\ntype_: 2",
        "",
        "\n\nleading and trailing blank lines\n\n",
        "emoji 🌒🌑🌘 and tabs\tand \\n literal backslash-n",
        &"long line ".repeat(50_000),
    ];
    let titles = [
        "タイトル",
        "multi\nline title",
        "",
        "  spaces  ",
        "colon: in title",
        "x",
    ];
    let ids: Vec<String> = titles
        .iter()
        .zip(bodies)
        .map(|(t, body)| a.create_note(&folder.id, t, body).id)
        .collect();
    a.sync().await;
    c.sync().await;
    for id in &ids {
        assert_eq!(c.note(id).unwrap(), a.note(id).unwrap());
    }
    assert_eq!(
        c.db().load::<Folder>(&folder.id).unwrap().unwrap().title,
        "長編小説「日食」"
    );
}

async fn many_items(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let n = if b.kind.is_real_service() { 120 } else { 400 };
    let folder = a.create_folder("", "Big");
    for i in 0..n {
        a.create_note(&folder.id, &format!("Note {i}"), &format!("Body {i}"));
    }
    assert_eq!(a.sync().await.created_remote, n + 1);
    assert_eq!(c.sync().await.created_local, n + 1);
    assert_converged(&[&a, &c]);
    assert!(a.sync().await.is_noop());
}

async fn picks_up_items_written_by_other_clients(b: &Backend) {
    let a = Client::of(b).await;
    a.sync().await; // initializes the target
    let s = b.storage().await;

    let id = "0b8a7e1a3c4d4e5f8a9b0c1d2e3f4a5b";
    let text = format!(
        "Written by Joplin\n\nBody line\n\nid: {id}\nparent_id: \ncreated_time: 2024-03-01T10:00:00.000Z\n\
         updated_time: 2024-03-02T11:30:15.250Z\nis_conflict: 0\nmarkup_language: 1\n\
         source_application: net.cozic.joplin-desktop\nuser_data: \ndeleted_time: 0\ntype_: 1"
    );
    s.put(&item_path(id), text.as_bytes()).await.unwrap();
    // Items of types this version does not handle, and other files, are left alone.
    let rev = "1b8a7e1a3c4d4e5f8a9b0c1d2e3f4a5b";
    s.put(&item_path(rev), format!("id: {rev}\ntype_: 9").as_bytes())
        .await
        .unwrap();
    s.put("notes.txt", b"not an item").await.unwrap();

    let report = a.sync().await;
    assert_eq!(report.created_local, 1);
    let note = a.note(id).unwrap();
    assert_eq!(
        (note.title.as_str(), note.body.as_str()),
        ("Written by Joplin", "Body line")
    );
    assert!(s.stat(&item_path(rev)).await.unwrap().is_some());

    // Corrupt files are reported, not fatal, and do not block other items.
    let bad = "2b8a7e1a3c4d4e5f8a9b0c1d2e3f4a5b";
    s.put(&item_path(bad), b"garbage without properties")
        .await
        .unwrap();
    let ok = a.create_note("", "Still syncs", "");
    let report = a.synchronizer.sync().await.unwrap();
    assert_eq!(report.errors.len(), 1, "{report:?}");
    assert!(s.stat(&item_path(&ok.id)).await.unwrap().is_some());
}

async fn a_new_client_gets_everything(b: &Backend) {
    let a = Client::of(b).await;
    let folder = a.create_folder("", "Novel");
    for i in 0..5 {
        a.create_note(&folder.id, &format!("Ch {i}"), "Text");
    }
    a.add_resource("img", b"\x89PNG data");
    a.sync().await;

    let newcomer = Client::of(b).await;
    newcomer.sync().await;
    assert_converged(&[&a, &newcomer]);
}

/// Connecting a profile to a target that already has its items (e.g. after
/// resetting sync state) must not produce conflict copies.
async fn relinking_a_target_creates_no_conflicts(b: &Backend) {
    let a = Client::of(b).await;
    let folder = a.create_folder("", "Novel");
    a.create_note(&folder.id, "Ch 1", "Text");
    a.add_resource("img", b"data");
    a.sync().await;

    let relinked = Client::with_db(
        a.db.clone(),
        b.storage().await,
        TARGET_ID + 1,
        tempfile::tempdir().unwrap(),
    );
    // Same database, same files, new target id: everything looks unsynced.
    for r in a.db().all::<Resource>().unwrap() {
        std::fs::copy(a.resource_path(&r), relinked.resource_path(&r)).unwrap();
    }
    let report = relinked.sync().await;
    assert_eq!(report.conflicts, 0, "{report:?}");
    assert_eq!(a.notes().len(), 1);
    assert!(relinked.sync().await.is_noop());
}

/// The upload succeeds but the client never learns it (lost response).
async fn lost_response_after_upload_is_harmless(b: &Backend) {
    let (a, c) = (Client::of(b).await, Client::of(b).await);
    let note = a.create_note(&a.create_folder("", "Novel").id, "Chapter 1", "Draft.");
    a.sync().await;
    c.sync().await;
    a.edit_note(&note.id, "Edit whose response was lost.");

    // Make A forget that the upload happened.
    a.sync().await;
    a.db()
        .set_sync_time(TARGET_ID, ItemType::Note, &note.id, 1)
        .unwrap();
    let report = a.sync().await;
    assert_eq!(report.conflicts, 0, "{report:?}");
    assert_eq!(a.notes().len(), 1);
    c.sync().await;
    assert_converged(&[&a, &c]);
}

async fn interrupted_syncs_resume(b: &Backend) {
    let storage = b.storage().await;
    let flaky = Flaky::new(storage, 7, 150);
    let a = Client::new(flaky.clone());
    let c = Client::of(b).await;
    let folder = a.create_folder("", "Novel");
    for i in 0..20 {
        a.create_note(&folder.id, &format!("Ch {i}"), &format!("Text {i}"));
    }
    a.add_resource("img", b"data");

    let mut failures = 0;
    for _ in 0..100 {
        match a.synchronizer.sync().await {
            Ok(report) if report.errors.is_empty() => break,
            Ok(report) => panic!("item errors: {:?}", report.errors),
            Err(SyncError::Remote(_)) => failures += 1,
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
    assert!(failures > 0, "the test should have injected failures");
    flaky.set_failure_rate(0);
    sync_until_stable(&[&a, &c]).await;
    assert_converged(&[&a, &c]);
    assert_eq!(c.notes().len(), 20);
    assert!(
        a.notes().iter().all(|n| n.is_conflict == 0),
        "no spurious conflict copies"
    );
}

async fn cancelled_sync_releases_lock_and_resumes(b: &Backend) {
    let flag: Arc<Mutex<Option<Arc<AtomicBool>>>> = Arc::new(Mutex::new(None));
    let hook_flag = flag.clone();
    let storage = Hooked::new(b.storage().await, move |n, _, _| {
        if n == 12
            && let Some(f) = hook_flag.lock().unwrap().as_ref()
        {
            f.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    });
    let a = Client::new(storage);
    *flag.lock().unwrap() = Some(a.synchronizer.cancel_flag());
    let folder = a.create_folder("", "Novel");
    for i in 0..10 {
        a.create_note(&folder.id, &format!("Ch {i}"), "Text");
    }
    assert!(matches!(
        a.synchronizer.sync().await,
        Err(SyncError::Cancelled)
    ));
    assert!(b.storage().await.list("locks").await.unwrap().is_empty());

    a.sync().await;
    let c = Client::of(b).await;
    c.sync().await;
    assert_converged(&[&a, &c]);
}

async fn exclusive_lock_blocks_sync(b: &Backend) {
    let a = Client::of(b).await;
    a.sync().await;
    let lock = format!(
        r#"{{"type":2,"clientType":"desktop","clientId":"other","updatedTime":{}}}"#,
        nisshoku_core::time::now_ms()
    );
    b.storage()
        .await
        .put("locks/2_desktop_other.json", lock.as_bytes())
        .await
        .unwrap();
    assert!(matches!(
        a.synchronizer.sync().await,
        Err(SyncError::Locked(_))
    ));
}

async fn stale_exclusive_lock_is_ignored(b: &Backend) {
    let a = Client::of(b).await;
    a.sync().await;
    let lock = format!(
        r#"{{"type":2,"clientType":"mobile","clientId":"crashed","updatedTime":{}}}"#,
        nisshoku_core::time::now_ms() - 10 * 60 * 1000
    );
    b.storage()
        .await
        .put("locks/2_mobile_crashed.json", lock.as_bytes())
        .await
        .unwrap();
    a.sync().await;
}

async fn refuses_old_targets(b: &Backend) {
    let a = Client::of(b).await;
    let s = b.storage().await;
    s.put(".sync/version.txt", b"2").await.unwrap();
    assert!(matches!(
        a.synchronizer.sync().await,
        Err(SyncError::IncompatibleTarget(_))
    ));

    s.delete(".sync/version.txt").await.unwrap();
    s.put(
        &item_path("0b8a7e1a3c4d4e5f8a9b0c1d2e3f4a5b"),
        b"id: 0b8a7e1a3c4d4e5f8a9b0c1d2e3f4a5b\ntype_: 1",
    )
    .await
    .unwrap();
    // Items but no version information: do not touch it.
    assert!(matches!(
        a.synchronizer.sync().await,
        Err(SyncError::IncompatibleTarget(_))
    ));
}

#[test]
fn sync_future_is_send() {
    fn assert_send<T: Send>(_: &T) {}
    let dir = tempfile::tempdir().unwrap();
    let a = Client::new(Arc::new(nisshoku_sync::drivers::FileSystemStorage::new(
        dir.path(),
    )));
    let fut = a.synchronizer.sync();
    assert_send(&fut);
}
