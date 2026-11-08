//! Revisions written by Joplin desktop 3.7.22, captured from a sync target,
//! with the values Joplin keeps in its database for them.

use nisshoku_core::{AnyItem, Database, Folder, Model, Note, RawItem, Revision};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/joplin_revisions");
const FIRST: &str = "929cb5f3c6164d1ea4f69359c0a39286";
const SECOND: &str = "f0142b4f20ea4608b26a3e18fb6b542e";
const NOTE: &str = "6b0c9d90d67a40518eacaed24cdb66b1";

fn file(id: &str) -> String {
    std::fs::read_to_string(format!("{DIR}/{id}.md")).unwrap()
}

fn revision(id: &str) -> Revision {
    Revision::from_raw(&RawItem::parse(&file(id)).unwrap()).unwrap()
}

#[test]
fn parsing_gives_joplins_database_values() {
    let expected: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(format!("{DIR}/database_values.json")).unwrap(),
    )
    .unwrap();
    for row in expected {
        let r = revision(row["id"].as_str().unwrap());
        assert_eq!(r.title_diff, row["title_diff"].as_str().unwrap());
        assert_eq!(r.body_diff, row["body_diff"].as_str().unwrap());
        assert_eq!(r.metadata_diff, row["metadata_diff"].as_str().unwrap());
        assert_eq!(r.item_id, NOTE);
    }
}

#[test]
fn serializing_reproduces_joplins_files() {
    for id in [FIRST, SECOND] {
        let original = file(id);
        let ours = revision(id).to_text();
        for key in [
            "title_diff",
            "body_diff",
            "metadata_diff",
            "item_updated_time",
            "item_id",
        ] {
            let line = |text: &str| {
                text.lines()
                    .find(|l| l.starts_with(&format!("{key}:")))
                    .map(str::to_string)
            };
            assert_eq!(line(&ours), line(&original), "{key} of {id}");
        }
    }
}

#[test]
fn rebuilds_the_note_versions() {
    let db = Database::open_in_memory().unwrap();
    let mut folder = Folder::new("", "f");
    folder.id = "98274495b3f64eb496b98f80a308ee7f".into();
    db.write(&folder).unwrap();
    for id in [FIRST, SECOND] {
        let Some(AnyItem::Revision(r)) = AnyItem::parse(&file(id)).unwrap() else {
            panic!()
        };
        db.write(&r).unwrap();
    }

    let v1 = db.note_at_revision(NOTE, FIRST).unwrap();
    assert_eq!(v1.title, "版3のタイトル");
    assert_eq!(v1.body, "全面的に書き直した本文。\n\n新しい段落。");
    assert_eq!((v1.is_todo, v1.author.as_str()), (1, "作者"));
    assert_eq!(v1.parent_id, folder.id);

    let v2: Note = db.note_at_revision(NOTE, SECOND).unwrap();
    assert_eq!(v2.title, "版4のタイトル 🌒 \\n");
    let expected_body = std::fs::read_to_string(format!("{DIR}/expected_body.txt")).unwrap();
    assert_eq!(v2.body, expected_body);
    assert_eq!((v2.is_todo, v2.author.as_str()), (0, ""));

    // Our diff from version 1 to version 2 applies to give the same result.
    let ours = nisshoku_core::dmp::diff_json(&v1.body, &v2.body);
    assert_eq!(
        nisshoku_core::dmp::apply_json(&ours, &v1.body).unwrap(),
        expected_body
    );
}
