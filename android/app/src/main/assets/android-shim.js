// Runs the shared settings UI (src/index.html + app.js) inside the Android WebView:
// maps the Tauri API the UI expects to the Kotlin bridge and adds Android-only bits.
(() => {
  document.documentElement.classList.add("android");
  const pending = new Map();
  const listeners = {};
  let nextId = 1;
  window.__voiceoResolve = (id, ok, json) => {
    const p = pending.get(id);
    if (!p) return;
    pending.delete(id);
    const v = JSON.parse(json);
    ok ? p.res(v) : p.rej(v);
  };
  window.__voiceoEmit = (name, payload) => (listeners[name] || []).forEach((cb) => cb({ payload }));
  window.__TAURI__ = {
    core: {
      invoke(cmd, args = {}) {
        return new Promise((res, rej) => {
          const id = nextId++;
          pending.set(id, { res, rej });
          VoiceoBridge.invoke(id, cmd, JSON.stringify(args));
        });
      },
    },
    event: {
      listen(name, cb) {
        (listeners[name] = listeners[name] || []).push(cb);
        return Promise.resolve(() => {});
      },
    },
  };

  // navigator.clipboard is unavailable on file: pages
  try { Object.defineProperty(navigator, "clipboard", { value: { writeText: (t) => { VoiceoBridge.copy(t); return Promise.resolve(); } } }); } catch (_) {}

  // window / status bar follow the UI theme
  new MutationObserver(() => VoiceoBridge.theme(document.documentElement.dataset.theme || "dark"))
    .observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

  const de = () => window.LANG !== "en";
  window.platformChecks = async (s) => {
    const row = (ok, name, okText, badText) => [ok, name, ok ? okText : badText];
    const checks = [
      row(s.api_key, "API-Key", de() ? "Gesetzt" : "Set", de() ? "Fehlt" : "Missing"),
      row(s.accessibility, de() ? "Bedienungshilfe" : "Accessibility service", de() ? "Aktiv: Overlay, Einfügen, Lernen" : "On: overlay, insert, learn", de() ? "Aus. Nötig für Overlay, Einfügen und Lernen" : "Off. Needed for overlay, insert and learning"),
      row(s.mic, de() ? "Mikrofon" : "Microphone", de() ? "Erlaubt" : "Allowed", de() ? "Nicht erlaubt" : "Not allowed"),
      row(s.notifications, de() ? "Benachrichtigung" : "Notification", de() ? "Erlaubt" : "Allowed", de() ? "Aus. Nötig für die Aufnahme-Anzeige" : "Off. Needed for the recording notice"),
    ];
    return { checks, needSetup: !(s.accessibility && s.mic && s.notifications), core: s.accessibility && s.mic && s.api_key };
  };

  // how to start a dictation on a phone (there is no global hotkey)
  window.addEventListener("DOMContentLoaded", () => {
    const sec = document.querySelector("#tab-general");
    if (!sec) return;
    const card = document.createElement("div");
    card.className = "card";
    card.innerHTML = `<h2></h2>
      <div class="row col"><div class="l"><small id="howto" style="max-width:none"></small></div>
      <button class="btn primary" id="start-test"></button></div>`;
    const first = sec.querySelector(".card");
    first.after(card);
    const fill = () => {
      card.querySelector("h2").textContent = de() ? "Diktat starten" : "Start dictation";
      card.querySelector("#howto").innerHTML = de()
        ? `<b>Schnelleinstellung:</b> Kachel &bdquo;Voiceo&ldquo; hinzufügen (Statusleiste herunterziehen, Stift).<br>
           <b>Seitentaste:</b> Einstellungen &rarr; Erweiterte Funktionen &rarr; Seitentaste &rarr; Doppeltes Drücken &rarr; App öffnen &rarr; Voiceo &bdquo;Diktat&ldquo;.<br>
           <b>Bedienungshilfen-Taste:</b> Einstellungen &rarr; Bedienungshilfen &rarr; Voiceo &rarr; Verknüpfung.<br>
           Einmal tippen startet, nochmal tippen fügt den Text am Cursor ein.`
        : `<b>Quick Settings:</b> add the &ldquo;Voiceo&rdquo; tile (pull down the status bar, pencil).<br>
           <b>Side key:</b> Settings &rarr; Advanced features &rarr; Side key &rarr; Double press &rarr; Open app &rarr; Voiceo &ldquo;Dictate&rdquo;.<br>
           <b>Accessibility button:</b> Settings &rarr; Accessibility &rarr; Voiceo &rarr; Shortcut.<br>
           Tap once to start, tap again to insert the text at the cursor.`;
      card.querySelector("#start-test").textContent = de() ? "Jetzt testen" : "Try now";
    };
    fill();
    card.querySelector("#start-test").onclick = () => { __TAURI__.core.invoke("start_dictation"); };
    new MutationObserver(fill).observe(document.documentElement, { attributes: true, attributeFilter: ["lang"] });
  });
})();
