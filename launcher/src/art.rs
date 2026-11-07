//! Game art: <root>/art/custom/<key>/ (hand-placed, wins) over <root>/art/cache/<key>/ (downloaded).
//!
//! Slots: cover (portrait box art), hero (wide background), logo (transparent title).
//! Sources, in order: SteamGridDB (needs STEAMGRIDDB_API_KEY), then libretro-thumbnails
//! (Named_Boxarts -> cover, Named_Snaps -> hero; no key, but no Switch and no logos).
//! Only game titles and ids leave the machine.

use crate::manifest::Entry;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const SLOTS: [&str; 3] = ["cover", "hero", "logo"];
const EXTS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
/// A fetch that left slots empty is retried after this long (or on Refresh art).
const RETRY_SECS: u64 = 7 * 24 * 3600;
const MAX_BYTES: u64 = 40 << 20;
const SGDB: &str = "https://www.steamgriddb.com/api/v2";
const LIBRETRO: &str = "https://thumbnails.libretro.com";

pub fn cache_dir(root: &Path) -> PathBuf {
    root.join("art").join("cache")
}
pub fn custom_dir(root: &Path) -> PathBuf {
    root.join("art").join("custom")
}

fn find(dir: &Path, slot: &str) -> Option<PathBuf> {
    EXTS.iter().map(|e| dir.join(format!("{slot}.{e}"))).find(|p| p.is_file())
}

/// The image to show for a slot: custom first, then cache.
pub fn image(root: &Path, key: &str, slot: &str) -> Option<PathBuf> {
    find(&custom_dir(root).join(key), slot).or_else(|| find(&cache_dir(root).join(key), slot))
}

#[derive(Serialize, Deserialize, Default)]
struct Meta {
    fetched_at: u64,
    with_steamgriddb: bool,
    steamgriddb_id: Option<u64>,
    /// slot -> URL it was downloaded from
    sources: BTreeMap<String, String>,
    errors: Vec<String>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn read_meta(dir: &Path) -> Option<Meta> {
    serde_json::from_str(&fs::read_to_string(dir.join("meta.json")).ok()?).ok()
}

/// Whether an automatic (non-forced) fetch is worth trying for this entry.
pub fn needs_fetch(root: &Path, e: &Entry, has_key: bool) -> bool {
    let key = e.art_key();
    if SLOTS.iter().all(|s| image(root, &key, s).is_some()) {
        return false;
    }
    match read_meta(&cache_dir(root).join(&key)) {
        None => true,
        Some(m) => (has_key && !m.with_steamgriddb) || now().saturating_sub(m.fetched_at) > RETRY_SECS,
    }
}

pub struct Fetcher {
    agent: ureq::Agent,
    key: Option<String>,
}

impl Fetcher {
    pub fn new(sgdb_key: Option<String>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("decomps-launcher/", env!("CARGO_PKG_VERSION")))
            .build();
        Fetcher { agent, key: sgdb_key }
    }

    /// Downloads every slot that has no custom image. A slot that fails keeps its old cached file.
    /// Returns a one-line summary.
    pub fn fetch(&self, root: &Path, e: &Entry) -> String {
        let key = e.art_key();
        let dir = cache_dir(root).join(&key);
        let custom = custom_dir(root).join(&key);
        let _ = fs::create_dir_all(&dir);
        let wanted: Vec<&str> = SLOTS.iter().copied().filter(|s| find(&custom, s).is_none()).collect();
        let mut meta = Meta { fetched_at: now(), with_steamgriddb: self.key.is_some(), ..Default::default() };

        if let Some(api_key) = &self.key {
            match self.sgdb_id(api_key, e) {
                Ok(id) => {
                    meta.steamgriddb_id = Some(id);
                    for slot in &wanted {
                        match self.sgdb_asset(api_key, id, slot).and_then(|u| self.save(&u, &dir, slot).map(|_| u)) {
                            Ok(url) => {
                                meta.sources.insert(slot.to_string(), url);
                            }
                            Err(err) => meta.errors.push(format!("steamgriddb {slot}: {err}")),
                        }
                    }
                }
                Err(err) => meta.errors.push(format!("steamgriddb: {err}")),
            }
        }

        if let Some(name) = &e.art.libretro {
            let system = e.art.libretro_system.clone().or_else(|| e.platform.as_ref().map(|p| format!("Nintendo - {p}")));
            if let Some(system) = system {
                let file: String = name.chars().map(|c| if "&*/:`<>?\\|\"".contains(c) { '_' } else { c }).collect();
                for (slot, kind) in [("cover", "Named_Boxarts"), ("hero", "Named_Snaps")] {
                    if !wanted.contains(&slot) || meta.sources.contains_key(slot) {
                        continue;
                    }
                    let url = format!("{LIBRETRO}/{}/{kind}/{}.png", enc(&system), enc(&file));
                    match self.save(&url, &dir, slot) {
                        Ok(()) => {
                            meta.sources.insert(slot.to_string(), url);
                        }
                        Err(err) => meta.errors.push(format!("libretro {slot}: {err}")),
                    }
                }
            }
        }

        let _ = fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap_or_default());
        let got: Vec<&str> = meta.sources.keys().map(String::as_str).collect();
        let missing: Vec<&str> = SLOTS.iter().copied().filter(|s| image(root, &key, s).is_none()).collect();
        format!(
            "{}: downloaded [{}], missing [{}]{}",
            e.name,
            got.join(", "),
            missing.join(", "),
            if meta.errors.is_empty() { String::new() } else { format!("; {}", meta.errors.join("; ")) }
        )
    }

    fn sgdb_get(&self, api_key: &str, path: &str) -> Result<Value, String> {
        let resp = self
            .agent
            .get(&format!("{SGDB}{path}"))
            .set("Authorization", &format!("Bearer {api_key}"))
            .call()
            .map_err(|e| e.to_string())?;
        resp.into_json::<Value>().map_err(|e| e.to_string())
    }

    fn sgdb_id(&self, api_key: &str, e: &Entry) -> Result<u64, String> {
        let term = match &e.art.steamgriddb {
            Some(Value::Number(n)) => return n.as_u64().ok_or_else(|| "bad steamgriddb id".to_string()),
            Some(Value::String(s)) => s.clone(),
            _ => e.title().to_string(),
        };
        let v = self.sgdb_get(api_key, &format!("/search/autocomplete/{}", enc(&term)))?;
        v["data"][0]["id"].as_u64().ok_or_else(|| format!("no game matches \"{term}\""))
    }

    fn sgdb_asset(&self, api_key: &str, id: u64, slot: &str) -> Result<String, String> {
        let (kind, extra) = match slot {
            "cover" => ("grids", "&dimensions=600x900"),
            "hero" => ("heroes", ""),
            _ => ("logos", ""),
        };
        let v = self.sgdb_get(api_key, &format!("/{kind}/game/{id}?types=static&nsfw=false&humor=false&epilepsy=false{extra}"))?;
        v["data"][0]["url"].as_str().map(str::to_string).ok_or_else(|| "none available".to_string())
    }

    fn save(&self, url: &str, dir: &Path, slot: &str) -> Result<(), String> {
        let resp = self.agent.get(url).call().map_err(|e| e.to_string())?;
        let ext = match resp.content_type() {
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            "image/gif" => "gif",
            "image/png" => "png",
            other => return Err(format!("not an image ({other})")),
        };
        let mut bytes = Vec::new();
        resp.into_reader().take(MAX_BYTES).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        let tmp = dir.join(format!("{slot}.part"));
        fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
        for old in EXTS {
            let _ = fs::remove_file(dir.join(format!("{slot}.{old}")));
        }
        fs::rename(&tmp, dir.join(format!("{slot}.{ext}"))).map_err(|e| e.to_string())
    }
}

/// Percent-encodes one path segment.
fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
