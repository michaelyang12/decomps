"use strict";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const shelf = $("shelf");

let lib = null;          // last library() result
let sel = 0;             // selected index
let cards = new Map();   // game id -> card element
let bgLayer = 0;         // which backdrop layer is showing
let bgUrl = null;
let starting = null;     // id of a game whose launch is in flight

// WebView2 maps custom schemes to http://<scheme>.localhost/
const artUrl = (g, slot) => `http://art.localhost/${g.artKey}/${slot}?v=${g.art[slot]}`;

function store(key, value) {
  try {
    if (value === undefined) return localStorage.getItem(key);
    localStorage.setItem(key, value);
  } catch { return null; }
}

// ---------- formatting ----------

function ago(secs) {
  const d = Date.now() / 1000 - secs;
  if (d < 90) return "just now";
  if (d < 3600) return `${Math.round(d / 60)} min ago`;
  if (d < 86400) return `${Math.round(d / 3600)} h ago`;
  if (d < 86400 * 2) return "yesterday";
  if (d < 86400 * 30) return `${Math.round(d / 86400)} days ago`;
  return new Date(secs * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

function playtime(secs) {
  if (secs < 60) return null;
  const h = Math.floor(secs / 3600), m = Math.round((secs % 3600) / 60);
  return h ? `${h} h ${m} min played` : `${m} min played`;
}

function hue(s) {
  let h = 0;
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return h % 360;
}

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
}

// ---------- rendering ----------

// Tiles of the same game share art; every one after the first is labelled with the last part of its port,
// e.g. "Static recomp, modded" -> "Modded".
function variantLabel(g) {
  const first = lib.games.find((x) => x.artKey === g.artKey);
  if (first === g || !g.port) return "";
  const s = g.port.split(",").pop().trim();
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function makeCard(g) {
  const c = el("button", "card");
  c.setAttribute("role", "option");
  c.tabIndex = -1;
  c.append(el("div", "blur"), el("div", "ph"), el("img"), el("span", "badge"));
  const img = c.querySelector("img");
  img.alt = "";
  img.decoding = "async";
  img.addEventListener("load", () => {
    // Box scans (e.g. landscape N64 boxes) don't fit a 2:3 tile: letterbox them over a blurred copy.
    const r = img.naturalWidth / img.naturalHeight;
    c.classList.toggle("fit", Math.abs(r - 2 / 3) > 0.07);
  });
  c.addEventListener("click", () => select(lib.games.findIndex((x) => x.id === c.dataset.id)));
  c.addEventListener("dblclick", () => play());
  return c;
}

function updateCard(c, g) {
  c.dataset.id = g.id;
  c.setAttribute("aria-label", `${g.title}${g.port ? `, ${g.port}` : ""}`);
  c.classList.toggle("missing", !g.target);

  const img = c.querySelector("img");
  const ph = c.querySelector(".ph");
  if (g.art.cover) {
    const src = artUrl(g, "cover");
    if (img.getAttribute("src") !== src) {
      img.src = src;
      c.querySelector(".blur").style.backgroundImage = `url("${src}")`;
    }
    img.hidden = false;
    ph.hidden = true;
  } else {
    img.removeAttribute("src");
    img.hidden = true;
    c.classList.remove("fit");
    ph.hidden = false;
    const h = hue(g.title);
    ph.style.background = `linear-gradient(160deg, hsl(${h} 45% 34%), hsl(${(h + 40) % 360} 50% 14%))`;
    ph.replaceChildren(el("span", "ph-plat", g.platform || ""), el("span", "ph-title", g.title));
  }

  const badge = c.querySelector(".badge");
  badge.className = "badge";
  if (g.running) { badge.textContent = "Running"; badge.classList.add("live"); }
  else if (!g.target) badge.textContent = "Not built";
  else badge.textContent = variantLabel(g);
}

function renderShelf() {
  const seen = new Set();
  lib.games.forEach((g, i) => {
    let c = cards.get(g.id);
    if (!c) { c = makeCard(g); cards.set(g.id, c); }
    updateCard(c, g);
    if (shelf.children[i] !== c) shelf.insertBefore(c, shelf.children[i] || null);
    seen.add(g.id);
  });
  for (const [id, c] of cards) if (!seen.has(id)) { c.remove(); cards.delete(id); }
}

function setBackdrop(url) {
  if (url === bgUrl) return;
  bgUrl = url;
  const layers = [$("bgA"), $("bgB")];
  const next = layers[1 - bgLayer];
  const show = (soft) => {
    next.style.backgroundImage = url ? `url("${url}")` : "none";
    next.classList.toggle("soft", soft);
    next.classList.add("on");
    layers[bgLayer].classList.remove("on");
    bgLayer = 1 - bgLayer;
  };
  if (!url) return show(false);
  const probe = new Image();
  probe.onload = () => {
    // Low-res heroes (libretro snaps, or a cover standing in) look better blurred.
    if (bgUrl === url) show(probe.naturalWidth < 1200 || probe.naturalHeight > probe.naturalWidth);
  };
  probe.onerror = () => { if (bgUrl === url) show(false); };
  probe.src = url;
}

function renderDetail() {
  const g = lib.games[sel];
  const detail = $("detail");
  if (!g) {
    detail.hidden = true;
    setBackdrop(null);
    return;
  }
  detail.hidden = false;

  $("title").textContent = g.title;
  const logo = $("logo");
  if (g.art.logo) {
    const src = artUrl(g, "logo");
    if (logo.getAttribute("src") !== src) logo.src = src;
    detail.classList.add("has-logo");
  } else {
    logo.removeAttribute("src");
    detail.classList.remove("has-logo");
  }

  const meta = $("meta");
  meta.replaceChildren();
  if (g.platform) meta.append(el("span", "chip", g.platform));
  if (g.port) meta.append(el("span", null, g.port));
  if (g.lastPlayed) meta.append(el("span", null, `Played ${ago(g.lastPlayed)}`));
  const pt = playtime(g.playtimeSecs);
  if (pt) meta.append(el("span", null, pt));

  const btn = $("btnPlay");
  btn.disabled = !g.target;
  btn.classList.toggle("running", g.running);
  $("playLabel").textContent = !g.target ? "Not built" : starting === g.id ? "Starting…" : g.running ? "Running" : "Play";

  const note = $("note");
  note.replaceChildren();
  const code = (t) => el("code", null, t);
  if (!g.target) {
    note.append("Launch target not found: ", code(g.missingTarget), ". Build the game, or fix its target in ", code("shortcuts.json"), ".");
  } else if (!g.art.cover && !lib.fetching) {
    note.append("No art found. ");
    if (!lib.hasArtKey) note.append("Add ", code("STEAMGRIDDB_API_KEY"), " to ", code(".env"), " in the Decomps folder, or use ");
    else note.append("Use ");
    note.append(el("b", null, "Custom art"), " to add a cover.png.");
  }

  setBackdrop(g.art.hero ? artUrl(g, "hero") : g.art.cover ? artUrl(g, "cover") : null);
}

function renderChrome() {
  document.body.classList.toggle("fs", lib.fullscreen);
  $("count").textContent = `${lib.games.length} game${lib.games.length === 1 ? "" : "s"}`;
  const status = $("status");
  status.textContent = lib.fetching ? "Fetching art" : "";
  status.classList.toggle("busy", lib.fetching);
  $("btnArt").disabled = lib.fetching;
}

function select(i, smooth = true) {
  if (!lib || !lib.games.length) return;
  sel = Math.max(0, Math.min(lib.games.length - 1, i));
  const g = lib.games[sel];
  store("selected", g.id);
  for (const c of cards.values()) {
    const on = c.dataset.id === g.id;
    c.classList.toggle("sel", on);
    c.setAttribute("aria-selected", on);
  }
  cards.get(g.id)?.scrollIntoView({ behavior: smooth ? "smooth" : "auto", inline: "nearest", block: "nearest" });
  renderDetail();
}

async function refresh() {
  let next;
  try {
    next = await invoke("library");
  } catch (err) {
    shelf.replaceChildren(el("p", "empty", `Could not read the game list: ${err}`));
    return;
  }
  const keepId = lib ? lib.games[sel]?.id : store("selected");
  const first = !lib;
  lib = next;
  renderChrome();
  if (!lib.games.length) {
    shelf.replaceChildren(el("p", "empty", `No games in ${lib.root}\\shortcuts.json.`));
    $("detail").hidden = true;
    return;
  }
  renderShelf();
  const i = lib.games.findIndex((g) => g.id === keepId);
  select(i < 0 ? 0 : i, !first);
}

// ---------- actions ----------

let toastTimer;
function toast(msg) {
  const t = $("toast");
  t.textContent = msg;
  t.classList.add("on");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove("on"), 5000);
}

async function play() {
  const g = lib?.games[sel];
  if (!g || !g.target || starting) return;
  starting = g.id;
  renderDetail();
  try {
    await invoke("launch", { id: g.id });
  } catch (err) {
    toast(String(err));
  } finally {
    // Keep "Starting…" briefly so a double press doesn't launch twice.
    setTimeout(() => { starting = null; if (lib) renderDetail(); }, 2500);
  }
}

function openFolder() {
  const g = lib?.games[sel];
  if (g) invoke("open_folder", { id: g.id }).catch((e) => toast(String(e)));
}

function openArtFolder() {
  const g = lib?.games[sel];
  if (g) invoke("open_art_folder", { id: g.id }).catch((e) => toast(String(e)));
}

function refreshArt() {
  if (lib && !lib.fetching) invoke("refresh_art").catch((e) => toast(String(e)));
}

async function toggleFullscreen(on = !lib?.fullscreen) {
  try {
    lib.fullscreen = await invoke("set_fullscreen", { on });
    renderChrome();
  } catch (e) { toast(String(e)); }
}

$("btnPlay").addEventListener("click", play);
$("btnFolder").addEventListener("click", openFolder);
$("btnArtFolder").addEventListener("click", openArtFolder);
$("btnArt").addEventListener("click", refreshArt);
$("btnFull").addEventListener("click", () => toggleFullscreen());

// ---------- keyboard / mouse ----------

document.addEventListener("keydown", (e) => {
  const onButton = document.activeElement?.tagName === "BUTTON" && !document.activeElement.classList.contains("card");
  switch (e.key) {
    case "ArrowLeft": select(sel - 1); break;
    case "ArrowRight": select(sel + 1); break;
    case "Home": select(0); break;
    case "End": select(Infinity); break;
    case "Enter": case " ": if (onButton) return; play(); break;
    case "F11": toggleFullscreen(); break;
    case "F5": refreshArt(); break;
    case "Escape": if (lib?.fullscreen) toggleFullscreen(false); break;
    default:
      if ((e.ctrlKey && (e.key === "r" || e.key === "R")) || e.key === "F7") break; // no reload / caret browsing
      return;
  }
  e.preventDefault();
});

document.addEventListener("contextmenu", (e) => e.preventDefault());

shelf.addEventListener("wheel", (e) => {
  if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
    shelf.scrollLeft += e.deltaY;
    e.preventDefault();
  }
}, { passive: false });

// ---------- gamepad (standard mapping) ----------

const pad = { prev: new Map(), dir: 0, next: 0 };
const BTN = { A: 0, X: 2, Y: 3, START: 9, LEFT: 14, RIGHT: 15, LB: 4, RB: 5 };

function pollPads(t) {
  const pads = navigator.getGamepads ? [...navigator.getGamepads()].filter(Boolean) : [];
  document.body.classList.toggle("pad", pads.length > 0);
  // Only act while the launcher has focus: presses inside a running game must not reach it.
  const active = document.hasFocus() && lib;
  let dir = 0;
  for (const gp of pads) {
    const down = (i) => !!gp.buttons[i]?.pressed;
    const fresh = (i) => down(i) && !pad.prev.get(`${gp.index}:${i}`);
    if (active) {
      if (fresh(BTN.A)) play();
      if (fresh(BTN.X)) openFolder();
      if (fresh(BTN.Y)) refreshArt();
      if (fresh(BTN.START)) toggleFullscreen();
      if (fresh(BTN.LB)) select(0);
      if (fresh(BTN.RB)) select(Infinity);
    }
    const x = gp.axes[0] || 0;
    if (down(BTN.LEFT) || x < -0.5) dir = -1;
    else if (down(BTN.RIGHT) || x > 0.5) dir = 1;
    gp.buttons.forEach((b, i) => pad.prev.set(`${gp.index}:${i}`, b.pressed));
  }
  if (active && dir) {
    if (dir !== pad.dir) { select(sel + dir); pad.next = t + 380; }
    else if (t >= pad.next) { select(sel + dir); pad.next = t + 110; }
  }
  pad.dir = active ? dir : 0;
  requestAnimationFrame(pollPads);
}
requestAnimationFrame(pollPads);

// ---------- start ----------

listen("library-changed", () => refresh());
refresh();
