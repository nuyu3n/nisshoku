//! Signs in to OneDrive or Google Drive in the browser and prints the
//! refresh token (for the development CLI and the tests against the real
//! services).
//!
//! ```text
//! cargo run --example login -- onedrive <client-id>
//! cargo run --example login -- gdrive <client-id> <client-secret>
//! ```
//!
//! - OneDrive: an app registration in Microsoft Entra ID (any account type,
//!   "Mobile and desktop applications" platform with redirect URI
//!   `http://localhost`, public client flows allowed) with the delegated
//!   permission `Files.ReadWrite`.
//! - Google Drive: an OAuth client of type "Desktop app" in a Google Cloud
//!   project with the Drive API enabled; while the consent screen is in
//!   testing mode, the account must be listed as a test user.

use nisshoku_sync::drivers::BrowserLogin;
use nisshoku_sync::drivers::google_drive::{GoogleDriveEndpoints, google_drive_oauth};
use nisshoku_sync::drivers::onedrive::{OneDriveEndpoints, onedrive_oauth};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn open_browser(url: &str) {
    let result = if cfg!(windows) {
        // Not `cmd /C start`: cmd would split the URL at `&`.
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    if result.is_err() {
        eprintln!("(could not open a browser)");
    }
}

#[tokio::main]
async fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let provider = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["onedrive", client_id] => onedrive_oauth(client_id, &OneDriveEndpoints::default()),
        ["gdrive", client_id, secret] => {
            google_drive_oauth(client_id, secret, &GoogleDriveEndpoints::default())
        }
        _ => {
            return Err(
                "usage: login onedrive <client-id> | login gdrive <client-id> <client-secret>"
                    .into(),
            );
        }
    };
    let login = BrowserLogin::start(provider).await?;
    eprintln!(
        "Sign in in the browser. If it does not open, visit:\n\n{}\n",
        login.url()
    );
    open_browser(login.url());
    let tokens = login.finish().await?;
    println!("{}", tokens.refresh_token);
    Ok(())
}
