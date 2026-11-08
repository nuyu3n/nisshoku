//! App data stored in items' `user_data`, so that it syncs (also through
//! Joplin, which keeps it). The layout follows Joplin's:
//! `{ "<namespace>": { "<key>": { "v": value, "t": updated_ms } } }`.

use serde_json::{Map, Value, json};

const NAMESPACE: &str = "app.nisshoku";

pub fn get(user_data: &str, key: &str) -> Option<Value> {
    let root: Value = serde_json::from_str(user_data).ok()?;
    let entry = root.get(NAMESPACE)?.get(key)?;
    if entry.get("d").and_then(Value::as_i64) == Some(1) {
        return None;
    }
    entry.get("v").cloned()
}

/// Returns the new `user_data`. `None` removes the key.
pub fn set(user_data: &str, key: &str, value: Option<Value>) -> String {
    let mut root: Map<String, Value> = serde_json::from_str(user_data).unwrap_or_default();
    let ns = root
        .entry(NAMESPACE)
        .or_insert_with(|| Value::Object(Map::new()));
    if !ns.is_object() {
        *ns = Value::Object(Map::new());
    }
    let ns = ns.as_object_mut().expect("object");
    match value {
        Some(v) => {
            ns.insert(
                key.into(),
                json!({ "v": v, "t": nisshoku_core::time::now_ms() }),
            );
        }
        // A key that was set is marked deleted rather than dropped, so that
        // merging with another device's copy (see nisshoku_core::user_data)
        // does not bring it back.
        None => {
            if ns.contains_key(key) {
                ns.insert(key.into(), json!({ "d": 1, "t": nisshoku_core::time::now_ms() }));
            }
        }
    }
    if ns.is_empty() {
        root.remove(NAMESPACE);
    }
    if root.is_empty() {
        String::new()
    } else {
        Value::Object(root).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeping_other_namespaces() {
        let other = r#"{"org.joplinapp.plugin":{"x":{"v":1,"t":5}}}"#;
        let s = set(other, "goal", Some(json!(3000)));
        assert_eq!(get(&s, "goal"), Some(json!(3000)));
        assert!(s.contains("org.joplinapp.plugin"));
        let s = set(&s, "goal", None);
        assert_eq!(get(&s, "goal"), None);
        let parse = |s: &str| serde_json::from_str::<Value>(s).unwrap();
        assert_eq!(parse(&s)["org.joplinapp.plugin"], parse(other)["org.joplinapp.plugin"]);
        assert_eq!(parse(&s)[NAMESPACE]["goal"]["d"], 1);
        assert_eq!(set("", "goal", None), "");
    }
}
