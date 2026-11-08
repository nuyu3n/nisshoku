//! Merging items' `user_data` when two clients changed the same item.
//!
//! The layout is Joplin's: `{ "<namespace>": { "<key>": { "v": value,
//! "t": updated_ms, "d": 1 if deleted } } }`. Each key is merged on its own:
//! the entry with the later `t` wins (the remote one on a tie), so that
//! changes made to different keys on two devices both survive.

use serde_json::{Map, Value};

fn entry_time(v: &Value) -> i64 {
    v.get("t").and_then(Value::as_i64).unwrap_or(0)
}

/// `remote` with the keys `local` changed later merged in. Text that is not
/// in this layout is left to `remote`.
pub fn merge(local: &str, remote: &str) -> String {
    let parse = |s: &str| -> Option<Map<String, Value>> {
        if s.trim().is_empty() {
            return Some(Map::new());
        }
        serde_json::from_str::<Value>(s).ok()?.as_object().cloned()
    };
    let (Some(local_map), Some(mut out)) = (parse(local), parse(remote)) else {
        return remote.to_string();
    };
    let mut changed = false;
    for (ns, local_ns) in local_map {
        let Some(local_ns) = local_ns.as_object() else { continue };
        let out_ns = out.entry(ns).or_insert_with(|| Value::Object(Map::new()));
        let Some(out_ns) = out_ns.as_object_mut() else { continue };
        for (key, local_entry) in local_ns {
            let newer = match out_ns.get(key) {
                None => true,
                Some(remote_entry) => entry_time(local_entry) > entry_time(remote_entry),
            };
            if newer {
                out_ns.insert(key.clone(), local_entry.clone());
                changed = true;
            }
        }
    }
    if !changed {
        return remote.to_string();
    }
    Value::Object(out).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_changed_on_each_side_both_survive() {
        let base = r#"{"app":{"a":{"v":1,"t":10},"b":{"v":1,"t":10}}}"#;
        let local = r#"{"app":{"a":{"v":2,"t":20},"b":{"v":1,"t":10}}}"#;
        let remote = r#"{"app":{"a":{"v":1,"t":10},"b":{"v":3,"t":30}}}"#;
        let merged: Value = serde_json::from_str(&merge(local, remote)).unwrap();
        assert_eq!(merged["app"]["a"]["v"], 2);
        assert_eq!(merged["app"]["b"]["v"], 3);
        assert_eq!(merge(base, base), base);
        // A deletion is an entry too.
        let deleted = r#"{"app":{"a":{"v":1,"t":40,"d":1}}}"#;
        let merged: Value = serde_json::from_str(&merge(deleted, local)).unwrap();
        assert_eq!(merged["app"]["a"]["d"], 1);
        // Not this layout: the remote text.
        assert_eq!(merge("x", remote), remote);
        let merged: Value = serde_json::from_str(&merge(local, "")).unwrap();
        assert_eq!(merged, serde_json::from_str::<Value>(local).unwrap());
    }
}
