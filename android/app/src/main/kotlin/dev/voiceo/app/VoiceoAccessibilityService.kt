package dev.voiceo.app

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.AccessibilityButtonController
import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.graphics.PixelFormat
import android.os.Build
import android.os.Bundle
import android.os.PersistableBundle
import android.view.Gravity
import android.view.WindowManager
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.webkit.JavascriptInterface
import android.webkit.WebView
import org.json.JSONArray
import org.json.JSONObject

/**
 * The phone-side counterpart of the desktop overlay + paste + AT-SPI watcher:
 * - draws the bubble out of the camera cutout (accessibility overlay window, never takes focus)
 * - inserts the dictated text into the focused field
 * - watches that field afterwards to learn the user's corrections
 * - the accessibility button / shortcut starts and stops dictation
 */
class VoiceoAccessibilityService : AccessibilityService(), Dictation.Listener {
    private lateinit var store: Store
    private lateinit var groq: Groq
    private var web: WebView? = null
    private var overlayAttached = false
    private var pageReady = false
    private val pendingJs = ArrayList<String>()
    private var hideRunnable: Runnable? = null
    private val handler = android.os.Handler(android.os.Looper.getMainLooper())

    // ---------- lifecycle ----------

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
        store = Store(this)
        groq = Groq(this)
        Dictation.listeners.add(this)
        buildOverlay()
        runCatching {
            accessibilityButtonController.registerAccessibilityButtonCallback(object : AccessibilityButtonController.AccessibilityButtonCallback() {
                override fun onClicked(controller: AccessibilityButtonController) { Dictation.toggle(this@VoiceoAccessibilityService) }
            })
        }
    }

    override fun onUnbind(intent: android.content.Intent?): Boolean {
        Dictation.listeners.remove(this)
        detachOverlay()
        if (instance === this) instance = null
        return super.onUnbind(intent)
    }

    override fun onInterrupt() {}

    // ---------- overlay ----------

    @SuppressLint("SetJavaScriptEnabled", "JavascriptInterface")
    private fun buildOverlay() {
        val wv = WebView(this)
        wv.setBackgroundColor(0)
        wv.settings.javaScriptEnabled = true
        wv.settings.allowFileAccess = true
        wv.isVerticalScrollBarEnabled = false
        wv.addJavascriptInterface(object {
            @JavascriptInterface fun ready() { Dictation.post { pageReady = true; pendingJs.forEach { wv.evaluateJavascript(it, null) }; pendingJs.clear() } }
        }, "VoiceoNative")
        wv.loadUrl("file:///android_asset/overlay-android.html")
        web = wv
    }

    private fun js(code: String) {
        val wv = web ?: return
        if (!pageReady) { pendingJs.add(code); return }
        wv.evaluateJavascript(code, null)
    }

    private fun density() = resources.displayMetrics.density

    /** Camera cutout in CSS px: [centerX, centerY, radius]; hole-less devices pop out of the top edge centre. */
    private fun cutout(): DoubleArray {
        val wm = getSystemService(WindowManager::class.java)
        val d = density()
        val bounds = wm.currentWindowMetrics.bounds
        var rect: android.graphics.Rect? = null
        if (Build.VERSION.SDK_INT >= 30) {
            rect = wm.currentWindowMetrics.windowInsets.displayCutout?.boundingRectTop
        }
        if (rect == null || rect.isEmpty) return doubleArrayOf(bounds.width() / d / 2.0, -2.0, 9.0)
        return doubleArrayOf(rect.exactCenterX() / d.toDouble(), rect.exactCenterY() / d.toDouble(), maxOf(rect.width(), rect.height()) / 2.0 / d)
    }

    private fun attachOverlay() {
        val wv = web ?: return
        hideRunnable?.let { handler.removeCallbacks(it) }
        hideRunnable = null
        if (overlayAttached) return
        val lp = WindowManager.LayoutParams(
            WindowManager.LayoutParams.MATCH_PARENT,
            (OVERLAY_HEIGHT_DP * density()).toInt(),
            WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN or WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS or
                WindowManager.LayoutParams.FLAG_HARDWARE_ACCELERATED,
            PixelFormat.TRANSLUCENT,
        )
        lp.gravity = Gravity.TOP or Gravity.START
        lp.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
        lp.setTitle("Voiceo overlay")
        runCatching { getSystemService(WindowManager::class.java).addView(wv, lp); overlayAttached = true }
        val c = cutout()
        val s = store.settings()
        js("VoiceoOverlay.config(${c[0]},${c[1]},${c[2]},${JSONObject.quote(s.optString("font", "lexend"))},${JSONObject.quote(s.optString("ui_lang", "de"))})")
    }

    private fun detachOverlay() {
        val wv = web ?: return
        if (!overlayAttached) return
        runCatching { getSystemService(WindowManager::class.java).removeView(wv) }
        overlayAttached = false
    }

    override fun onState(state: String, message: String?) {
        if (!store.settings().optBoolean("overlay", true)) return
        if (state != "idle") attachOverlay()
        js("VoiceoOverlay.state(${JSONObject.quote(state)},${if (message == null) "null" else JSONObject.quote(message)})")
        if (state == "idle") {
            val r = Runnable { if (Dictation.state == "idle") detachOverlay() }
            hideRunnable = r
            handler.postDelayed(r, 900)
        }
    }

    override fun onAudio(rms: Float, bands: FloatArray) {
        if (!overlayAttached) return
        val arr = JSONArray(); bands.forEach { arr.put(it.toDouble()) }
        js("VoiceoOverlay.audio($rms,$arr)")
    }

    // ---------- insertion ----------

    private fun focusedInput(): AccessibilityNodeInfo? {
        val root = rootInActiveWindow ?: return null
        root.findFocus(AccessibilityNodeInfo.FOCUS_INPUT)?.let { if (it.isEditable) return it }
        // some apps do not report input focus: take a focused editable node instead
        val stack = ArrayDeque<AccessibilityNodeInfo>().apply { add(root) }
        var firstEditable: AccessibilityNodeInfo? = null
        while (stack.isNotEmpty()) {
            val n = stack.removeLast()
            if (n.isEditable) { if (n.isFocused) return n; if (firstEditable == null) firstEditable = n }
            for (i in 0 until n.childCount) n.getChild(i)?.let { stack.add(it) }
        }
        return firstEditable?.takeIf { it.isAccessibilityFocused || it.isFocused }
    }

    /** Must run on the main thread. Returns false when no text field is available. */
    fun insertText(text: String, s: JSONObject): Boolean {
        val node = focusedInput() ?: return false
        var ok = false
        if (s.optString("insert_method", "paste") == "paste") ok = pasteInto(node, text, s.optBoolean("copy_to_clipboard"))
        if (!ok) ok = setTextInto(node, text)
        if (ok && s.optBoolean("auto_learn", true) && !node.isPassword) startWatch(node, text)
        return ok
    }

    private fun pasteInto(node: AccessibilityNodeInfo, text: String, keep: Boolean): Boolean {
        val cm = getSystemService(ClipboardManager::class.java)
        val prev = cm.primaryClip
        val clip = ClipData.newPlainText("Voiceo", text)
        if (Build.VERSION.SDK_INT >= 33) clip.description.extras = PersistableBundle().apply { putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true) }
        cm.setPrimaryClip(clip)
        val ok = node.performAction(AccessibilityNodeInfo.ACTION_PASTE)
        if (!keep) android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
            runCatching { if (prev != null) cm.setPrimaryClip(prev) else cm.clearPrimaryClip() }
        }, 700)
        return ok
    }

    private fun setTextInto(node: AccessibilityNodeInfo, text: String): Boolean {
        val cur = if (node.isShowingHintText) "" else node.text?.toString().orEmpty()
        var a = node.textSelectionStart; var b = node.textSelectionEnd
        if (a < 0 || b < 0 || a > cur.length || b > cur.length) { a = cur.length; b = cur.length }
        val start = minOf(a, b); val end = maxOf(a, b)
        val merged = cur.substring(0, start) + text + cur.substring(end)
        val args = Bundle().apply { putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, merged) }
        if (!node.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args)) return false
        val sel = Bundle().apply {
            putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, start + text.length)
            putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, start + text.length)
        }
        node.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, sel)
        return true
    }

    // ---------- learning from edits (port of data/atspi_helper.py) ----------

    private class Track(val node: AccessibilityNodeInfo, val original: String) {
        var prefix = ""; var suffix = ""; var region = original; var bound = false
        var changed = System.currentTimeMillis()
    }

    private var track: Track? = null
    private val idleFinalize = Runnable { finalizeTrack("idle") }
    private val goneFinalize = Runnable { finalizeTrack("gone") }

    private fun norm(s: String) = s.trim().split(Regex("\\s+")).joinToString(" ")

    private fun startWatch(node: AccessibilityNodeInfo, original: String) {
        finalizeTrack("new-dictation")
        val t = Track(node, original)
        track = t
        for (delay in longArrayOf(250, 900, 2500)) handler.postDelayed({ if (track === t && !t.bound) tryBind(t) }, delay)
        handler.removeCallbacks(idleFinalize); handler.postDelayed(idleFinalize, IDLE_FINALIZE_MS)
    }

    private fun nodeText(n: AccessibilityNodeInfo): String? {
        if (!n.refresh()) return null
        return if (n.isShowingHintText) "" else n.text?.toString()
    }

    private fun tryBind(t: Track): Boolean {
        val text = nodeText(t.node) ?: return false
        val idx = text.lastIndexOf(t.original)
        if (idx < 0) return false
        t.prefix = text.substring(maxOf(0, idx - 30), idx)
        t.suffix = text.substring(idx + t.original.length, minOf(text.length, idx + t.original.length + 30))
        t.region = t.original; t.bound = true; t.changed = System.currentTimeMillis()
        return true
    }

    /** Current version of the pasted passage inside the full field text. */
    private fun extract(text: String, t: Track): String? {
        var start: Int? = null
        if (t.prefix.isNotEmpty()) { val p = text.lastIndexOf(t.prefix); if (p >= 0) start = p + t.prefix.length } else start = 0
        if (start == null) {
            // prefix edited away: find the longest common run with the original
            val o = t.original
            var best = 0; var bestA = 0; var bestB = 0
            val prev = IntArray(o.length + 1)
            for (i in 1..text.length) {
                var diag = 0
                for (j in 1..o.length) {
                    val tmp = prev[j]
                    prev[j] = if (text[i - 1] == o[j - 1]) diag + 1 else 0
                    if (prev[j] > best) { best = prev[j]; bestA = i - best; bestB = j - best }
                    diag = tmp
                }
            }
            if (best < maxOf(4, o.length / 4)) return null
            start = maxOf(0, bestA - bestB)
        }
        var end: Int? = null
        if (t.suffix.isNotEmpty()) { val e = text.indexOf(t.suffix, start); if (e >= 0) end = e } else end = text.length
        if (end == null) end = minOf(text.length, start + t.original.length + 40)
        return if (end >= start) text.substring(start, end) else null
    }

    private fun finalizeTrack(reason: String) {
        val t = track ?: return
        track = null
        handler.removeCallbacks(idleFinalize); handler.removeCallbacks(goneFinalize)
        val region = t.region.trim()
        if (region.isNotEmpty() && norm(region) != norm(t.original)) learnFromEdit(t.original, region)
    }

    private fun learnFromEdit(original: String, corrected: String) {
        val s = store.settings()
        if (s.optString("api_key").isBlank()) return
        Thread {
            val pairs = try { groq.extractLearning(s, original, corrected) } catch (e: Exception) { Learn.diffPairs(original, corrected) }
            if (pairs.isEmpty()) return@Thread
            pairs.forEach { store.addLearned(it.first, it.second) }
            val (f, to) = pairs[0]
            val more = if (pairs.size > 1) " +${pairs.size - 1}" else ""
            Dictation.emit("learned", "Gelernt: $f → $to$more")
            Dictation.postDelayed(2200) { Dictation.emit("idle") }
        }.start()
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent) {
        val t = track ?: return
        when (event.eventType) {
            AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED -> {
                val src = event.source ?: return
                if (src != t.node) return
                if (!t.bound && !tryBind(t)) return
                val text = nodeText(src) ?: return
                val r = extract(text, t)
                if (r != null) {
                    t.region = r; t.changed = System.currentTimeMillis()
                    handler.removeCallbacks(goneFinalize)
                } else {
                    // field cleared (message sent) or passage gone: keep the last version and finish soon
                    handler.removeCallbacks(goneFinalize); handler.postDelayed(goneFinalize, 1500)
                }
                handler.removeCallbacks(idleFinalize); handler.postDelayed(idleFinalize, IDLE_FINALIZE_MS)
            }
            AccessibilityEvent.TYPE_VIEW_FOCUSED -> {
                val src = event.source ?: return
                if (src != t.node) finalizeTrack("focus-lost")
            }
        }
    }

    companion object {
        @Volatile var instance: VoiceoAccessibilityService? = null
        const val OVERLAY_HEIGHT_DP = 220
        const val IDLE_FINALIZE_MS = 45_000L
    }
}
