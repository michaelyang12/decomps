//! Library scan, like an emulator frontend's: every folder directly inside a scan dir is one game.
//! Folders that no shortcuts.json entry covers get entries appended, with a guessed launch target,
//! so the manifest stays the one list (exes/ and tools/sync_shortcuts.py keep working). A wrong guess
//! is fixed by editing the entry; a folder that isn't a game goes in the manifest's "ignore" list.
//!
//! Target guess, per folder:
//!   1. `Play*.bat`/`Play*.cmd` scripts (up to 2 levels down), one tile each: "Play X (modded)" -> port "modded".
//!   2. else an exe whose name matches the folder name (Nebula/…/Nebula.exe),
//!   3. else the largest exe that isn't an installer, uninstaller, settings app or build tool.
//! Version folders in the path (1.0.2, v2.1) become `*`, so updates keep resolving.

use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

const DEPTH: usize = 4;
/// Folders that hold build tools, dependencies or data, never the thing you launch.
const SKIP_DIRS: &[&str] = &[
    ".git", "toolchain", "deps", "node_modules", "build-cache", "decomp", "sdk", "dump", "romfs_sky",
    "content", "game-inputs", "cache", "__pycache__", "tools", "work", "staging", "source",
    "relink-cache", "extracted", "third_party", "external", "vendor", "cmakefiles", "_deps",
];
/// Exe name fragments that mark helpers rather than games.
const JUNK: &[&str] = &["setup", "install", "uninst", "unins", "update", "settings", "config", "crash", "redist"];

pub struct Report {
    pub added: Vec<String>,
    /// Folders with nothing launchable in them.
    pub skipped: Vec<String>,
}

struct File {
    rel: String,
    stem: String,
    ext: String,
    size: u64,
    depth: usize,
}

fn norm(p: &str) -> String {
    p.replace('\\', "/").trim_start_matches("./").trim_end_matches('/').to_lowercase()
}

fn alnum(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

fn walk(dir: &Path, rel: &str, depth: usize, out: &mut Vec<File>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let child_rel = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            if depth < DEPTH && !SKIP_DIRS.contains(&name.to_lowercase().as_str()) {
                walk(&e.path(), &child_rel, depth + 1, out);
            }
        } else if let Some((stem, ext)) = name.rsplit_once('.') {
            let ext = ext.to_lowercase();
            if ["exe", "bat", "cmd"].contains(&ext.as_str()) {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                out.push(File { rel: child_rel, stem: stem.to_string(), ext, size, depth });
            }
        }
    }
}

fn is_version(seg: &str) -> bool {
    let parts: Vec<&str> = seg.trim_start_matches(['v', 'V']).split('.').collect();
    parts.len() >= 2 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// The folder's launch targets (relative to it) with an optional port label.
fn guess(folder: &Path, folder_name: &str) -> Vec<(String, String, Option<String>)> {
    let mut files = Vec::new();
    walk(folder, "", 0, &mut files);

    let mut plays: Vec<&File> = files
        .iter()
        .filter(|f| f.ext != "exe" && f.depth <= 2 && f.stem.to_lowercase().starts_with("play"))
        .collect();
    if !plays.is_empty() {
        // The plain "Play X" first, so variants ("Play X (modded)") are the ones that get a badge.
        plays.sort_by_key(|f| (f.stem.contains('('), f.depth, f.stem.to_lowercase()));
        return plays
            .into_iter()
            .map(|f| {
                let port = f.stem.split_once('(').map(|(_, r)| r.trim_end_matches(')').trim().to_string());
                (f.rel.clone(), f.stem.clone(), port)
            })
            .collect();
    }

    let exes: Vec<&File> = files
        .iter()
        .filter(|f| f.ext == "exe" && !JUNK.iter().any(|j| f.stem.to_lowercase().contains(j)))
        .collect();
    let want = alnum(folder_name);
    let pick = exes
        .iter()
        .filter(|f| alnum(&f.stem) == want)
        .min_by_key(|f| f.depth)
        .or_else(|| exes.iter().max_by_key(|f| f.size));
    pick.map(|f| vec![(f.rel.clone(), folder_name.to_string(), None)]).unwrap_or_default()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// Scan dirs from the manifest's "scan" list (relative to the root, or absolute). Default: ["Games"].
pub fn scan(root: &Path) -> Result<Report, String> {
    let path = root.join("shortcuts.json");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut doc: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let dirs = if doc.get("scan").is_some() { strings(&doc["scan"]) } else { vec!["Games".to_string()] };
    let ignore: Vec<String> = strings(&doc["ignore"]).iter().map(|s| norm(s)).collect();
    let entries = doc["shortcuts"].as_array().cloned().unwrap_or_default();
    let covered: Vec<String> = entries.iter().filter_map(|e| e["target"].as_str()).map(norm).collect();
    let mut names: HashSet<String> = entries.iter().filter_map(|e| e["name"].as_str()).map(str::to_lowercase).collect();

    let mut report = Report { added: Vec::new(), skipped: Vec::new() };
    let mut new_entries = Vec::new();
    for d in dirs {
        let d = d.replace('\\', "/").trim_end_matches('/').to_string();
        let Ok(rd) = fs::read_dir(root.join(&d)) else { continue };
        let mut folders: Vec<_> = rd.flatten().filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false)).collect();
        folders.sort_by_key(|e| e.file_name().to_string_lossy().to_lowercase());
        for f in folders {
            let fname = f.file_name().to_string_lossy().to_string();
            let rel = format!("{d}/{fname}");
            let key = norm(&rel);
            if ignore.contains(&key) || covered.iter().any(|t| t.starts_with(&format!("{key}/"))) {
                continue;
            }
            let picks = guess(&f.path(), &fname);
            if picks.is_empty() {
                report.skipped.push(rel);
                continue;
            }
            for (sub, base_name, port) in picks {
                let sub: Vec<String> = sub.split('/').map(str::to_string).collect();
                let last = sub.len() - 1;
                let sub: Vec<String> =
                    sub.into_iter().enumerate().map(|(i, s)| if i < last && is_version(&s) { "*".into() } else { s }).collect();
                let mut name = base_name.clone();
                let mut n = 2;
                while names.contains(&name.to_lowercase()) {
                    name = format!("{base_name} ({n})");
                    n += 1;
                }
                names.insert(name.to_lowercase());
                let mut e = json!({ "name": name, "target": format!("{rel}/{}", sub.join("/")), "title": fname });
                if let Some(p) = port {
                    e["port"] = json!(p);
                }
                report.added.push(name);
                new_entries.push(e);
            }
        }
    }

    if !new_entries.is_empty() {
        if let Some(a) = doc["shortcuts"].as_array_mut() {
            a.extend(new_entries);
        }
        fs::write(&path, format_manifest(&doc)).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(report)
}

/// Same layout as dump_manifest() in tools/sync_shortcuts.py: one top-level key per line, one entry per line.
pub fn format_manifest(doc: &Value) -> String {
    fn compact(v: &Value) -> String {
        match v {
            Value::Array(a) => format!("[{}]", a.iter().map(compact).collect::<Vec<_>>().join(", ")),
            Value::Object(o) => format!(
                "{{{}}}",
                o.iter().map(|(k, v)| format!("{}: {}", Value::String(k.clone()), compact(v))).collect::<Vec<_>>().join(", ")
            ),
            _ => v.to_string(),
        }
    }
    let Some(obj) = doc.as_object() else { return compact(doc) + "\n" };
    let lines: Vec<String> = obj
        .iter()
        .map(|(k, v)| {
            let val = match (k.as_str(), v) {
                ("shortcuts", Value::Array(a)) if !a.is_empty() => {
                    format!("[\n{}\n  ]", a.iter().map(|e| format!("    {}", compact(e))).collect::<Vec<_>>().join(",\n"))
                }
                _ => compact(v),
            };
            format!("  {}: {val}", Value::String(k.clone()))
        })
        .collect();
    format!("{{\n{}\n}}\n", lines.join(",\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_version("1.0.2"));
        assert!(is_version("v2.1"));
        assert!(!is_version("build"));
        assert!(!is_version("us_pc"));
        assert!(!is_version("2026"));
    }
}
