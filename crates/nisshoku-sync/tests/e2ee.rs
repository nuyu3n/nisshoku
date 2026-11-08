//! End-to-end encryption scenarios, run against every backend.

mod common;

use common::*;
use nisshoku_core::{Folder, ItemType, Note, Resource};
use nisshoku_sync::storage::{item_path, resource_blob_path};
use nisshoku_sync::{RemoteStorage, SyncError};

backend_tests!(
    encrypted_round_trip,
    encrypted_target_requires_the_password,
    turning_encryption_on_and_off_reuploads_everything,
    changing_the_master_password,
    reads_a_target_encrypted_by_joplin,
    reads_a_target_encrypted_by_joplin_2,
);

/// Copies a target captured from a real Joplin client onto the storage.
async fn load_sample(s: &dyn RemoteStorage, sample: &str) {
    s.mkdir("").await.unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(sample);
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(&path).unwrap();
        match name.strip_prefix("blob-") {
            Some(id) => s.put(&resource_blob_path(id), &data).await.unwrap(),
            None => s.put(&name, &data).await.unwrap(),
        }
    }
}

/// A target written by Joplin 2.14.1 with encryption enabled (password
/// "legacy-pw"), using the older SJCL methods: SJCL4 master key, SJCL1a items
/// and attachment data.
async fn reads_a_target_encrypted_by_joplin_2(b: &Backend) {
    let s = b.storage().await;
    load_sample(&*s, "joplin2_e2ee").await;
    let c = client_with_password(b, "legacy-pw").await;
    c.sync().await;

    let folders = c.db().all::<Folder>().unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].title, "旧形式ノート");
    let notes = c.notes();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].title, "旧形式の章");
    assert!(
        notes[0]
            .body
            .contains("Joplin 2 が暗号化した本文。\nemoji 🌘 test"),
        "{}",
        notes[0].body
    );
    let resources = c.db().all::<Resource>().unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(
        c.resource_data(&resources[0]).as_deref(),
        Some(&b"legacy\x00attachment\xff bytes"[..])
    );

    // Edits are written with the current (native) method, readable by others.
    c.edit_note(&notes[0].id, "日食が書き直した本文");
    c.sync().await;
    let text = String::from_utf8(s.get(&item_path(&notes[0].id)).await.unwrap().unwrap()).unwrap();
    let cipher = text
        .lines()
        .find_map(|l| l.strip_prefix("encryption_cipher_text: "))
        .unwrap();
    assert_eq!(&cipher[11..13], "0a", "StringV1");
    let other = client_with_password(b, "legacy-pw").await;
    other.sync().await;
    assert_eq!(
        other.note(&notes[0].id).unwrap().body,
        "日食が書き直した本文"
    );
}

async fn client_with_password(b: &Backend, password: &str) -> Client {
    let mut c = Client::of(b).await;
    c.synchronizer.set_master_password(Some(password.into()));
    c
}

/// Every item file on the target is encrypted and contains none of `secrets`.
async fn assert_all_encrypted(s: &dyn RemoteStorage, secrets: &[&str]) {
    let mut items = 0;
    for stat in s.list("").await.unwrap() {
        if nisshoku_sync::storage::item_id_from_path(&stat.path).is_none() {
            continue;
        }
        let text = String::from_utf8(s.get(&stat.path).await.unwrap().unwrap()).unwrap();
        assert!(text.contains("encryption_applied: 1"), "{text}");
        assert!(text.contains("encryption_cipher_text: JED01"), "{text}");
        for secret in secrets {
            assert!(
                !text.contains(secret),
                "plaintext {secret:?} leaked: {text}"
            );
        }
        items += 1;
    }
    assert!(items > 0);
}

async fn assert_none_encrypted(s: &dyn RemoteStorage) {
    for stat in s.list("").await.unwrap() {
        if nisshoku_sync::storage::item_id_from_path(&stat.path).is_some() {
            let text = String::from_utf8(s.get(&stat.path).await.unwrap().unwrap()).unwrap();
            assert!(text.contains("encryption_applied: 0"), "{text}");
        }
    }
}

async fn encrypted_round_trip(b: &Backend) {
    let mut a = Client::of(b).await;
    a.sync().await;
    a.synchronizer
        .enable_encryption("correct horse")
        .await
        .unwrap();

    let folder = a.create_folder("", "秘密のノートブック");
    let note = a.create_note(&folder.id, "極秘の章", "誰にも読まれない本文 🌒");
    let r = a.add_resource("secret.bin", b"binary \x00 secret \xff data");
    a.sync().await;

    let s = b.storage().await;
    assert_all_encrypted(&*s, &["秘密", "極秘", "誰にも", "secret.bin"]).await;
    let blob = s.get(&resource_blob_path(&r.id)).await.unwrap().unwrap();
    assert!(blob.starts_with(b"JED01"));
    assert!(!blob.windows(6).any(|w| w == b"secret"));

    let c = client_with_password(b, "correct horse").await;
    c.sync().await;
    assert_converged(&[&a, &c]);
    assert_eq!(c.note(&note.id).unwrap().body, "誰にも読まれない本文 🌒");
    assert_eq!(
        c.resource_data(&r).as_deref(),
        Some(&b"binary \x00 secret \xff data"[..])
    );

    // Edits, conflicts and deletions work as usual.
    c.edit_note(&note.id, "C の編集");
    a.edit_note(&note.id, "A の編集");
    c.sync().await;
    assert_eq!(a.sync().await.conflicts, 1);
    c.db().delete(ItemType::Resource, &r.id).unwrap();
    c.sync().await;
    sync_until_stable(&[&a, &c]).await;
    assert_converged(&[&a, &c]);
    assert!(s.get(&resource_blob_path(&r.id)).await.unwrap().is_none());
    assert_all_encrypted(&*s, &["C の編集", "A の編集"]).await;
}

async fn encrypted_target_requires_the_password(b: &Backend) {
    let mut a = Client::of(b).await;
    a.sync().await;
    a.synchronizer.enable_encryption("pw1").await.unwrap();
    a.create_note("", "n", "secret");
    a.sync().await;

    let none = Client::of(b).await;
    assert!(matches!(
        none.synchronizer.sync().await,
        Err(SyncError::EncryptionPasswordRequired)
    ));
    let wrong = client_with_password(b, "nope").await;
    assert!(matches!(
        wrong.synchronizer.sync().await,
        Err(SyncError::WrongEncryptionPassword)
    ));
    let right = client_with_password(b, "pw1").await;
    right.sync().await;
    assert_eq!(right.notes().len(), 1);
}

async fn turning_encryption_on_and_off_reuploads_everything(b: &Backend) {
    let mut a = Client::of(b).await;
    let c = client_with_password(b, "pw").await;
    let folder = a.create_folder("", "Plain folder");
    a.create_note(&folder.id, "Plain note", "plain body");
    let r = a.add_resource("plain.bin", b"plain data");
    a.sync().await;
    c.sync().await;
    let s = b.storage().await;
    assert_none_encrypted(&*s).await;

    a.synchronizer.enable_encryption("pw").await.unwrap();
    let report = a.sync().await;
    assert_eq!(report.updated_remote, 3, "{report:?}");
    assert_eq!(report.conflicts, 0);
    assert_all_encrypted(&*s, &["Plain", "plain body"]).await;
    assert!(
        s.get(&resource_blob_path(&r.id))
            .await
            .unwrap()
            .unwrap()
            .starts_with(b"JED01")
    );
    c.sync().await;
    assert_converged(&[&a, &c]);
    assert_eq!(c.resource_data(&r).as_deref(), Some(&b"plain data"[..]));

    a.synchronizer.disable_encryption().await.unwrap();
    a.sync().await;
    assert_none_encrypted(&*s).await;
    assert_eq!(
        s.get(&resource_blob_path(&r.id)).await.unwrap().as_deref(),
        Some(&b"plain data"[..])
    );
    // Without a password one can sync again, as nothing is encrypted.
    let newcomer = Client::of(b).await;
    newcomer.sync().await;
    sync_until_stable(&[&a, &c, &newcomer]).await;
    assert_converged(&[&a, &c, &newcomer]);
}

async fn changing_the_master_password(b: &Backend) {
    let mut a = Client::of(b).await;
    a.sync().await;
    a.synchronizer.enable_encryption("old pw").await.unwrap();
    a.create_note("", "n", "secret");
    a.sync().await;

    assert!(matches!(
        a.synchronizer
            .change_master_password("wrong", "new pw")
            .await,
        Err(SyncError::WrongEncryptionPassword)
    ));
    a.synchronizer
        .change_master_password("old pw", "new pw")
        .await
        .unwrap();
    a.sync().await;
    let old = client_with_password(b, "old pw").await;
    assert!(matches!(
        old.synchronizer.sync().await,
        Err(SyncError::WrongEncryptionPassword)
    ));
    let new = client_with_password(b, "new pw").await;
    new.sync().await;
    assert_eq!(new.notes().len(), 1);
}

/// A target written by Joplin 3.7.1 with encryption enabled (master password
/// "test-master-pw"): a notebook, a note and an attachment.
async fn reads_a_target_encrypted_by_joplin(b: &Backend) {
    let s = b.storage().await;
    load_sample(&*s, "joplin_e2ee").await;

    let c = client_with_password(b, "test-master-pw").await;
    c.sync().await;
    let folder = c
        .db()
        .load::<Folder>("07a1d7475ced4872bc18dc817be4515c")
        .unwrap()
        .unwrap();
    assert_eq!(folder.title, "暗号ノート");
    let note = c
        .db()
        .load::<Note>("0a4d35d634574a93bb6e526b49a3db73")
        .unwrap()
        .unwrap();
    assert_eq!(note.title, "秘密の章");
    assert!(
        note.body.contains("暗号化された本文。\nemoji 🌒 and ascii"),
        "{}",
        note.body
    );
    assert_eq!(note.parent_id, folder.id);
    let r = c
        .db()
        .load::<Resource>("3b9539838d624249a9284f6d0d4ce5a5")
        .unwrap()
        .unwrap();
    assert_eq!(
        c.resource_data(&r).as_deref(),
        Some(&b"secret\x00binary\xffdata"[..])
    );

    // Our edits stay encrypted with Joplin's master key and readable by others.
    c.edit_note(&note.id, "日食からの編集");
    c.sync().await;
    let text = String::from_utf8(s.get(&item_path(&note.id)).await.unwrap().unwrap()).unwrap();
    assert!(
        text.contains("JED010000220ac8ecdeca606d414ab8772f8d0b6cf052"),
        "{text}"
    );
    let other = client_with_password(b, "test-master-pw").await;
    other.sync().await;
    assert_eq!(other.note(&note.id).unwrap().body, "日食からの編集");
}
