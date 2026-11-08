//! Text diffs for note revisions, as patch objects of Neil Fraser's
//! diff-match-patch serialized to JSON (the form Joplin stores).
//!
//! Positions and lengths count UTF-16 code units, as the library's JavaScript
//! version works on JavaScript strings. Diffs are computed on whole
//! characters, so a surrogate pair is never split.
//!
//! Patches are applied to the exact text they were made from (revisions form
//! a chain), so applying looks for each hunk at its expected position and,
//! failing that, elsewhere in the text, without fuzzy matching.

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Equal,
    Delete,
    Insert,
}

pub type Diff = (Op, Vec<char>);

/// Characters of context kept around each change.
const MARGIN: usize = 4;
/// Above this many edit steps, fall back to a plain replace of the middle part.
const MAX_EDIT_DISTANCE: usize = 4_000;

// -------------------------------------------------------------------------
// Diff (Myers, on characters)

pub fn diff(a: &[char], b: &[char]) -> Vec<Diff> {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let (a_rest, b_rest) = (&a[prefix..], &b[prefix..]);
    let suffix = a_rest
        .iter()
        .rev()
        .zip(b_rest.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (a_mid, b_mid) = (
        &a_rest[..a_rest.len() - suffix],
        &b_rest[..b_rest.len() - suffix],
    );

    let mut out = Vec::new();
    if prefix > 0 {
        out.push((Op::Equal, a[..prefix].to_vec()));
    }
    out.extend(cleanup_semantic(diff_middle(a_mid, b_mid)));
    if suffix > 0 {
        out.push((Op::Equal, a_rest[a_rest.len() - suffix..].to_vec()));
    }
    merge(out)
}

/// Absorbs short equalities surrounded by larger edits into those edits,
/// which turns "-b r -own +ed" into "-brown +red".
fn cleanup_semantic(mut diffs: Vec<Diff>) -> Vec<Diff> {
    loop {
        let mut changed = false;
        let edit_len = |d: &[Diff]| -> usize {
            d.iter()
                .filter(|(o, _)| *o != Op::Equal)
                .map(|(_, t)| t.len())
                .max()
                .unwrap_or(0)
        };
        let mut i = 0;
        while i < diffs.len() {
            if diffs[i].0 == Op::Equal {
                let before_start = diffs[..i]
                    .iter()
                    .rposition(|(o, _)| *o == Op::Equal)
                    .map_or(0, |p| p + 1);
                let after_end = diffs[i + 1..]
                    .iter()
                    .position(|(o, _)| *o == Op::Equal)
                    .map_or(diffs.len(), |p| i + 1 + p);
                let before = edit_len(&diffs[before_start..i]);
                let after = edit_len(&diffs[i + 1..after_end]);
                let len = diffs[i].1.len();
                if before > 0 && after > 0 && len <= before && len <= after {
                    let text = diffs[i].1.clone();
                    diffs.splice(i..=i, [(Op::Delete, text.clone()), (Op::Insert, text)]);
                    diffs = merge(diffs);
                    changed = true;
                    break;
                }
            }
            i += 1;
        }
        if !changed {
            return diffs;
        }
    }
}

fn diff_middle(a: &[char], b: &[char]) -> Vec<Diff> {
    match (a.is_empty(), b.is_empty()) {
        (true, true) => return Vec::new(),
        (true, false) => return vec![(Op::Insert, b.to_vec())],
        (false, true) => return vec![(Op::Delete, a.to_vec())],
        _ => {}
    }
    myers(a, b).unwrap_or_else(|| vec![(Op::Delete, a.to_vec()), (Op::Insert, b.to_vec())])
}

/// Greedy Myers diff keeping the frontier of each step for the backtrack.
/// `None` if the texts differ by more than `MAX_EDIT_DISTANCE` steps.
fn myers(a: &[char], b: &[char]) -> Option<Vec<Diff>> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    let max = (n + m).min(MAX_EDIT_DISTANCE as isize);
    let offset = max + 1;
    let mut v = vec![0isize; 2 * offset as usize + 1];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    for d in 0..=max {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let idx = (k + offset) as usize;
            let mut x = if k == -d || (k != d && v[idx - 1] < v[idx + 1]) {
                v[idx + 1]
            } else {
                v[idx - 1] + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[idx] = x;
            if x >= n && y >= m {
                return Some(backtrack(a, b, &trace, d, offset));
            }
            k += 2;
        }
    }
    None
}

fn backtrack(
    a: &[char],
    b: &[char],
    trace: &[Vec<isize>],
    d_end: isize,
    offset: isize,
) -> Vec<Diff> {
    let (mut x, mut y) = (a.len() as isize, b.len() as isize);
    let mut ops: Vec<(Op, char)> = Vec::new();
    for d in (1..=d_end).rev() {
        let v = &trace[d as usize];
        let k = x - y;
        let idx = (k + offset) as usize;
        let prev_k = if k == -d || (k != d && v[idx - 1] < v[idx + 1]) {
            k + 1
        } else {
            k - 1
        };
        let prev_x = v[(prev_k + offset) as usize];
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            ops.push((Op::Equal, a[x as usize]));
        }
        if x == prev_x {
            y -= 1;
            ops.push((Op::Insert, b[y as usize]));
        } else {
            x -= 1;
            ops.push((Op::Delete, a[x as usize]));
        }
    }
    while x > 0 && y > 0 {
        x -= 1;
        y -= 1;
        ops.push((Op::Equal, a[x as usize]));
    }
    ops.reverse();
    merge(ops.into_iter().map(|(op, c)| (op, vec![c])).collect())
}

/// Joins consecutive diffs of the same kind; deletions before insertions.
fn merge(diffs: Vec<Diff>) -> Vec<Diff> {
    let mut out: Vec<Diff> = Vec::new();
    let (mut del, mut ins) = (Vec::new(), Vec::new());
    let flush = |out: &mut Vec<Diff>, del: &mut Vec<char>, ins: &mut Vec<char>| {
        if !del.is_empty() {
            out.push((Op::Delete, std::mem::take(del)));
        }
        if !ins.is_empty() {
            out.push((Op::Insert, std::mem::take(ins)));
        }
    };
    for (op, text) in diffs {
        match op {
            Op::Delete => del.extend(text),
            Op::Insert => ins.extend(text),
            Op::Equal => {
                flush(&mut out, &mut del, &mut ins);
                match out.last_mut() {
                    Some((Op::Equal, prev)) => prev.extend(text),
                    _ if !text.is_empty() => out.push((Op::Equal, text)),
                    _ => {}
                }
            }
        }
    }
    flush(&mut out, &mut del, &mut ins);
    out
}

// -------------------------------------------------------------------------
// Patches

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Patch {
    pub diffs: Vec<Diff>,
    /// Start in the old and new text, in characters.
    pub start1: usize,
    pub start2: usize,
    pub length1: usize,
    pub length2: usize,
}

/// Patches turning `a` into `b`.
pub fn patch_make(a: &[char], b: &[char]) -> Vec<Patch> {
    let diffs = diff(a, b);
    let mut patches = Vec::new();
    let mut patch = Patch::default();
    // Positions in the old and new text.
    let (mut pos1, mut pos2) = (0usize, 0usize);
    for (i, (op, text)) in diffs.iter().enumerate() {
        if patch.diffs.is_empty() && *op != Op::Equal {
            // Start a patch with leading context.
            let ctx_start = pos1.saturating_sub(MARGIN);
            let ctx = a[ctx_start..pos1].to_vec();
            patch.start1 = ctx_start;
            patch.start2 = pos2 - ctx.len();
            patch.length1 = ctx.len();
            patch.length2 = ctx.len();
            if !ctx.is_empty() {
                patch.diffs.push((Op::Equal, ctx));
            }
        }
        match op {
            Op::Insert => {
                patch.diffs.push((Op::Insert, text.clone()));
                patch.length2 += text.len();
                pos2 += text.len();
            }
            Op::Delete => {
                patch.diffs.push((Op::Delete, text.clone()));
                patch.length1 += text.len();
                pos1 += text.len();
            }
            Op::Equal => {
                let last = i + 1 == diffs.len();
                if !patch.diffs.is_empty() && (text.len() > 2 * MARGIN || last) {
                    // Close the patch with trailing context.
                    let ctx: Vec<char> = text.iter().take(MARGIN).copied().collect();
                    patch.length1 += ctx.len();
                    patch.length2 += ctx.len();
                    if !ctx.is_empty() {
                        patch.diffs.push((Op::Equal, ctx));
                    }
                    patches.push(std::mem::take(&mut patch));
                } else if !patch.diffs.is_empty() {
                    // Short equality inside a patch: keep it.
                    patch.diffs.push((Op::Equal, text.clone()));
                    patch.length1 += text.len();
                    patch.length2 += text.len();
                }
                pos1 += text.len();
                pos2 += text.len();
            }
        }
    }
    if !patch.diffs.is_empty() {
        patches.push(patch);
    }
    patches
}

/// A patch as serialized by Joplin: the library's patch objects as JSON,
/// with positions and lengths in UTF-16 code units.
///
/// ```json
/// [{"diffs":[[0,"ick "],[-1,"brown"],[1,"red"],[0," fox"]],"start1":6,"start2":6,"length1":13,"length2":11}]
/// ```
#[derive(serde::Serialize, serde::Deserialize)]
struct JsonPatch {
    diffs: Vec<(i8, String)>,
    start1: usize,
    start2: usize,
    length1: usize,
    length2: usize,
}

/// UTF-16 offset of each character position (one more entry than characters).
fn utf16_offsets(chars: &[char]) -> Vec<usize> {
    let mut out = Vec::with_capacity(chars.len() + 1);
    let mut units = 0;
    out.push(0);
    for c in chars {
        units += c.len_utf16();
        out.push(units);
    }
    out
}

/// The diff turning `old` into `new`, in Joplin's JSON form. `"[]"` if equal.
pub fn diff_json(old: &str, new: &str) -> String {
    let a: Vec<char> = old.chars().collect();
    let b: Vec<char> = new.chars().collect();
    let (oa, ob) = (utf16_offsets(&a), utf16_offsets(&b));
    let patches: Vec<JsonPatch> = patch_make(&a, &b)
        .into_iter()
        .map(|p| JsonPatch {
            start1: oa[p.start1],
            start2: ob[p.start2],
            length1: oa[p.start1 + p.length1] - oa[p.start1],
            length2: ob[p.start2 + p.length2] - ob[p.start2],
            diffs: p
                .diffs
                .into_iter()
                .map(|(op, text)| {
                    let op = match op {
                        Op::Delete => -1,
                        Op::Equal => 0,
                        Op::Insert => 1,
                    };
                    (op, text.into_iter().collect())
                })
                .collect(),
        })
        .collect();
    serde_json::to_string(&patches).expect("serializable")
}

/// The occurrence of `needle` closest to `expected`, if any.
fn find_near(haystack: &[char], needle: &[char], expected: usize) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    let at = |i: usize| haystack.get(i..i + needle.len()) == Some(needle);
    if at(expected) {
        return Some(expected);
    }
    (0..=haystack.len() - needle.len())
        .filter(|&i| at(i))
        .min_by_key(|&i| i.abs_diff(expected))
}

/// Lengths (in characters) of the unchanged context at both ends of a patch.
fn context_lengths(diffs: &[(i8, String)]) -> (usize, usize) {
    let len = |d: Option<&(i8, String)>| match d {
        Some((0, t)) => t.chars().count(),
        _ => 0,
    };
    let lead = len(diffs.first());
    let trail = if diffs.len() > 1 {
        len(diffs.last())
    } else {
        0
    };
    (lead, trail)
}

/// Applies a diff in Joplin's JSON form (empty means no change). Like the
/// library's `patch_apply`, hunks that no longer fit are applied as well as
/// possible or skipped, never failing the whole diff.
pub fn apply_json(diff: &str, text: &str) -> Result<String> {
    if diff.trim().is_empty() {
        return Ok(text.to_string());
    }
    let patches: Vec<JsonPatch> = serde_json::from_str(diff)
        .map_err(|e| Error::Parse(format!("invalid revision diff: {e}")))?;
    let mut chars: Vec<char> = text.chars().collect();
    for p in patches {
        let mut old = Vec::new();
        let mut new = Vec::new();
        for (op, t) in &p.diffs {
            match op {
                0 => {
                    old.extend(t.chars());
                    new.extend(t.chars());
                }
                -1 => old.extend(t.chars()),
                1 => new.extend(t.chars()),
                _ => return Err(Error::Parse(format!("invalid diff operation {op}"))),
            }
        }
        // Expected position: start2 (UTF-16) in the text patched so far.
        let offsets = utf16_offsets(&chars);
        let expected = offsets
            .iter()
            .position(|&u| u >= p.start2)
            .unwrap_or(chars.len());
        let (lead, trail) = context_lengths(&p.diffs);
        let core_old =
            &old[lead.min(old.len())..old.len().saturating_sub(trail).max(lead.min(old.len()))];
        let core_new =
            &new[lead.min(new.len())..new.len().saturating_sub(trail).max(lead.min(new.len()))];
        if let Some(pos) = find_near(&chars, &old, expected) {
            chars.splice(pos..pos + old.len(), new);
        } else if !core_old.is_empty() {
            // The context changed (e.g. a revision made concurrently on another
            // device): apply the change itself where it is found.
            if let Some(pos) = find_near(&chars, core_old, expected + lead) {
                chars.splice(pos..pos + core_old.len(), core_new.iter().copied());
            }
        } else {
            // A pure insertion: insert at its position.
            let pos = (expected + lead).min(chars.len());
            chars.splice(pos..pos, core_new.iter().copied());
        }
    }
    Ok(chars.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(old: &str, new: &str) {
        let d = diff_json(old, new);
        assert_eq!(apply_json(&d, old).unwrap(), new, "diff: {d}");
    }

    #[test]
    fn diffs_round_trip() {
        round_trip("", "");
        round_trip("", "new text");
        round_trip("old text", "");
        round_trip("The quick brown fox", "The quick red fox jumps");
        round_trip(
            "一行目\n二行目\n三行目",
            "一行目\n二行目（版2で編集）\n三行目\n追加行 🌒",
        );
        round_trip("🌒🌑🌘 emoji", "🌒🌘 emoji 🌑");
        round_trip(
            "a\nb\nc\nd\ne\nf\ng\nh\ni\nj",
            "a\nB\nc\nd\ne\nf\ng\nh\nI\nj",
        );
        round_trip("100% & <tag> \"q\" %20", "100% & <tag> \"quoted\" %20 +");
        let long_a: String = (0..3000).map(|i| format!("line {i}\n")).collect();
        let long_b: String = (0..3000)
            .map(|i| {
                if i % 7 == 0 {
                    format!("changed {i}\n")
                } else {
                    format!("line {i}\n")
                }
            })
            .collect();
        round_trip(&long_a, &long_b);
        round_trip(&"a".repeat(20_000), &"b".repeat(20_000)); // beyond the edit limit
    }

    #[test]
    fn format_matches_joplin() {
        // A first revision, as written by Joplin desktop 3.7.
        assert_eq!(
            diff_json("", "版3のタイトル"),
            r#"[{"diffs":[[1,"版3のタイトル"]],"start1":0,"start2":0,"length1":0,"length2":7}]"#
        );
        assert_eq!(diff_json("same", "same"), "[]");
        assert_eq!(
            diff_json("The quick brown fox", "The quick red fox"),
            r#"[{"diffs":[[0,"ick "],[-1,"brown"],[1,"red"],[0," fox"]],"start1":6,"start2":6,"length1":13,"length2":11}]"#
        );
        // Positions count UTF-16 units: the emoji counts as two.
        assert_eq!(
            diff_json("🌒 abcdefgh", "🌒 abcdXefgh"),
            r#"[{"diffs":[[0,"abcd"],[1,"X"],[0,"efgh"]],"start1":3,"start2":3,"length1":8,"length2":9}]"#
        );
        // A hunk whose context no longer matches still applies its change.
        let d = diff_json("one two three four five", "one two THREE four five");
        assert_eq!(
            apply_json(&d, "zero one two three four five six").unwrap(),
            "zero one two THREE four five six"
        );
        assert_eq!(
            apply_json(&d, "one 2 three 4 five").unwrap(),
            "one 2 THREE 4 five"
        );
        assert_eq!(
            apply_json(&d, "nothing in common").unwrap(),
            "nothing in common"
        );
        // Joplin's own first revision applies.
        let body = r#"[{"diffs":[[1,"全面的に書き直した本文。\n\n新しい段落。"]],"start1":0,"start2":0,"length1":0,"length2":20}]"#;
        assert_eq!(
            apply_json(body, "").unwrap(),
            "全面的に書き直した本文。\n\n新しい段落。"
        );
    }
}
