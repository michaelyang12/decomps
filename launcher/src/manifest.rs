//! shortcuts.json: the one list of games, shared with tools/sync_shortcuts.py.

use serde::Deserialize;
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Manifest {
    shortcuts: Vec<Entry>,
}

#[derive(Deserialize, Clone)]
pub struct Entry {
    /// Also the name of exes/<name>.lnk; the stable id of the tile.
    pub name: String,
    /// Relative to the Decomps root; `*`/`?` glob segments pick the highest natural-sorted match.
    pub target: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Original console, e.g. "Nintendo 64". Picks the libretro-thumbnails system.
    #[serde(default)]
    pub platform: Option<String>,
    /// Short description of the build, e.g. "Static recomp".
    #[serde(default)]
    pub port: Option<String>,
    #[serde(default)]
    pub art: ArtSpec,
}

#[derive(Deserialize, Clone, Default)]
pub struct ArtSpec {
    /// No-Intro name in libretro-thumbnails, e.g. "Super Mario 64 (USA)".
    #[serde(default)]
    pub libretro: Option<String>,
    /// libretro-thumbnails system folder. Default: "Nintendo - <platform>".
    #[serde(default, rename = "libretroSystem")]
    pub libretro_system: Option<String>,
    /// SteamGridDB game id (number) or search term (string). Default: search by title.
    #[serde(default)]
    pub steamgriddb: Option<serde_json::Value>,
}

impl Entry {
    pub fn id(&self) -> String {
        slug(&self.name)
    }
    pub fn title(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.name)
    }
    /// Art folder under art/cache and art/custom. Keyed by title so tiles of the same game share art.
    pub fn art_key(&self) -> String {
        slug(self.title())
    }
}

pub fn load(root: &Path) -> Result<Vec<Entry>, String> {
    let path = root.join("shortcuts.json");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let m: Manifest = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(m.shortcuts)
}

pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

// ---------- root ----------

fn is_root(p: &Path) -> bool {
    p.join("shortcuts.json").is_file() && p.join("Games").is_dir()
}

/// --root <dir>, else DECOMPS_ROOT, else the nearest parent of the exe that holds shortcuts.json and Games/,
/// else %USERPROFILE%\Decomps.
pub fn find_root() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--root") {
        if let Some(p) = args.get(i + 1) {
            return PathBuf::from(p);
        }
    }
    if let Ok(p) = std::env::var("DECOMPS_ROOT") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(found) = exe.ancestors().skip(1).find(|p| is_root(p)) {
            return found.to_path_buf();
        }
    }
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Decomps")
}

/// KEY=VALUE lines from <root>/.env (gitignored). The process environment wins.
pub fn env_var(root: &Path, key: &str) -> Option<String> {
    if let Ok(v) = std::env::var(key) {
        if !v.trim().is_empty() {
            return Some(v.trim().to_string());
        }
    }
    let text = fs::read_to_string(root.join(".env")).ok()?;
    text.lines().find_map(|line| {
        let (k, v) = line.trim().split_once('=')?;
        if k.trim() != key {
            return None;
        }
        let v = v.trim().trim_matches(|c| c == '"' || c == '\'');
        (!v.is_empty()).then(|| v.to_string())
    })
}

// ---------- targets ----------

pub fn resolve(root: &Path, rel: &str) -> Option<PathBuf> {
    let mut cands = vec![root.to_path_buf()];
    for seg in rel.split(['/', '\\']).filter(|s| !s.is_empty()) {
        let mut next = Vec::new();
        for c in &cands {
            if seg.contains(['*', '?']) {
                if let Ok(rd) = fs::read_dir(c) {
                    for e in rd.flatten() {
                        if wildmatch(seg, &e.file_name().to_string_lossy()) {
                            next.push(e.path());
                        }
                    }
                }
            } else {
                let p = c.join(seg);
                if p.exists() {
                    next.push(p);
                }
            }
        }
        cands = next;
    }
    cands.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
    cands.pop()
}

fn wildmatch(pat: &str, s: &str) -> bool {
    fn go(p: &[char], s: &[char]) -> bool {
        match p.first() {
            None => s.is_empty(),
            Some('*') => (0..=s.len()).any(|i| go(&p[1..], &s[i..])),
            Some('?') => !s.is_empty() && go(&p[1..], &s[1..]),
            Some(c) => s.first().is_some_and(|x| x.eq_ignore_ascii_case(c)) && go(&p[1..], &s[1..]),
        }
    }
    go(&pat.chars().collect::<Vec<_>>(), &s.chars().collect::<Vec<_>>())
}

/// Same order as natural_key() in tools/sync_shortcuts.py: digit runs compare as numbers.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn tokens(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let d = c.is_ascii_digit();
            match out.last_mut() {
                Some((ld, t)) if *ld == d => t.push(c),
                _ => out.push((d, c.to_string())),
            }
        }
        out
    }
    let (ta, tb) = (tokens(a), tokens(b));
    for (x, y) in ta.iter().zip(tb.iter()) {
        let o = match (x.0, y.0) {
            (true, true) => {
                let (xn, yn) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
                xn.len().cmp(&yn.len()).then_with(|| xn.cmp(yn))
            }
            _ => x.1.to_lowercase().cmp(&y.1.to_lowercase()),
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    ta.len().cmp(&tb.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        assert_eq!(natural_cmp("v/1.0.10", "v/1.0.9"), Ordering::Greater);
        assert_eq!(natural_cmp("v/2.0", "v/10.0"), Ordering::Less);
    }

    #[test]
    fn globbing() {
        assert!(wildmatch("*", "1.0.2"));
        assert!(wildmatch("nebula.EXE", "Nebula.exe"));
        assert!(!wildmatch("a?c", "ac"));
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Super Mario 64 (sm64ex-alo)"), "super-mario-64-sm64ex-alo");
        assert_eq!(slug("The Legend of Zelda: Tears of the Kingdom"), "the-legend-of-zelda-tears-of-the-kingdom");
    }
}
