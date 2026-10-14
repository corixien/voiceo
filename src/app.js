const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];

const MODES = [
  ["raw", "Roh", "Nur Whisper, keine Nachbearbeitung."],
  ["clean", "Sauber", "Satzzeichen, Füllwörter weg, textnah."],
  ["message", "Nachricht", "Locker und kurz, wie im Chat."],
  ["email", "E-Mail", "Höflich und strukturiert."],
  ["prompt", "Prompt", "Klare Anweisung für KI oder Entwickler."],
  ["notes", "Notizen", "Stichpunkte."],
  ["custom", "Eigener", "Dein eigener Prompt."],
];

let S = {};
let saveTimer = null;

function toast(msg) {
  const t = $("#toast");
  t.textContent = msg;
  t.classList.add("on");
  clearTimeout(toast.t);
  toast.t = setTimeout(() => t.classList.remove("on"), 1400);
}

function queueSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    try {
      await invoke("save_settings", { settings: S });
      toast("Gespeichert");
      refreshStatus();
    } catch (e) { toast("Fehler: " + e); }
  }, 350);
}

// ---------- binding ----------
function bind() {
  $$("[data-k]").forEach((el) => {
    const k = el.dataset.k;
    if (el.type === "checkbox") el.checked = !!S[k];
    else el.value = S[k] ?? "";
    el.addEventListener("input", () => {
      if (el.type === "checkbox") S[k] = el.checked;
      else if (el.type === "number") S[k] = parseFloat(el.value) || 0;
      else S[k] = el.value;
      if (k === "mode" || k === "cleanup") renderModes();
      queueSave();
    });
  });
  $$("[data-seg]").forEach((seg) => {
    const k = seg.dataset.seg;
    const paint = () => $$("button", seg).forEach((b) => b.classList.toggle("on", b.dataset.v === S[k]));
    paint();
    $$("button", seg).forEach((b) => b.addEventListener("click", () => { S[k] = b.dataset.v; paint(); queueSave(); }));
  });
  const dict = $("[data-dict]");
  dict.value = (S.dictionary || []).join("\n");
  dict.addEventListener("input", () => {
    S.dictionary = dict.value.split("\n").map((x) => x.trim()).filter(Boolean);
    queueSave();
  });
}

// ---------- modes ----------
function renderModes() {
  const box = $("#modes");
  box.innerHTML = "";
  MODES.forEach(([id, name, desc]) => {
    const b = document.createElement("button");
    b.className = "mode" + (S.mode === id ? " on" : "");
    b.innerHTML = `<b>${name}</b><small>${desc}</small>`;
    b.onclick = () => { S.mode = id; renderModes(); queueSave(); };
    box.appendChild(b);
  });
  $("#custom-wrap").style.display = S.mode === "custom" ? "" : "none";
  box.style.opacity = S.cleanup ? 1 : 0.45;
}

// ---------- hotkey ----------
async function renderHotkey() {
  for (const [key, id] of [["hotkey", "#hk-keys"], ["learn_hotkey", "#lk-keys"]]) {
    const labels = await invoke("key_labels", { codes: S[key] || [] });
    $(id).innerHTML = labels.map((l) => `<kbd>${l}</kbd>`).join("") || "<kbd>aus</kbd>";
  }
}
function wireCapture(btn, box, keys, field) {
  $(btn).onclick = async () => {
    $(box).classList.add("capturing");
    $(keys).innerHTML = "<kbd>Taste drücken…</kbd>";
    $(btn).disabled = true;
    try {
      const codes = await invoke("capture_hotkey");
      if (codes.length) { S[field] = codes; queueSave(); }
    } catch (e) { toast(String(e)); }
    $(box).classList.remove("capturing");
    $(btn).disabled = false;
    renderHotkey();
  };
}
wireCapture("#hk-change", "#hk-box", "#hk-keys", "hotkey");
wireCapture("#lk-change", "#lk-box", "#lk-keys", "learn_hotkey");
$("#lk-clear").onclick = () => { S.learn_hotkey = []; queueSave(); renderHotkey(); };

// ---------- learned corrections ----------
function renderLearned(items) {
  const box = $("#learned");
  if (!items.length) { box.innerHTML = '<div class="empty">Noch nichts gelernt.</div>'; return; }
  box.innerHTML = "";
  items.forEach((it) => {
    const r = document.createElement("div");
    r.className = "row";
    r.innerHTML = `<div class="l"><b></b><small>${it.count}× gelernt</small></div><button class="btn small">Entfernen</button>`;
    $("b", r).textContent = `${it.from}  →  ${it.to}`;
    $("button", r).onclick = async () => renderLearned(await invoke("delete_learned", { from: it.from }));
    box.appendChild(r);
  });
}
$("#l-add").onclick = async () => {
  const from = $("#l-from").value.trim(), to = $("#l-to").value.trim();
  if (!from || !to) return;
  renderLearned(await invoke("add_learned", { from, to }));
  $("#l-from").value = ""; $("#l-to").value = "";
};

// ---------- api test ----------
$("#test-key").onclick = async () => {
  const row = $("#key-msg-row"), msg = $("#key-msg");
  row.style.display = ""; msg.className = "hint"; msg.textContent = "Teste…";
  try {
    msg.textContent = await invoke("test_api", { key: S.api_key || "" });
    msg.className = "msg-ok";
  } catch (e) { msg.textContent = String(e); msg.className = "msg-bad"; }
};

// ---------- status ----------
async function refreshStatus() {
  const s = await invoke("system_status");
  const checks = [
    [s.api_key, "Groq API-Key", s.api_key ? "Gesetzt" : "Fehlt. Oben eintragen."],
    [s.keyboards > 0, "Globaler Hotkey", s.keyboards > 0
      ? `${s.keyboards} Tastatur(en) erkannt`
      : "Kein Zugriff auf die Tastatur. Einmalige Einrichtung nötig."],
    [s.wl_copy, "Zwischenablage (wl-copy)", s.wl_copy ? "Bereit" : "Paket wl-clipboard installieren."],
    [s.uinput, "Einfügen (uinput)", s.uinput ? "Bereit" : "Kein Zugriff auf /dev/uinput. Einmalige Einrichtung nötig."],
  ];
  $("#checks").innerHTML = checks.map(([ok, t, d], i) => `
    <div class="check"><span class="dot ${ok ? "ok" : "bad"}"></span>
      <div><b>${t}</b><small>${d}</small></div></div>`).join("")
    + ((s.keyboards > 0 && s.uinput) ? "" : '<div class="check"><button class="btn primary" id="run-setup">Jetzt einrichten</button><small>Fragt einmal nach deinem Passwort. Kein Neustart nötig.</small></div>');
  const rs = $("#run-setup");
  if (rs) rs.onclick = async () => {
    rs.disabled = true; rs.textContent = "Richte ein…";
    try { const ok = await invoke("run_setup"); toast(ok ? "Eingerichtet" : "Noch nicht bereit, bitte erneut versuchen"); }
    catch (e) { toast(String(e)); }
    refreshStatus(); renderHotkey();
  };
  const allOk = checks.every((c) => c[0]);
  $("#mini-dot").className = "dot " + (allOk ? "ok" : "bad");
  $("#mini-text").textContent = allOk ? "Bereit" : "Setup unvollständig";
}

// ---------- history ----------
async function renderHistory() {
  const items = await invoke("get_history");
  const list = $("#hist-list");
  if (!items.length) { list.innerHTML = '<div class="empty">Noch keine Diktate.</div>'; return; }
  list.innerHTML = "";
  items.forEach((it) => {
    const d = document.createElement("div");
    d.className = "hist";
    const time = new Date(it.ts * 1000).toLocaleString("de-DE", { dateStyle: "short", timeStyle: "short" });
    const esc = (x) => x.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));
    d.innerHTML = `<div class="meta"><span>${time} · ${it.mode}</span><span>${(it.ms / 1000).toFixed(1)} s</span></div>
      <p>${esc(it.text)}</p>${it.raw !== it.text ? `<p class="raw">Roh: ${esc(it.raw)}</p>` : ""}
      <div class="acts"><button class="btn small">Kopieren</button></div>`;
    $("button", d).onclick = () => { navigator.clipboard.writeText(it.text); toast("Kopiert"); };
    list.appendChild(d);
  });
}
$("#hist-clear").onclick = async () => { await invoke("clear_history"); renderHistory(); };

// ---------- tabs ----------
$$("#nav button").forEach((b) => b.addEventListener("click", () => {
  $$("#nav button").forEach((x) => x.classList.toggle("active", x === b));
  $$("main section").forEach((s) => s.classList.toggle("active", s.id === "tab-" + b.dataset.tab));
  if (b.dataset.tab === "history") renderHistory();
  if (b.dataset.tab === "general") refreshStatus();
}));

// ---------- init ----------
(async () => {
  S = await invoke("get_settings");
  bind(); renderModes(); renderHotkey(); refreshStatus();
  listen("voiceo://history", renderHistory);
  listen("voiceo://learned", async () => renderLearned(await invoke("get_learned")));
  renderLearned(await invoke("get_learned"));
  listen("voiceo://setup", () => { refreshStatus(); renderHotkey(); });
  const logo = $("#logo");
  const loop = (t) => { drawMascot(logo, "idle", t, 0); requestAnimationFrame(loop); };
  requestAnimationFrame(loop);
})();
