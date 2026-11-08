//! Development helper: turns on encryption for a profile's target.
//! `cargo run --example enable_e2ee -- <profile-dir> <password>` (target from the same env vars as `nisshoku`).

use std::sync::{Arc, Mutex};

use nisshoku_core::Database;
use nisshoku_sync::drivers::{NextcloudConfig, NextcloudStorage};
use nisshoku_sync::{SyncOptions, Synchronizer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let profile = std::path::PathBuf::from(&args[0]);
    let config = NextcloudConfig {
        server_url: std::env::var("NISSHOKU_NEXTCLOUD_URL")?,
        username: std::env::var("NISSHOKU_NEXTCLOUD_USER")?,
        password: std::env::var("NISSHOKU_NEXTCLOUD_PASSWORD")?,
        folder: std::env::var("NISSHOKU_NEXTCLOUD_FOLDER")?,
    };
    let db = Arc::new(Mutex::new(Database::open(profile.join("database.sqlite"))?));
    let mut sync = Synchronizer::new(
        db,
        Arc::new(NextcloudStorage::connect(&config).await?),
        SyncOptions {
            target_id: 1,
            resource_dir: profile.join("resources"),
            client_type: "desktop".into(),
            ..Default::default()
        },
    );
    sync.enable_encryption(&args[1]).await?;
    println!("encryption enabled");
    Ok(())
}
