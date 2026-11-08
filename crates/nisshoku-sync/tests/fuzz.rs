//! Randomized multi-client test. Three clients make random changes and sync
//! at random moments (sometimes simultaneously) through a storage that fails
//! at random, before or after operations. Once failures stop, all clients
//! must converge to the same state, with no notes left without a folder.
//!
//! Reproduce a failure with `NISSHOKU_FUZZ_SEED=<seed> cargo test --test fuzz`;
//! run more seeds with `NISSHOKU_FUZZ_COUNT=<n>`.

mod common;

use std::sync::Arc;

use common::*;
use nisshoku_core::{Folder, ItemType, Note, NoteTag, Resource, Tag};
use nisshoku_sync::SyncError;

struct Actor {
    client: Client,
    flaky: Arc<Flaky>,
}

fn pick<T: Clone>(rng: &mut Rng, items: &[T]) -> Option<T> {
    (!items.is_empty()).then(|| items[rng.below(items.len())].clone())
}

fn trace(msg: impl FnOnce() -> String) {
    if std::env::var_os("NISSHOKU_FUZZ_TRACE").is_some() {
        eprintln!("{}", msg());
    }
}

/// One random local change.
fn mutate(rng: &mut Rng, c: &Client, tag: &str, step: usize) {
    let before = c.snapshot();
    mutate_inner(rng, c, tag, step);
    if std::env::var_os("NISSHOKU_FUZZ_TRACE").is_some() {
        let after = c.snapshot();
        for (k, v) in &after {
            if before.get(k) != Some(v) {
                let first = v.lines().next().unwrap_or("");
                let parent = v
                    .lines()
                    .find(|l| l.starts_with("parent_id: "))
                    .unwrap_or("");
                eprintln!(
                    "  {tag} step {step}: set {} {first:?} {parent}",
                    &k[..k.len().min(16)]
                );
            }
        }
        for k in before.keys().filter(|k| !after.contains_key(*k)) {
            eprintln!("  {tag} step {step}: removed {}", &k[..k.len().min(16)]);
        }
    }
}

fn mutate_inner(rng: &mut Rng, c: &Client, tag: &str, step: usize) {
    let folders = c.db().all::<Folder>().unwrap();
    let notes = c.db().query::<Note, _>("is_conflict = 0", []).unwrap();
    let text = format!("{tag} step {step}");
    match rng.below(100) {
        0..=11 => {
            let parent = if rng.chance(400) {
                pick(rng, &folders).map(|f| f.id).unwrap_or_default()
            } else {
                String::new()
            };
            c.create_folder(&parent, &format!("Folder {text}"));
        }
        12..=36 => match pick(rng, &folders) {
            Some(f) => {
                c.create_note(&f.id, &format!("Note {text}"), &format!("Body {text}"));
            }
            None => {
                c.create_folder("", &format!("Folder {text}"));
            }
        },
        37..=61 => {
            if let Some(mut n) = pick(rng, &notes) {
                n.body = format!("Edit {text}");
                // Through the history-keeping save, sometimes recording a revision.
                c.db().save_note(&mut n).unwrap();
                if rng.chance(300) {
                    c.db().create_revision(&n).unwrap();
                }
            }
        }
        62..=66 => {
            if let Some(mut f) = pick(rng, &folders) {
                f.title = format!("Renamed {text}");
                c.db().save(&mut f).unwrap();
            }
        }
        67..=73 => {
            if let (Some(mut n), Some(f)) = (pick(rng, &notes), pick(rng, &folders)) {
                n.parent_id = f.id;
                c.db().save(&mut n).unwrap();
            }
        }
        74..=79 => {
            if let Some(n) = pick(rng, &notes) {
                c.db().delete(ItemType::Note, &n.id).unwrap();
            }
        }
        80..=82 => {
            if let Some(f) = pick(rng, &folders) {
                c.db().delete(ItemType::Folder, &f.id).unwrap();
            }
        }
        83..=88 => {
            if let Some(n) = pick(rng, &notes) {
                let tags = c.db().all::<Tag>().unwrap();
                let tag = match pick(rng, &tags) {
                    Some(t) if rng.chance(700) => t,
                    _ => {
                        let mut t = Tag {
                            title: format!("tag {text}"),
                            ..Default::default()
                        };
                        c.db().save(&mut t).unwrap();
                        t
                    }
                };
                let mut link = NoteTag {
                    note_id: n.id,
                    tag_id: tag.id,
                    ..Default::default()
                };
                c.db().save(&mut link).unwrap();
            }
        }
        89..=90 => {
            let tags = c.db().all::<Tag>().unwrap();
            if let Some(t) = pick(rng, &tags) {
                c.db().delete(ItemType::Tag, &t.id).unwrap();
            }
        }
        91..=94 => {
            c.add_resource(&format!("res {text}"), format!("data {text}").as_bytes());
        }
        95..=97 => {
            let resources = c.db().all::<Resource>().unwrap();
            if let Some(r) = pick(rng, &resources)
                && c.resource_path(&r).exists()
            {
                c.set_resource_data(&r.id, format!("new data {text}").as_bytes());
            }
        }
        _ => {
            let resources = c.db().all::<Resource>().unwrap();
            if let Some(r) = pick(rng, &resources) {
                let _ = std::fs::remove_file(c.resource_path(&r));
                c.db().delete(ItemType::Resource, &r.id).unwrap();
            }
        }
    }
}

async fn try_sync(actor: &Actor, name: &str) {
    let result = actor.client.synchronizer.sync().await;
    trace(|| format!("{name} sync: {result:?}"));
    match result {
        Ok(report) => assert!(report.errors.is_empty(), "item errors: {:?}", report.errors),
        Err(SyncError::Remote(msg)) if msg.contains("injected") => {}
        Err(e) => panic!("unexpected sync error: {e}"),
    }
}

async fn run(
    kind: Kind,
    seed: u64,
    steps: usize,
    failure_per_mille: u32,
    concurrent: bool,
    encrypted: bool,
) {
    let Some(backend) = Backend::new(kind).await else {
        return;
    };
    let mut rng = Rng::new(seed);
    let mut actors = Vec::new();
    for i in 0..3 {
        let flaky = Flaky::new(backend.storage().await, seed * 31 + i, failure_per_mille);
        let mut client = Client::new(flaky.clone());
        if encrypted {
            client
                .synchronizer
                .set_master_password(Some("fuzz password".into()));
        }
        actors.push(Actor { client, flaky });
    }
    if encrypted {
        actors[0].flaky.set_failure_rate(0);
        actors[0]
            .client
            .synchronizer
            .enable_encryption("fuzz password")
            .await
            .unwrap();
        actors[0].flaky.set_failure_rate(failure_per_mille);
    }

    for step in 0..steps {
        let i = rng.below(actors.len());
        if encrypted && rng.chance(15) {
            // Switch encryption off or on while others keep working.
            let sync = &mut actors[i].client.synchronizer;
            let result = if rng.chance(500) {
                sync.disable_encryption().await
            } else {
                sync.enable_encryption("fuzz password").await
            };
            trace(|| format!("c{i} toggles encryption: {result:?}"));
            match result {
                Ok(()) | Err(SyncError::Remote(_)) => {}
                Err(e) => panic!("unexpected error toggling encryption: {e}"),
            }
        } else if rng.chance(250) {
            if concurrent && rng.chance(300) {
                let j = (i + 1 + rng.below(actors.len() - 1)) % actors.len();
                let (ni, nj) = (format!("c{i}"), format!("c{j}"));
                tokio::join!(try_sync(&actors[i], &ni), try_sync(&actors[j], &nj));
            } else {
                try_sync(&actors[i], &format!("c{i}")).await;
            }
        } else {
            mutate(&mut rng, &actors[i].client, &format!("c{i}"), step);
        }
    }

    for a in &actors {
        a.flaky.set_failure_rate(0);
    }
    trace(|| "--- failures off ---".into());
    for _ in 0..3 {
        for (i, a) in actors.iter().enumerate() {
            try_sync(a, &format!("c{i}")).await;
        }
    }
    let clients: Vec<&Client> = actors.iter().map(|a| &a.client).collect();
    sync_until_stable(&clients).await;
    assert_converged(&clients);
    backend.cleanup().await;
}

/// Prints the seed if the test panics, so that it can be reproduced.
struct SeedReporter(u64);

impl Drop for SeedReporter {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("FAILING SEED: {}", self.0);
        }
    }
}

fn seeds(default: std::ops::Range<u64>) -> Vec<u64> {
    if let Ok(s) = std::env::var("NISSHOKU_FUZZ_SEED") {
        return vec![s.parse().expect("NISSHOKU_FUZZ_SEED must be a number")];
    }
    let count = std::env::var("NISSHOKU_FUZZ_COUNT")
        .ok()
        .and_then(|c| c.parse::<u64>().ok())
        .unwrap_or(default.end - default.start);
    (default.start * 1000..default.start * 1000 + count).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_without_failures() {
    for seed in seeds(1..21) {
        eprintln!("seed {seed}");
        let _guard = SeedReporter(seed);
        run(Kind::FileSystem, seed, 150, 0, false, false).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_with_failures_and_simultaneous_syncs() {
    for seed in seeds(100..130) {
        eprintln!("seed {seed}");
        let _guard = SeedReporter(seed);
        run(Kind::FileSystem, seed, 150, 40, true, false).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mock_nextcloud_with_failures_and_simultaneous_syncs() {
    for seed in seeds(200..215) {
        eprintln!("seed {seed}");
        let _guard = SeedReporter(seed);
        run(Kind::MockNextcloud, seed, 150, 40, true, false).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_nextcloud_with_failures_and_simultaneous_syncs() {
    for seed in seeds(300..302) {
        eprintln!("seed {seed}");
        let _guard = SeedReporter(seed);
        run(Kind::Nextcloud, seed, 80, 40, true, true).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_encrypted() {
    for seed in seeds(400..415) {
        eprintln!("seed {seed}");
        let _guard = SeedReporter(seed);
        run(Kind::FileSystem, seed, 150, 40, true, true).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mock_nextcloud_encrypted() {
    for seed in seeds(500..515) {
        eprintln!("seed {seed}");
        let _guard = SeedReporter(seed);
        run(Kind::MockNextcloud, seed, 150, 40, true, true).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mock_onedrive_with_failures_and_simultaneous_syncs() {
    for seed in seeds(600..615) {
        eprintln!("seed {seed}");
        let _guard = SeedReporter(seed);
        run(Kind::MockOneDrive, seed, 150, 40, true, seed % 2 == 0).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mock_google_drive_with_failures_and_simultaneous_syncs() {
    for seed in seeds(700..715) {
        eprintln!("seed {seed}");
        let _guard = SeedReporter(seed);
        run(Kind::MockGoogleDrive, seed, 150, 40, true, seed % 2 == 0).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_onedrive_with_failures_and_simultaneous_syncs() {
    for seed in seeds(800..802) {
        eprintln!("seed {seed}");
        let _guard = SeedReporter(seed);
        run(Kind::OneDrive, seed, 80, 40, true, true).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_google_drive_with_failures_and_simultaneous_syncs() {
    for seed in seeds(900..902) {
        eprintln!("seed {seed}");
        let _guard = SeedReporter(seed);
        run(Kind::GoogleDrive, seed, 80, 40, true, true).await;
    }
}
