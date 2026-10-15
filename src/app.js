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
const PRESETS = [[97], [100], [127], [119], [70]];
const RISKY = (c) => c.includes(125) || c.includes(126) || (c.includes(56) && c.includes(57)) || (c.includes(29) && c.includes(56) && c.length > 2);

let S = {};
let saveTimer = null;
const refreshers = [];            // custom controls re-sync after settings load

// ---------- tiny helpers ----------
function toast(msg) {
  const t = $("#toast");
  t.textContent = msg; t.classList.add("on");
  clearTimeout(toast.t); toast.t = setTimeout(() => t.classList.remove("on"), 1500);
}
function queueSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    try { await invoke("save_settings", { settings: S }); toast("Gespeichert"); mascot.react("done"); refreshStatus(); }
    catch (e) { toast("Fehler: " + e); }
  }, 350);
}
document.addEventListener("pointerdown", (e) => {
  const b = e.target.closest(".btn");
  if (!b) return;
  const r = b.getBoundingClientRect(), d = Math.max(r.width, r.height);
  const s = document.createElement("span");
  s.className = "ripple"; s.style.cssText = `width:${d}px;height:${d}px;left:${e.clientX - r.left - d / 2}px;top:${e.clientY - r.top - d / 2}px`;
  b.appendChild(s); setTimeout(() => s.remove(), 650);
});

// ---------- theme ----------
function applyTheme() {
  const t = S.theme || "system";
  const light = t === "light" || (t === "system" && matchMedia("(prefers-color-scheme: light)").matches);
  document.documentElement.dataset.theme = light ? "light" : "dark";
}
matchMedia("(prefers-color-scheme: light)").addEventListener?.("change", applyTheme);

// ---------- sidebar mascot ----------
const mascot = (() => {
  const cv = $("#logo"), ctx = cv.getContext("2d");
  const dpr = 2; ctx.scale(dpr, dpr);
  let state = "idle", until = 0, bounce = 0, look = [0, 0];
  addEventListener("pointermove", (e) => {
    const r = cv.getBoundingClientRect();
    const dx = e.clientX - (r.left + r.width / 2), dy = e.clientY - (r.top + r.height / 2);
    const d = Math.hypot(dx, dy) || 1, k = Math.min(1, d / 240);
    look = [(dx / d) * k, (dy / d) * k * 0.7];
  });
  $("#brand").addEventListener("click", () => api.react("done", 1100, true));
  const api = {
    react(s, ms = 900, hop = false) { state = s; until = performance.now() + ms; if (hop) bounce = 1; },
  };
  (function loop(t) {
    if (state !== "idle" && t > until) state = "idle";
    bounce *= 0.9;
    ctx.clearRect(0, 0, 62, 62);
    window.Voi.draw(ctx, { x: 31, y: 36 - Math.sin(bounce * 7) * bounce * 7, s: 0.62, state, t, level: 0, look });
    requestAnimationFrame(loop);
  })(0);
  return api;
})();

// ---------- custom controls ----------
function enhanceSelect(native) {
  const wrap = document.createElement("div");
  wrap.className = "sel";
  const btn = document.createElement("button");
  btn.className = "sel-btn"; btn.type = "button";
  const menu = document.createElement("div");
  menu.className = "menu";
  wrap.append(btn, menu);
  native.style.display = "none";
  native.after(wrap);

  const build = () => {
    menu.innerHTML = "";
    [...native.options].forEach((o) => {
      const d = document.createElement("div");
      d.textContent = o.textContent; d.dataset.v = o.value;
      d.onclick = () => { native.value = o.value; native.dispatchEvent(new Event("input")); sync(); wrap.classList.remove("open"); };
      menu.appendChild(d);
    });
    sync();
  };
  const sync = () => {
    const o = [...native.options].find((x) => x.value === native.value) || native.options[0];
    btn.textContent = o ? o.textContent : "";
    $$("div", menu).forEach((d) => d.classList.toggle("on", d.dataset.v === native.value));
  };
  btn.onclick = (e) => { e.stopPropagation(); $$(".sel.open").forEach((x) => x !== wrap && x.classList.remove("open")); wrap.classList.toggle("open"); };
  native.rebuild = build;
  build();
  refreshers.push(sync);
}
function enhanceCombo(input) {
  const opts = (input.dataset.opts || "").split(",").filter(Boolean);
  const wrap = document.createElement("div");
  wrap.className = "sel combo";
  input.before(wrap); wrap.appendChild(input);
  const menu = document.createElement("div"); menu.className = "menu"; wrap.appendChild(menu);
  opts.forEach((v) => {
    const d = document.createElement("div"); d.textContent = v;
    d.onclick = () => { input.value = v; input.dispatchEvent(new Event("input")); wrap.classList.remove("open"); mark(); };
    menu.appendChild(d);
  });
  const mark = () => $$("div", menu).forEach((d) => d.classList.toggle("on", d.textContent === input.value));
  input.addEventListener("focus", () => { mark(); wrap.classList.add("open"); });
  input.addEventListener("input", mark);
  wrap.addEventListener("click", (e) => e.stopPropagation());
  wrap.addEventListener("mousedown", (e) => { if (e.target === wrap) { wrap.classList.toggle("open"); } });
  refreshers.push(mark);
}
function enhanceNumber(input) {
  const wrap = document.createElement("div");
  wrap.className = "step";
  const mk = (label, dir) => {
    const b = document.createElement("button"); b.type = "button"; b.textContent = label;
    let hold;
    const step = () => { dir > 0 ? input.stepUp() : input.stepDown(); input.dispatchEvent(new Event("input")); };
    b.onpointerdown = () => { step(); hold = setTimeout(function rep() { step(); hold = setTimeout(rep, 70); }, 420); };
    ["pointerup", "pointerleave"].forEach((ev) => b.addEventListener(ev, () => clearTimeout(hold)));
    return b;
  };
  input.before(wrap);
  wrap.append(mk("−", -1), input, mk("+", 1));
  input.style.width = "";
}
document.addEventListener("click", () => $$(".sel.open").forEach((x) => x.classList.remove("open")));
addEventListener("keydown", (e) => { if (e.key === "Escape") $$(".sel.open").forEach((x) => x.classList.remove("open")); });

function sliderFor(seg) {
  const thumb = document.createElement("div"); thumb.className = "thumb"; seg.prepend(thumb);
  const place = () => {
    const on = $("button.on", seg);
    if (!on) return;
    thumb.style.width = on.offsetWidth + "px"; thumb.style.transform = `translateX(${on.offsetLeft - 3}px)`;
  };
  seg.place = place; return place;
}

// ---------- binding ----------
function bind() {
  $$("select[data-k]").forEach(enhanceSelect);
  $$("input[data-opts]").forEach(enhanceCombo);
  $$("input[type=number]").forEach(enhanceNumber);

  $$("[data-k]").forEach((el) => {
    const k = el.dataset.k;
    if (el.type === "checkbox") el.checked = !!S[k]; else el.value = S[k] ?? "";
    el.addEventListener("input", () => {
      if (el.type === "checkbox") S[k] = el.checked;
      else if (el.type === "number") S[k] = parseFloat(el.value) || 0;
      else S[k] = el.value;
      if (k === "mode" || k === "cleanup") renderModes();
      if (k === "theme") applyTheme();
      queueSave();
    });
  });
  refreshers.forEach((f) => f());

  $$("[data-seg]").forEach((seg) => {
    const k = seg.dataset.seg;
    const place = sliderFor(seg);
    const paint = () => { $$("button", seg).forEach((b) => b.classList.toggle("on", b.dataset.v === S[k])); place(); };
    paint();
    $$("button", seg).forEach((b) => b.addEventListener("click", () => { S[k] = b.dataset.v; paint(); queueSave(); }));
    seg.paint = paint;
  });
  const dict = $("[data-dict]");
  dict.value = (S.dictionary || []).join("\n");
  dict.addEventListener("input", () => { S.dictionary = dict.value.split("\n").map((x) => x.trim()).filter(Boolean); queueSave(); });
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
  box.style.opacity = S.cleanup ? 1 : 0.4;
  box.style.transition = "opacity .25s";
}

// ---------- hotkeys ----------
async function renderPresets() {
  const box = $("#hk-presets");
  box.innerHTML = "";
  for (const codes of PRESETS) {
    const [label] = await invoke("key_labels", { codes });
    const b = document.createElement("button");
    b.textContent = label;
    b.className = JSON.stringify(S.hotkey) === JSON.stringify(codes) ? "on" : "";
    b.onclick = () => { S.hotkey = codes; queueSave(); renderHotkey(); };
    box.appendChild(b);
  }
  if (RISKY(S.hotkey || [])) {
    const w = document.createElement("div");
    w.className = "warn";
    w.textContent = "Diese Kombination kollidiert wahrscheinlich mit anderen Programmen. Wähle eine einzelne Taste.";
    box.appendChild(w);
  }
}
async function renderHotkey() {
  renderPresets();
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

// ---------- learned ----------
function renderLearned(items) {
  const box = $("#learned");
  if (!items.length) { box.innerHTML = '<div class="empty">Noch nichts gelernt. Korrigiere ein Wort nach einem Diktat.</div>'; return; }
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
  $("#l-from").value = ""; $("#l-to").value = ""; mascot.react("learned", 1400, true);
};

// ---------- api test ----------
$("#test-key").onclick = async () => {
  const row = $("#key-msg-row"), msg = $("#key-msg");
  row.style.display = ""; msg.className = "hint"; msg.textContent = "Teste…";
  try { msg.textContent = await invoke("test_api", { key: S.api_key || "" }); msg.className = "msg-ok"; mascot.react("done", 1400, true); }
  catch (e) { msg.textContent = String(e); msg.className = "msg-bad"; mascot.react("error", 1600); }
};

// ---------- status ----------
async function refreshStatus() {
  const s = await invoke("system_status");
  const checks = [
    [s.api_key, "Groq API-Key", s.api_key ? "Gesetzt" : "Fehlt. Oben eintragen."],
    [s.keyboards > 0, "Globaler Hotkey", s.keyboards > 0 ? `${s.keyboards} Tastatur(en) erkannt` : "Kein Zugriff auf die Tastatur. Einmalige Einrichtung nötig."],
    [s.wl_copy, "Zwischenablage (wl-copy)", s.wl_copy ? "Bereit" : "Paket wl-clipboard installieren."],
    [s.uinput, "Einfügen (uinput)", s.uinput ? "Bereit" : "Kein Zugriff auf /dev/uinput. Einmalige Einrichtung nötig."],
    [s.atspi, "Automatisches Lernen", s.atspi ? "Bereit (AT-SPI)" : "Nicht aktiv. python3-gobject und at-spi2-core nötig, oder Lern-Taste nutzen."],
  ];
  $("#checks").innerHTML = checks.map(([ok, t, d]) => `
    <div class="check"><span class="dot ${ok ? "ok" : "bad"}"></span><div><b>${t}</b><small>${d}</small></div></div>`).join("")
    + ((s.keyboards > 0 && s.uinput) ? "" : '<div class="check"><button class="btn primary" id="run-setup">Jetzt einrichten</button><small>Fragt einmal nach deinem Passwort. Kein Neustart nötig.</small></div>');
  const rs = $("#run-setup");
  if (rs) rs.onclick = async () => {
    rs.disabled = true; rs.textContent = "Richte ein…";
    try { const ok = await invoke("run_setup"); toast(ok ? "Eingerichtet" : "Noch nicht bereit, bitte erneut versuchen"); }
    catch (e) { toast(String(e)); }
    refreshStatus(); renderHotkey();
  };
  const core = checks.slice(0, 4).every((c) => c[0]);
  $("#mini-dot").className = "dot " + (core ? "ok" : "bad");
  $("#mini-text").textContent = core ? "Bereit" : "Setup unvollständig";
}

// ---------- history ----------
async function renderHistory() {
  const items = await invoke("get_history");
  const list = $("#hist-list");
  if (!items.length) { list.innerHTML = '<div class="empty">Noch keine Diktate.</div>'; return; }
  list.innerHTML = "";
  const esc = (x) => x.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));
  items.forEach((it) => {
    const d = document.createElement("div");
    d.className = "hist";
    const time = new Date(it.ts * 1000).toLocaleString("de-DE", { dateStyle: "short", timeStyle: "short" });
    d.innerHTML = `<div class="meta"><span>${time} · ${it.mode}</span><span>${(it.ms / 1000).toFixed(1)} s</span></div>
      <p>${esc(it.text)}</p>${it.raw !== it.text ? `<p class="raw">Roh: ${esc(it.raw)}</p>` : ""}
      <div class="acts"><button class="btn small">Kopieren</button></div>`;
    $("button", d).onclick = () => { navigator.clipboard.writeText(it.text); toast("Kopiert"); };
    list.appendChild(d);
  });
}
$("#hist-clear").onclick = async () => { await invoke("clear_history"); renderHistory(); };

// ---------- tabs ----------
function moveIndicator() {
  const b = $("#nav button.active"), ind = $("#nav-ind");
  if (!b) return;
  ind.style.transform = `translateY(${b.offsetTop}px)`; ind.style.height = b.offsetHeight + "px";
}
$$("#nav button").forEach((b) => b.addEventListener("click", () => {
  $$("#nav button").forEach((x) => x.classList.toggle("active", x === b));
  $$("main section").forEach((s) => s.classList.toggle("active", s.id === "tab-" + b.dataset.tab));
  moveIndicator();
  $("main").scrollTo({ top: 0 });
  requestAnimationFrame(() => $$("[data-seg]").forEach((s) => s.place && s.place()));
  if (b.dataset.tab === "history") renderHistory();
  if (b.dataset.tab === "general") refreshStatus();
}));
addEventListener("resize", moveIndicator);

// ---------- init ----------
(async () => {
  S = await invoke("get_settings");
  applyTheme();
  try {
    const devices = await invoke("list_inputs");
    const sel = $("#mic-select");
    devices.forEach((n) => { const o = document.createElement("option"); o.value = n; o.textContent = n; sel.appendChild(o); });
  } catch (_) {}
  bind(); renderModes(); renderHotkey(); refreshStatus(); moveIndicator();
  renderLearned(await invoke("get_learned"));
  listen("voiceo://history", renderHistory);
  listen("voiceo://learned", async () => { renderLearned(await invoke("get_learned")); mascot.react("learned", 1600, true); });
  listen("voiceo://setup", () => { refreshStatus(); renderHotkey(); });
  document.fonts?.ready.then(() => { moveIndicator(); $$("[data-seg]").forEach((s) => s.place && s.place()); });
})();
