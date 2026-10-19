const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];

const MODES = ["raw", "clean", "message", "email", "prompt", "notes", "custom"];

let S = {};
let saveTimer = null;
const refreshers = [];            // custom controls re-sync after settings load

// ---------- i18n ----------
function applyI18n() {
  document.documentElement.lang = window.LANG;
  $$("[data-i18n]").forEach((el) => { el.textContent = t(el.dataset.i18n); });
  $$("[data-i18n-ph]").forEach((el) => { el.placeholder = t(el.dataset.i18nPh); });
  placeSegs();
}
function setLang(l) {
  window.LANG = l; S.ui_lang = l;
  applyI18n();
  $$("select[data-k]").forEach((s) => s.rebuild && s.rebuild());
  renderModes(); renderHotkey(); refreshStatus();
  invoke("get_learned").then(renderLearned);
  if ($("#tab-history").classList.contains("active")) renderHistory();
  queueSave();
}
// ---------- tiny helpers ----------
function toast(msg) {
  const el = $("#toast");
  el.textContent = msg; el.classList.add("on");
  clearTimeout(toast.t); toast.t = setTimeout(() => el.classList.remove("on"), 1500);
}
function queueSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    try { await invoke("save_settings", { settings: S }); toast(t("saved")); mascot.react("done"); refreshStatus(); }
    catch (e) { toast(t("error") + ": " + e); }
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

// ---------- theme + font ----------
function applyTheme() {
  const th = S.theme || "system";
  const light = th === "light" || (th === "system" && matchMedia("(prefers-color-scheme: light)").matches);
  document.documentElement.dataset.theme = light ? "light" : "dark";
  document.documentElement.dataset.font = S.font || "lexend";
}
matchMedia("(prefers-color-scheme: light)").addEventListener?.("change", applyTheme);

// ---------- sidebar mascot ----------
const mascot = (() => {
  const cv = $("#logo"), ctx = cv.getContext("2d");
  ctx.scale(2, 2);
  let state = "idle", until = 0, bounce = 0, look = [0, 0];
  addEventListener("pointermove", (e) => {
    const r = cv.getBoundingClientRect();
    const dx = e.clientX - (r.left + r.width / 2), dy = e.clientY - (r.top + r.height / 2);
    const d = Math.hypot(dx, dy) || 1, k = Math.min(1, d / 240);
    look = [(dx / d) * k, (dy / d) * k * 0.7];
  });
  $("#brand").addEventListener("click", () => api.react("done", 1100, true));
  const api = { react(s, ms = 900, hop = false) { state = s; until = performance.now() + ms; if (hop) bounce = 1; } };
  (function loop(now) {
    if (state !== "idle" && now > until) state = "idle";
    bounce *= 0.9;
    ctx.clearRect(0, 0, 62, 62);
    window.Voi.draw(ctx, { x: 31, y: 36 - Math.sin(bounce * 7) * bounce * 7, s: 0.62, state, t: now, level: 0, look });
    requestAnimationFrame(loop);
  })(0);
  return api;
})();

// ---------- custom controls ----------
function closeAll() { $$(".sel.open").forEach((x) => { x.classList.remove("open"); x.closest(".card")?.classList.remove("raise"); }); }
function openSel(wrap, menu) {
  closeAll();
  wrap.classList.add("open");
  wrap.closest(".card")?.classList.add("raise");
  // flip upwards when there is not enough room below
  menu.classList.remove("up");
  const r = wrap.getBoundingClientRect();
  if (innerHeight - r.bottom < Math.min(menu.scrollHeight, 280) + 24 && r.top > innerHeight - r.bottom) menu.classList.add("up");
}
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
  const sync = () => {
    const o = [...native.options].find((x) => x.value === native.value) || native.options[0];
    btn.textContent = o ? o.textContent : "";
    $$("div", menu).forEach((d) => d.classList.toggle("on", d.dataset.v === native.value));
  };
  const build = () => {
    menu.innerHTML = "";
    [...native.options].forEach((o) => {
      const d = document.createElement("div");
      d.textContent = o.dataset.i18n ? t(o.dataset.i18n) : o.textContent; d.dataset.v = o.value;
      if (o.dataset.i18n) o.textContent = d.textContent;
      d.onclick = () => { native.value = o.value; native.dispatchEvent(new Event("input")); sync(); closeAll(); };
      menu.appendChild(d);
    });
    sync();
  };
  btn.onclick = (e) => { e.stopPropagation(); wrap.classList.contains("open") ? closeAll() : openSel(wrap, menu); };
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
  const mark = () => $$("div", menu).forEach((d) => d.classList.toggle("on", d.textContent === input.value));
  opts.forEach((v) => {
    const d = document.createElement("div"); d.textContent = v;
    d.onclick = () => { input.value = v; input.dispatchEvent(new Event("input")); closeAll(); mark(); };
    menu.appendChild(d);
  });
  input.addEventListener("focus", () => { mark(); openSel(wrap, menu); });
  input.addEventListener("input", mark);
  wrap.addEventListener("click", (e) => e.stopPropagation());
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
}
document.addEventListener("click", closeAll);
addEventListener("keydown", (e) => { if (e.key === "Escape") closeAll(); });

function sliderFor(seg) {
  const thumb = document.createElement("div"); thumb.className = "thumb"; seg.prepend(thumb);
  seg.place = () => {
    const on = $("button.on", seg);
    if (!on) return;
    thumb.style.width = on.offsetWidth + "px"; thumb.style.transform = `translateX(${on.offsetLeft}px)`;
  };
  return seg.place;
}
function placeSegs() { requestAnimationFrame(() => $$(".seg").forEach((s) => s.place && s.place())); }

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
      if (k === "theme" || k === "font") applyTheme();
      if (k === "ui_lang") { setLang(S[k]); return; }
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
  });
}

// ---------- modes ----------
function renderModes() {
  const box = $("#modes");
  box.innerHTML = "";
  MODES.forEach((id) => {
    const b = document.createElement("button");
    b.className = "mode" + (S.mode === id ? " on" : "");
    b.innerHTML = `<b>${t("mode." + id)}</b><small>${t("mode." + id + ".d")}</small>`;
    b.onclick = () => { S.mode = id; renderModes(); queueSave(); };
    box.appendChild(b);
  });
  $("#custom-wrap").style.display = S.mode === "custom" ? "" : "none";
  box.style.opacity = S.cleanup ? 1 : 0.4;
  box.style.transition = "opacity .25s";
}

// ---------- hotkeys (both rows share the same component) ----------
const HK = {
  hotkey: { label: "hotkey", hint: "hotkey.hint" },
  learn_hotkey: { label: "learnkey", hint: "learnkey.hint" },
};
async function renderHotkey() {
  for (const [field, cfg] of Object.entries(HK)) {
    const row = $(`[data-hk="${field}"]`);
    const keys = (await invoke("key_labels", { codes: S[field] || [] }));
    row.innerHTML = `
      <div class="top">
        <div class="l"><b>${t(cfg.label)}</b><small>${t(cfg.hint)}</small></div>
        <div class="r"><div class="keys">${keys.map((l) => `<kbd>${l}</kbd>`).join("") || `<kbd>${t("off")}</kbd>`}</div>
          <button class="btn">${t("change")}</button></div>
      </div>`;
    $(".btn", row).onclick = async () => {
      row.classList.add("capturing");
      $(".keys", row).innerHTML = `<kbd>${t("press")}</kbd>`;
      $(".btn", row).disabled = true;
      try {
        const codes = await invoke("capture_hotkey");
        S[field] = codes; queueSave();   // empty = Esc pressed = hotkey off
      } catch (e) { toast(String(e)); }
      row.classList.remove("capturing");
      renderHotkey();
    };
  }
}

// ---------- learned corrections + terms ----------
let learnedItems = [];
function renderLearned(items) {
  if (items) learnedItems = items;
  const box = $("#learned");
  const q = ($("#l-search").value || "").trim().toLowerCase();
  const list = learnedItems.filter((it) => !q || it.from.toLowerCase().includes(q) || it.to.toLowerCase().includes(q));
  if (!learnedItems.length) { box.innerHTML = `<div class="empty">${t("learned.empty")}</div>`; return; }
  if (!list.length) { box.innerHTML = `<div class="empty">${t("nomatch")}</div>`; return; }
  box.innerHTML = "";
  [...list].reverse().forEach((it) => {
    const r = document.createElement("div");
    r.className = "row";
    const isTerm = !it.from;
    r.innerHTML = `<div class="l"><b></b>${isTerm ? "" : `<small>${t("times", { n: it.count })}</small>`}</div><button class="btn small">${t("remove")}</button>`;
    $("b", r).textContent = isTerm ? it.to : `${it.from}  →  ${it.to}`;
    if (isTerm) { const tag = document.createElement("span"); tag.className = "tag"; tag.textContent = t("term"); $("b", r).appendChild(tag); }
    $("button", r).onclick = async () => renderLearned(await invoke("delete_learned", { from: it.from, to: it.to }));
    box.appendChild(r);
  });
}
$("#l-search").addEventListener("input", () => renderLearned());
async function addLearned() {
  const from = $("#l-from").value.trim(), to = $("#l-to").value.trim();
  if (!to) return;
  renderLearned(await invoke("add_learned", { from, to }));
  $("#l-from").value = ""; $("#l-to").value = ""; $("#l-search").value = ""; mascot.react("learned", 1400, true);
}
$("#l-add").onclick = addLearned;
["#l-from", "#l-to"].forEach((id) => $(id).addEventListener("keydown", (e) => { if (e.key === "Enter") addLearned(); }));

// ---------- api test ----------
$("#test-key").onclick = async () => {
  const row = $("#key-msg-row"), msg = $("#key-msg");
  row.style.display = ""; msg.className = "hint"; msg.textContent = t("testing");
  try { const n = await invoke("test_api", { key: S.api_key || "" }); msg.textContent = t("connected", { n }); msg.className = "msg-ok"; mascot.react("done", 1400, true); }
  catch (e) { msg.textContent = String(e); msg.className = "msg-bad"; mascot.react("error", 1600); }
};

// ---------- status ----------
async function refreshStatus() {
  const s = await invoke("system_status");
  const pc = window.platformChecks ? await window.platformChecks(s) : null;   // Android supplies its own checks
  const checks = pc ? pc.checks : [
    [s.api_key, t("chk.key"), s.api_key ? t("chk.key.ok") : t("chk.key.bad")],
    [s.keyboards > 0, t("chk.hotkey"), s.keyboards > 0 ? t("chk.hotkey.ok", { n: s.keyboards }) : t("chk.hotkey.bad")],
    [s.wl_copy, t("chk.clip"), s.wl_copy ? t("chk.clip.ok") : t("chk.clip.bad")],
    [s.uinput, t("chk.paste"), s.uinput ? t("chk.paste.ok") : t("chk.paste.bad")],
    [s.atspi, t("chk.learn"), s.atspi ? t("chk.learn.ok") : t("chk.learn.bad")],
  ];
  const needSetup = pc ? pc.needSetup : !(s.keyboards > 0 && s.uinput);
  $("#checks").innerHTML = checks.map(([ok, name, d]) => `
    <div class="check"><span class="dot ${ok ? "ok" : "bad"}"></span><div><b>${name}</b><small>${d}</small></div></div>`).join("")
    + (needSetup ? `<div class="check"><button class="btn primary" id="run-setup">${t("setup")}</button><small>${t("setup.hint")}</small></div>` : "");
  const rs = $("#run-setup");
  if (rs) rs.onclick = async () => {
    rs.disabled = true; rs.textContent = t("setting-up");
    try { const ok = await invoke("run_setup"); toast(ok ? t("setup.done") : t("setup.retry")); }
    catch (e) { toast(String(e)); }
    refreshStatus(); renderHotkey();
  };
  const core = pc ? pc.core : checks.slice(0, 4).every((c) => c[0]);
  $("#mini-dot").className = "dot " + (core ? "ok" : "bad");
  $("#mini-text").textContent = core ? t("ready") : t("incomplete");
}

// ---------- history ----------
async function renderHistory() {
  const items = await invoke("get_history");
  const list = $("#hist-list");
  if (!items.length) { list.innerHTML = `<div class="empty">${t("history.empty")}</div>`; return; }
  list.innerHTML = "";
  const esc = (x) => x.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));
  items.forEach((it) => {
    const d = document.createElement("div");
    d.className = "hist";
    const time = new Date(it.ts * 1000).toLocaleString(window.LANG === "de" ? "de-DE" : "en-GB", { dateStyle: "short", timeStyle: "short" });
    d.innerHTML = `<div class="meta"><span>${time} · ${t("mode." + it.mode)}</span><span>${(it.ms / 1000).toFixed(1)} s</span></div>
      <p>${esc(it.text)}</p>${it.raw !== it.text ? `<p class="raw">${t("raw")}: ${esc(it.raw)}</p>` : ""}
      <div class="acts"><button class="btn small">${t("copy")}</button></div>`;
    $("button", d).onclick = () => { navigator.clipboard.writeText(it.text); toast(t("copied")); };
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
  closeAll();
  $$("#nav button").forEach((x) => x.classList.toggle("active", x === b));
  $$("main section").forEach((s) => s.classList.toggle("active", s.id === "tab-" + b.dataset.tab));
  moveIndicator();
  $("main").scrollTo({ top: 0 });
  placeSegs();
  if (b.dataset.tab === "history") renderHistory();
  if (b.dataset.tab === "general") refreshStatus();
}));
addEventListener("resize", moveIndicator);

// ---------- init ----------
(async () => {
  S = await invoke("get_settings");
  window.LANG = S.ui_lang || "de";
  applyTheme();
  applyI18n();
  try {
    const devices = await invoke("list_inputs");
    const sel = $("#mic-select");
    devices.forEach((n) => { const o = document.createElement("option"); o.value = n; o.textContent = n; sel.appendChild(o); });
  } catch (_) {}
  bind(); applyI18n(); renderModes(); renderHotkey(); refreshStatus(); moveIndicator();
  renderLearned(await invoke("get_learned"));
  listen("voiceo://history", renderHistory);
  listen("voiceo://learned", async () => { renderLearned(await invoke("get_learned")); mascot.react("learned", 1600, true); });
  listen("voiceo://setup", () => { refreshStatus(); renderHotkey(); });
  document.fonts?.ready.then(() => { moveIndicator(); placeSegs(); });
})();
