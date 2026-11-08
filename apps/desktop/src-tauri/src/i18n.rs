//! Language of the messages and names this side produces (errors, default
//! notebook names), following the interface language preference.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

pub fn set_english(english: bool) {
    ENGLISH.store(english, Ordering::Relaxed);
}

pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Picks the Japanese or the English text.
pub fn tr(ja: &'static str, en: &'static str) -> &'static str {
    if english() { en } else { ja }
}

/// `tr` for messages with arguments: `trf!("…{x}…", "…{x}…")`.
#[macro_export]
macro_rules! trf {
    ($ja:literal, $en:literal) => {
        if $crate::i18n::english() {
            format!($en)
        } else {
            format!($ja)
        }
    };
}
