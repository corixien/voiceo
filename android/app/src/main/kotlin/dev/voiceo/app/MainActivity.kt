package dev.voiceo.app

import android.Manifest
import android.annotation.SuppressLint
import android.content.ComponentName
import android.content.Intent
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebChromeClient
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.app.Activity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.Executors

/** Hosts the shared settings UI (src/index.html) in a WebView; android-shim.js maps Tauri commands to [Bridge]. */
class MainActivity : Activity(), Dictation.Listener {
    private lateinit var store: Store
    private lateinit var web: WebView
    private lateinit var root: FrameLayout
    private val pool = Executors.newCachedThreadPool()

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store = Store(this)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        root = FrameLayout(this)
        web = WebView(this)
        web.settings.javaScriptEnabled = true
        web.settings.domStorageEnabled = true
        web.settings.allowFileAccess = true
        web.webChromeClient = WebChromeClient()
        web.webViewClient = WebViewClient()
        web.addJavascriptInterface(Bridge(), "VoiceoBridge")
        root.addView(web, ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        setContentView(root)
        ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            v.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
            WindowInsetsCompat.CONSUMED
        }
        applyChrome(store.settings().optString("theme", "system"))
        loadUi()
        ensurePermissions()
        Dictation.listeners.add(this)
    }

    override fun onDestroy() {
        Dictation.listeners.remove(this)
        super.onDestroy()
    }

    override fun onResume() {
        super.onResume()
        emit("voiceo://setup", "null")
    }

    private fun isDark(theme: String) = when (theme) {
        "dark" -> true
        "light" -> false
        else -> (resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK) == android.content.res.Configuration.UI_MODE_NIGHT_YES
    }

    /** Window background + status bar icons follow the UI theme. */
    private fun applyChrome(theme: String) {
        val dark = isDark(theme)
        val bg = if (dark) Color.parseColor("#1A1918") else Color.parseColor("#F6F4EE")
        root.setBackgroundColor(bg)
        web.setBackgroundColor(bg)
        WindowCompat.getInsetsController(window, window.decorView).apply {
            isAppearanceLightStatusBars = !dark
            isAppearanceLightNavigationBars = !dark
        }
    }

    private fun loadUi() {
        var html = assets.open("index.html").bufferedReader().use { it.readText() }
        html = html.replace("<head>", "<head>\n<script src=\"android-shim.js\"></script>\n<link rel=\"stylesheet\" href=\"mobile.css\">")
        web.loadDataWithBaseURL("file:///android_asset/index.html", html, "text/html", "utf-8", null)
    }

    private fun ensurePermissions() {
        val need = ArrayList<String>()
        if (!Dictation.hasMic(this)) need.add(Manifest.permission.RECORD_AUDIO)
        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED) need.add(Manifest.permission.POST_NOTIFICATIONS)
        if (need.isNotEmpty()) requestPermissions(need.toTypedArray(), 1)
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        emit("voiceo://setup", "null")
    }

    private fun emit(name: String, payloadJson: String) {
        runOnUiThread { web.evaluateJavascript("window.__voiceoEmit&&window.__voiceoEmit(${JSONObject.quote(name)},$payloadJson)", null) }
    }

    override fun onState(state: String, message: String?) {
        if (state == "done") emit("voiceo://history", "null")
        if (state == "learned") emit("voiceo://learned", "null")
    }

    private fun accessibilityEnabled(): Boolean {
        if (VoiceoAccessibilityService.instance != null) return true
        val flat = Settings.Secure.getString(contentResolver, Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES) ?: return false
        val me = ComponentName(this, VoiceoAccessibilityService::class.java).flattenToString()
        return flat.split(':').any { it.equals(me, true) || it.equals(me.replace("dev.voiceo.app/dev.voiceo.app.", "dev.voiceo.app/."), true) }
    }

    /** Android 13+ blocks accessibility services of apps that did not come through a store-like installer. */
    private fun accessibilityRestricted(): Boolean {
        if (Build.VERSION.SDK_INT < 33) return false
        return runCatching {
            val ops = getSystemService(android.app.AppOpsManager::class.java)
            ops.unsafeCheckOpNoThrow("android:access_restricted_settings", android.os.Process.myUid(), packageName) != android.app.AppOpsManager.MODE_ALLOWED
        }.getOrDefault(false)
    }

    private fun learnedJson(items: List<Learned>): String {
        val a = JSONArray()
        items.forEach { a.put(JSONObject().put("from", it.from).put("to", it.to).put("count", it.count)) }
        return a.toString()
    }

    inner class Bridge {
        @JavascriptInterface
        fun invoke(id: Int, cmd: String, argsJson: String) {
            pool.execute {
                val (ok, out) = try { true to run(cmd, JSONObject(argsJson.ifEmpty { "{}" })) } catch (e: Exception) { false to JSONObject.quote(e.message ?: e.javaClass.simpleName) }
                runOnUiThread { web.evaluateJavascript("window.__voiceoResolve($id,$ok,${JSONObject.quote(out)})", null) }
            }
        }

        @JavascriptInterface fun copy(text: String) {
            runOnUiThread { getSystemService(android.content.ClipboardManager::class.java).setPrimaryClip(android.content.ClipData.newPlainText("Voiceo", text)) }
        }

        @JavascriptInterface fun theme(name: String) { runOnUiThread { applyChrome(name) } }

        private fun run(cmd: String, a: JSONObject): String = when (cmd) {
            "get_settings" -> store.settings().toString()
            "save_settings" -> { store.saveSettings(a.getJSONObject("settings")); "null" }
            "get_learned" -> learnedJson(store.learnedItems())
            "add_learned" -> learnedJson(store.addLearned(a.optString("from"), a.optString("to")))
            "delete_learned" -> learnedJson(store.deleteLearned(a.optString("from"), a.optString("to")))
            "get_history" -> store.history().toString()
            "clear_history" -> { store.clearHistory(); "null" }
            "test_api" -> Groq(this@MainActivity).checkKey(a.optString("key")).toString().let { JSONObject.quote(it) }
            "system_status" -> JSONObject()
                .put("api_key", store.settings().optString("api_key").isNotBlank())
                .put("accessibility", accessibilityEnabled())
                .put("restricted", accessibilityRestricted())
                .put("mic", Dictation.hasMic(this@MainActivity))
                .put("notifications", Build.VERSION.SDK_INT < 33 || checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == android.content.pm.PackageManager.PERMISSION_GRANTED)
                .toString()
            "run_setup" -> {
                runOnUiThread {
                    if (!Dictation.hasMic(this@MainActivity)) ensurePermissions()
                    else if (!accessibilityEnabled()) startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
                    else ensurePermissions()
                }
                "false"
            }
            "open_app_info" -> {
                runOnUiThread { startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, android.net.Uri.parse("package:$packageName"))) }
                "null"
            }
            "start_dictation" -> { runOnUiThread { Dictation.toggle(this@MainActivity) }; "null" }
            "key_labels", "list_inputs" -> "[]"
            "capture_hotkey" -> throw IllegalStateException("Nicht auf Android verfügbar")
            else -> throw IllegalArgumentException("Unbekannter Befehl: $cmd")
        }
    }
}
