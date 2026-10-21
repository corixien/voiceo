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
      row(s.accessibility, de() ? "Bedienungshilfe" : "Accessibility service", de() ? "Aktiv: Overlay, Einfügen, Lernen" : "On: overlay, insert, learn",
        s.restricted ? (de() ? "Von Android gesperrt (App nicht aus einem Store). Lösung unter „Diktat starten“" : "Blocked by Android (app not from a store). See “Start dictation” for the fix")
                     : (de() ? "Aus. Nötig für Overlay, Einfügen und Lernen" : "Off. Needed for overlay, insert and learning")),
      row(s.mic, de() ? "Mikrofon" : "Microphone", de() ? "Erlaubt" : "Allowed", de() ? "Nicht erlaubt" : "Not allowed"),
      row(s.notifications, de() ? "Benachrichtigung" : "Notification", de() ? "Erlaubt" : "Allowed", de() ? "Aus. Nötig für die Aufnahme-Anzeige" : "Off. Needed for the recording notice"),
    ];
    return { checks, needSetup: !(s.accessibility && s.mic && s.notifications), core: s.accessibility && s.mic && s.api_key };
  };

  window.addEventListener("DOMContentLoaded", () => {
    if (window.I18N) { I18N.de["general.sub"] = "Groq-Key, Sprache, Overlay."; I18N.en["general.sub"] = "Groq key, language, overlay."; }
  });

  // bottom bar: sliding pill indicator (same spring as the desktop sidebar), horizontal
  window.addEventListener("DOMContentLoaded", () => {
    window.moveIndicator = () => {
      const b = document.querySelector("#nav button.active"), ind = document.querySelector("#nav-ind");
      if (!b || !ind) return;
      ind.style.width = b.offsetWidth + "px"; ind.style.height = b.offsetHeight + "px";
      ind.style.transform = `translateX(${b.offsetLeft}px)`;
    };
    window.moveIndicator();
    addEventListener("resize", window.moveIndicator);
  });

  // keyboard button settings (Android only)
  window.addEventListener("DOMContentLoaded", () => {
    const sec = document.querySelector("#tab-general");
    if (!sec) return;
    const card = document.createElement("div");
    card.className = "card";
    card.id = "kb-card";
    card.innerHTML = `<h2 id="kb-h"></h2>
      <div class="row"><div class="l"><b id="kb-on-t"></b><small id="kb-on-s"></small></div><label class="switch"><input type="checkbox" data-k="kb_button"><i></i></label></div>
      <div class="row"><div class="l"><b id="kb-size-t"></b><small id="kb-size-s"></small></div><input type="number" data-k="kb_button_size" min="28" max="120" step="2"></div>
      <div class="row"><div class="l"><b id="kb-lock-t"></b><small id="kb-lock-s"></small></div><label class="switch"><input type="checkbox" data-k="kb_button_locked"><i></i></label></div>
      <div class="row"><div class="l"><b id="kb-reset-t"></b></div><button class="btn" id="kb-reset"></button></div>`;
    sec.querySelector(".card").after(card);
    const fill = () => {
      const T = (id, v) => { card.querySelector("#" + id).textContent = v; };
      T("kb-h", de() ? "Tastatur-Button" : "Keyboard button");
      T("kb-on-t", de() ? "Button über der Tastatur" : "Button above the keyboard");
      T("kb-on-s", de() ? "Erscheint mit der Tastatur. Tippen startet das Diktat, nochmal tippen beendet es." : "Appears with the keyboard. Tap to start dictation, tap again to stop.");
      T("kb-size-t", de() ? "Größe (dp)" : "Size (dp)");
      T("kb-size-s", de() ? "Eine Taste ist etwa 40 bis 48 breit." : "A key is about 40 to 48 wide.");
      T("kb-lock-t", de() ? "Position sperren" : "Lock position");
      T("kb-lock-s", de() ? "Aus: Button mit dem Finger verschieben (gestrichelter Rand). Zum Einrichten ein Textfeld antippen, zum Beispiel das API-Key-Feld." : "Off: drag the button with your finger (dashed border). To set it up tap a text field, for example the API key field.");
      T("kb-reset-t", de() ? "Position zurücksetzen" : "Reset position");
      T("kb-reset", de() ? "Zurücksetzen" : "Reset");
    };
    fill();
    card.querySelector("#kb-reset").onclick = () => { __TAURI__.core.invoke("reset_kb_button"); };
    new MutationObserver(fill).observe(document.documentElement, { attributes: true, attributeFilter: ["lang"] });
  });

  // how to start a dictation on a phone (there is no global hotkey)
  window.addEventListener("DOMContentLoaded", () => {
    const sec = document.querySelector("#tab-general");
    if (!sec) return;
    const card = document.createElement("div");
    card.className = "card";
    card.innerHTML = `<h2></h2>
      <div class="row col"><div class="l"><small id="howto" style="max-width:none"></small></div>
      <div style="display:flex;gap:10px;flex-wrap:wrap"><button class="btn primary" id="start-test"></button><button class="btn" id="app-info"></button></div></div>`;
    const first = sec.querySelector(".card");
    first.after(card);
    const fill = () => {
      card.querySelector("h2").textContent = de() ? "Diktat starten" : "Start dictation";
      card.querySelector("#howto").innerHTML = de()
        ? `<strong>Tastatur-Button:</strong> erscheint über der Tastatur (Einstellungen unten).<br>
           <strong>Schnelleinstellung:</strong> Kachel &bdquo;Voiceo&ldquo; hinzufügen (Statusleiste herunterziehen, Stift).<br>
           <strong>Lautstärketasten:</strong> Einstellungen &rarr; Bedienungshilfen &rarr; Erweiterte Einstellungen &rarr; Lautstärketasten-Verknüpfung &rarr; Voiceo.<br>
           Einmal tippen startet, nochmal tippen fügt den Text am Cursor ein.<br><br>
           <strong>Bedienungshilfe ausgegraut oder &bdquo;App-Zugriff verweigert&ldquo;?</strong> Android sperrt sie für Apps, die nicht über einen Store-Installer kamen (Browser oder Dateimanager). Wege ohne Play Store:<br>
           1. Voiceo mit <strong>Obtainium</strong> installieren (Quelle <i>github.com/corixien/voiceo</i>). Dann gibt es meist keine Sperre.<br>
           2. Am Rechner mit USB-Debugging: <i>adb shell cmd appops set dev.voiceo.app ACCESS_RESTRICTED_SETTINGS allow</i><br>
           3. Ohne Rechner: Shizuku oder LADB (Drahtlos-Debugging) führt denselben Befehl am Handy aus.<br>
           Das Menü &#8942; in der App-Info zeigen nur manche Geräte, verlass dich nicht darauf.`
        : `<strong>Keyboard button:</strong> shown above the keyboard (settings below).<br>
           <strong>Quick Settings:</strong> add the &ldquo;Voiceo&rdquo; tile (pull down the status bar, pencil).<br>
           <strong>Volume keys:</strong> Settings &rarr; Accessibility &rarr; Advanced settings &rarr; Volume keys shortcut &rarr; Voiceo.<br>
           Tap once to start, tap again to insert the text at the cursor.<br><br>
           <strong>Accessibility service greyed out or &ldquo;App access denied&rdquo;?</strong> Android blocks it for apps that did not come through a store installer (browser or file manager). Ways without the Play Store:<br>
           1. Install Voiceo with <strong>Obtainium</strong> (source <i>github.com/corixien/voiceo</i>). Usually no block then.<br>
           2. On a computer with USB debugging: <i>adb shell cmd appops set dev.voiceo.app ACCESS_RESTRICTED_SETTINGS allow</i><br>
           3. Without a computer: Shizuku or LADB (wireless debugging) runs the same command on the phone.<br>
           Only some devices show the &#8942; menu in App info, do not rely on it.`;
      card.querySelector("#start-test").textContent = de() ? "Jetzt testen" : "Try now";
      card.querySelector("#app-info").textContent = de() ? "App-Info öffnen" : "Open app info";
    };
    fill();
    card.querySelector("#start-test").onclick = () => { __TAURI__.core.invoke("start_dictation"); };
    card.querySelector("#app-info").onclick = () => { __TAURI__.core.invoke("open_app_info"); };
    new MutationObserver(fill).observe(document.documentElement, { attributes: true, attributeFilter: ["lang"] });
  });
})();
