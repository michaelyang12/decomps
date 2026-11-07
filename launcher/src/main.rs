//! Decomps Launcher: a library of the workspace's games (from shortcuts.json) with art, that launches them.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod art;
mod manifest;
mod scan;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::http::{Response, StatusCode};
use tauri::{AppHandle, Emitter, Manager, State};

const SGDB_KEY_VAR: &str = "STEAMGRIDDB_API_KEY";
/// A launch whose process exits sooner than this probably handed off to another process (a launcher
/// stub), so the library window stays minimized and no playtime is counted.
const HANDOFF: Duration = Duration::from_secs(15);

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    #[serde(default)]
    fullscreen: bool,
    #[serde(default)]
    games: HashMap<String, Played>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct Played {
    last_played: Option<u64>,
    playtime_secs: u64,
}

struct Shared {
    root: PathBuf,
    state_path: PathBuf,
    saved: Mutex<Saved>,
    running: Mutex<HashSet<String>>,
    fetching: AtomicBool,
    /// Games the last scan added, until the UI picks them up.
    added: Mutex<Vec<String>>,
}

impl Shared {
    fn save(&self) {
        if let Ok(s) = self.saved.lock() {
            if let Some(dir) = self.state_path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&self.state_path, serde_json::to_string_pretty(&*s).unwrap_or_default());
        }
    }
    fn sgdb_key(&self) -> Option<String> {
        manifest::env_var(&self.root, SGDB_KEY_VAR)
    }
}

type Ctx<'a> = State<'a, Arc<Shared>>;

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ---------- library ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Game {
    id: String,
    name: String,
    title: String,
    platform: Option<String>,
    port: Option<String>,
    art_key: String,
    /// slot -> cache-busting version (file mtime); absent slots have no image.
    art: BTreeMap<String, u64>,
    target: Option<String>,
    missing_target: String,
    last_played: Option<u64>,
    playtime_secs: u64,
    running: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Library {
    root: String,
    has_art_key: bool,
    fetching: bool,
    fullscreen: bool,
    /// Names added to shortcuts.json by a scan since the last call.
    added: Vec<String>,
    games: Vec<Game>,
}

#[tauri::command]
fn library(st: Ctx) -> Result<Library, String> {
    let entries = manifest::load(&st.root)?;
    let saved = st.saved.lock().map_err(|e| e.to_string())?;
    let running = st.running.lock().map_err(|e| e.to_string())?;
    let games = entries
        .iter()
        .map(|e| {
            let key = e.art_key();
            let art = art::SLOTS
                .iter()
                .filter_map(|s| {
                    let p = art::image(&st.root, &key, s)?;
                    let v = fs::metadata(&p).and_then(|m| m.modified()).ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
                    Some((s.to_string(), v))
                })
                .collect();
            let played = saved.games.get(&e.id()).cloned().unwrap_or_default();
            Game {
                id: e.id(),
                name: e.name.clone(),
                title: e.title().to_string(),
                platform: e.platform.clone(),
                port: e.port.clone(),
                art_key: key,
                art,
                target: manifest::resolve(&st.root, &e.target).map(|p| p.display().to_string()),
                missing_target: e.target.clone(),
                last_played: played.last_played,
                playtime_secs: played.playtime_secs,
                running: running.contains(&e.id()),
            }
        })
        .collect();
    Ok(Library {
        root: st.root.display().to_string(),
        has_art_key: st.sgdb_key().is_some(),
        fetching: st.fetching.load(Ordering::SeqCst),
        fullscreen: saved.fullscreen,
        added: st.added.lock().map(|mut a| std::mem::take(&mut *a)).unwrap_or_default(),
        games,
    })
}

fn find_entry(root: &Path, id: &str) -> Result<manifest::Entry, String> {
    manifest::load(root)?.into_iter().find(|e| e.id() == id).ok_or_else(|| format!("no game \"{id}\" in shortcuts.json"))
}

// ---------- launching ----------

#[tauri::command]
fn launch(app: AppHandle, st: Ctx, id: String) -> Result<(), String> {
    let e = find_entry(&st.root, &id)?;
    let target = manifest::resolve(&st.root, &e.target).ok_or_else(|| format!("{} is not built or has moved ({})", e.title(), e.target))?;
    let dir = target.parent().ok_or("target has no folder")?;
    // Games load data relative to their folder. A .bat runs through cmd.exe and gets its own console.
    let mut child = Command::new(&target)
        .current_dir(dir)
        .spawn()
        .map_err(|err| format!("could not start {}: {err}", target.display()))?;

    st.running.lock().map_err(|e| e.to_string())?.insert(id.clone());
    {
        let mut s = st.saved.lock().map_err(|e| e.to_string())?;
        s.games.entry(id.clone()).or_default().last_played = Some(now());
    }
    st.save();
    let _ = app.emit("library-changed", ());

    let window = app.get_webview_window("main");
    let minimized = match &window {
        Some(w) if w.is_fullscreen().unwrap_or(false) => w.minimize().is_ok(),
        _ => false,
    };

    let shared = st.inner().clone();
    std::thread::spawn(move || {
        let started = Instant::now();
        let _ = child.wait();
        let elapsed = started.elapsed();
        if let Ok(mut r) = shared.running.lock() {
            r.remove(&id);
        }
        if elapsed >= HANDOFF {
            if let Ok(mut s) = shared.saved.lock() {
                s.games.entry(id.clone()).or_default().playtime_secs += elapsed.as_secs();
            }
            shared.save();
            if let (true, Some(w)) = (minimized, window) {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }
        let _ = app.emit("library-changed", ());
    });
    Ok(())
}

#[tauri::command]
fn open_folder(st: Ctx, id: String) -> Result<(), String> {
    let e = find_entry(&st.root, &id)?;
    let mut cmd = Command::new("explorer.exe");
    match manifest::resolve(&st.root, &e.target) {
        Some(target) => {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.raw_arg(format!("/select,\"{}\"", target.display()));
            }
            #[cfg(not(windows))]
            cmd.arg(target.parent().unwrap_or(&st.root));
        }
        // Not built: open the deepest folder of the target path that exists.
        None => {
            let mut p = st.root.join(&e.target);
            while !p.is_dir() && p.pop() {}
            cmd.arg(p);
        }
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_art_folder(st: Ctx, id: String) -> Result<(), String> {
    let e = find_entry(&st.root, &id)?;
    let dir = art::custom_dir(&st.root).join(e.art_key());
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Command::new("explorer.exe").arg(&dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}

// ---------- window ----------

#[tauri::command]
fn set_fullscreen(window: tauri::WebviewWindow, st: Ctx, on: bool) -> Result<bool, String> {
    window.set_fullscreen(on).map_err(|e| e.to_string())?;
    st.saved.lock().map_err(|e| e.to_string())?.fullscreen = on;
    st.save();
    Ok(on)
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

// ---------- art ----------

/// Fetches art in the background. force: refetch every game; otherwise only games with gaps that are due a retry.
fn fetch_art(app: AppHandle, shared: Arc<Shared>, force: bool) {
    if shared.fetching.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = app.emit("library-changed", ());
    std::thread::spawn(move || {
        let key = shared.sgdb_key();
        let fetcher = art::Fetcher::new(key.clone());
        let mut log = Vec::new();
        let mut seen = HashSet::new();
        for e in manifest::load(&shared.root).unwrap_or_default() {
            if !seen.insert(e.art_key()) || !(force || art::needs_fetch(&shared.root, &e, key.is_some())) {
                continue;
            }
            log.push(fetcher.fetch(&shared.root, &e));
            let _ = app.emit("library-changed", ());
        }
        if !log.is_empty() {
            let _ = fs::write(art::cache_dir(&shared.root).join("fetch.log"), log.join("\n") + "\n");
        }
        shared.fetching.store(false, Ordering::SeqCst);
        let _ = app.emit("library-changed", ());
    });
}

/// Scans the library folders, tells the UI what was added, then fetches art (force: every game).
fn rescan(app: AppHandle, shared: Arc<Shared>, force_art: bool) {
    std::thread::spawn(move || {
        match scan::scan(&shared.root) {
            Ok(r) => {
                if let Ok(mut a) = shared.added.lock() {
                    a.extend(r.added);
                }
            }
            Err(err) => {
                let _ = app.emit("scan-failed", err);
            }
        }
        let _ = app.emit("library-changed", ());
        fetch_art(app, shared, force_art);
    });
}

/// The refresh button: rescan the library folders and refetch all art.
#[tauri::command]
fn refresh(app: AppHandle, st: Ctx) {
    if !st.fetching.load(Ordering::SeqCst) {
        rescan(app, st.inner().clone(), true);
    }
}

/// art://localhost/<art key>/<slot>  (http://art.localhost/... in WebView2)
fn serve_art(root: &Path, path: &str) -> Response<Vec<u8>> {
    let mut parts = path.trim_matches('/').split('/');
    let (key, slot) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let valid = !key.is_empty() && key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    let file = (valid && art::SLOTS.contains(&slot)).then(|| art::image(root, key, slot)).flatten();
    match file.and_then(|p| fs::read(&p).ok().map(|b| (p, b))) {
        Some((p, bytes)) => {
            let mime = match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
                "jpg" | "jpeg" => "image/jpeg",
                "webp" => "image/webp",
                "gif" => "image/gif",
                _ => "image/png",
            };
            Response::builder()
                .header("Content-Type", mime)
                .header("Cache-Control", "max-age=31536000, immutable")
                .body(bytes)
                .unwrap()
        }
        None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap(),
    }
}

// ---------- main ----------

fn main() {
    let root = manifest::find_root();
    let args: Vec<String> = std::env::args().collect();

    // Headless: decomps-launcher --fetch-art  (refetches all art, writes art/cache/fetch.log, exits)
    if args.iter().any(|a| a == "--fetch-art") {
        let fetcher = art::Fetcher::new(manifest::env_var(&root, SGDB_KEY_VAR));
        let mut seen = HashSet::new();
        let mut log = Vec::new();
        match manifest::load(&root) {
            Ok(entries) => {
                for e in entries.iter().filter(|e| seen.insert(e.art_key())) {
                    log.push(fetcher.fetch(&root, e));
                }
            }
            Err(err) => log.push(err),
        }
        let _ = fs::create_dir_all(art::cache_dir(&root));
        let _ = fs::write(art::cache_dir(&root).join("fetch.log"), log.join("\n") + "\n");
        return;
    }
    // Headless: decomps-launcher --scan  (adds unlisted game folders to shortcuts.json, writes art/cache/scan.log, exits)
    if args.iter().any(|a| a == "--scan") {
        let log = match scan::scan(&root) {
            Ok(r) => format!(
                "added: {}\nnothing launchable: {}\n",
                if r.added.is_empty() { "-".to_string() } else { r.added.join(", ") },
                if r.skipped.is_empty() { "-".to_string() } else { r.skipped.join(", ") }
            ),
            Err(err) => format!("error: {err}\n"),
        };
        let _ = fs::create_dir_all(art::cache_dir(&root));
        let _ = fs::write(art::cache_dir(&root).join("scan.log"), log);
        return;
    }
    let start_fullscreen = args.iter().any(|a| a == "--fullscreen");

    let art_root = root.clone();
    tauri::Builder::default()
        .register_uri_scheme_protocol("art", move |_ctx, req| serve_art(&art_root, req.uri().path()))
        .invoke_handler(tauri::generate_handler![library, launch, open_folder, open_art_folder, set_fullscreen, refresh, quit])
        .setup(move |app| {
            let state_path = app.path().app_local_data_dir()?.join("state.json");
            let saved: Saved = fs::read_to_string(&state_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            let fullscreen = start_fullscreen || saved.fullscreen;
            let shared = Arc::new(Shared {
                root,
                state_path,
                saved: Mutex::new(saved),
                running: Mutex::new(HashSet::new()),
                fetching: AtomicBool::new(false),
                added: Mutex::new(Vec::new()),
            });
            shared.saved.lock().unwrap().fullscreen = fullscreen;
            app.manage(shared.clone());
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_fullscreen(fullscreen);
                let _ = w.show();
                let _ = w.set_focus();
            }
            rescan(app.handle().clone(), shared, false);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the launcher");
}
