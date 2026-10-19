package dev.voiceo.app

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** Same JSON files and keys as the desktop app (settings.json, learned.json, history.json). */
class Store(ctx: Context) {
    private val dir: File = ctx.filesDir
    private val lock = Any()

    private val settingsFile get() = File(dir, "settings.json")
    private val learnedFile get() = File(dir, "learned.json")
    private val historyFile get() = File(dir, "history.json")

    // ---------- settings ----------
    fun settings(): JSONObject = synchronized(lock) {
        val s = defaults()
        runCatching { JSONObject(settingsFile.readText()) }.getOrNull()?.let { saved ->
            for (k in saved.keys()) s.put(k, saved.get(k))
        }
        if (s.optString("llm_model").startsWith("llama-")) s.put("llm_model", "qwen/qwen3.8-27b")
        s
    }

    fun saveSettings(s: JSONObject) = synchronized(lock) {
        settingsFile.writeText(s.toString(2))
        settingsFile.setReadable(false, false); settingsFile.setReadable(true, true)
    }

    // ---------- learned ----------
    fun learned(): JSONArray = synchronized(lock) {
        runCatching { JSONArray(learnedFile.readText()) }.getOrDefault(JSONArray())
    }

    fun saveLearned(a: JSONArray) = synchronized(lock) { learnedFile.writeText(a.toString(2)) }

    fun learnedItems(): List<Learned> = synchronized(lock) {
        val a = learned()
        (0 until a.length()).map { val o = a.getJSONObject(it); Learned(o.optString("from"), o.optString("to"), o.optInt("count", 1)) }
    }

    fun saveLearnedItems(items: List<Learned>) {
        val a = JSONArray()
        items.forEach { a.put(JSONObject().put("from", it.from).put("to", it.to).put("count", it.count)) }
        saveLearned(a)
    }

    /** Adds a correction (or a plain term with empty `from`); returns the new list. */
    fun addLearned(from: String, to: String): List<Learned> = synchronized(lock) {
        val items = learnedItems().toMutableList()
        Learn.add(items, from, to)
        saveLearnedItems(items)
        items
    }

    fun deleteLearned(from: String, to: String): List<Learned> = synchronized(lock) {
        val items = learnedItems().filterNot { it.from == from && it.to == to }
        saveLearnedItems(items)
        items
    }

    // ---------- history ----------
    fun history(): JSONArray = synchronized(lock) {
        runCatching { JSONArray(historyFile.readText()) }.getOrDefault(JSONArray())
    }

    fun addHistory(raw: String, text: String, mode: String, ms: Long, limit: Int) = synchronized(lock) {
        val old = history()
        val out = JSONArray()
        out.put(JSONObject().put("ts", System.currentTimeMillis() / 1000).put("raw", raw).put("text", text).put("mode", mode).put("ms", ms))
        for (i in 0 until minOf(old.length(), limit.coerceAtLeast(1) - 1)) out.put(old.get(i))
        historyFile.writeText(out.toString())
    }

    fun clearHistory() = synchronized(lock) { historyFile.writeText("[]") }

    companion object {
        fun defaults(): JSONObject {
            val de = java.util.Locale.getDefault().language == "de"
            return JSONObject()
                .put("api_key", "")
                .put("hotkey", JSONArray())
                .put("hotkey_mode", "toggle")
                .put("learn_hotkey", JSONArray())
                .put("language", "auto")
                .put("stt_model", "whisper-large-v3")
                .put("llm_model", "qwen/qwen3.8-27b")
                .put("cleanup", true)
                .put("mode", "clean")
                .put("custom_prompt", "")
                .put("dictionary", JSONArray())
                .put("auto_insert", true)
                .put("insert_method", "paste")
                .put("paste_shortcut", "ctrl+v")
                .put("copy_to_clipboard", false)
                .put("stt_temperature", 0.0)
                .put("llm_temperature", 0.2)
                .put("min_words_cleanup", 3)
                .put("whisper_prompt", "")
                .put("overlay", true)
                .put("max_seconds", 300)
                .put("silence_threshold", 0.01)
                .put("trim_silence", true)
                .put("history_limit", 100)
                .put("input_device", "")
                .put("normalize", false)
                .put("highpass", false)
                .put("auto_learn", true)
                .put("theme", "system")
                .put("font", "lexend")
                .put("ui_lang", if (de) "de" else "en")
        }
    }
}

fun JSONObject.strs(key: String): List<String> {
    val a = optJSONArray(key) ?: return emptyList()
    return (0 until a.length()).map { a.optString(it) }.filter { it.isNotBlank() }
}
