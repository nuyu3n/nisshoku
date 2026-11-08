/// Generates a new item ID: 32 lowercase hex characters (a UUID v4 without dashes).
pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Whether `s` has the shape of an item ID.
pub fn is_valid_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_alphanumeric())
}
