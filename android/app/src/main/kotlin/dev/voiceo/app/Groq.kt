package dev.voiceo.app

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.util.UUID

/** Port of src-tauri/src/groq.rs: Whisper + LLM cleanup with retry/backoff. */
class Groq(private val ctx: Context, private val notify: (String) -> Unit = {}) {
    class ApiError(val code: Int, message: String) : Exception(message)

    private class Resp(val code: Int, val body: String, val retryAfter: Double?)

    private val base = "https://api.groq.com/openai/v1"
    private val retryBudgetMs = 80_000L
    private val retryMax = 9

    private fun apiMessage(code: Int, body: String): String {
        val msg = runCatching { JSONObject(body).getJSONObject("error").optString("message") }.getOrNull().orEmpty().ifEmpty { "unbekannter Fehler" }
        return when (code) {
            401 -> "Groq: API-Key ungültig"
            429 -> "Groq: Rate-Limit erreicht, später erneut ($msg)"
            else -> "Groq $code: $msg"
        }
    }

    /** One HTTP attempt. Network problems throw IOException. */
    private fun once(key: String, path: String, method: String, contentType: String?, body: ByteArray?): Resp {
        val c = URL(base + path).openConnection() as HttpURLConnection
        try {
            c.requestMethod = method
            c.connectTimeout = 8_000
            c.readTimeout = 60_000
            c.setRequestProperty("Authorization", "Bearer $key")
            if (body != null) {
                c.doOutput = true
                c.setRequestProperty("Content-Type", contentType)
                c.setFixedLengthStreamingMode(body.size)
                c.outputStream.use { it.write(body) }
            }
            val code = c.responseCode
            val stream = if (code in 200..299) c.inputStream else c.errorStream
            val text = stream?.bufferedReader()?.use { it.readText() }.orEmpty()
            return Resp(code, text, c.getHeaderField("retry-after")?.toDoubleOrNull())
        } finally {
            c.disconnect()
        }
    }

    private fun send(key: String, path: String, method: String, contentType: String?, body: () -> ByteArray?): Resp {
        val start = System.currentTimeMillis()
        var attempt = 0
        while (true) {
            try {
                val r = once(key, path, method, contentType, body())
                if ((r.code == 429 || r.code >= 500) && attempt < retryMax && System.currentTimeMillis() - start < retryBudgetMs) {
                    val secs = (r.retryAfter ?: (1 shl minOf(attempt, 4)).toDouble()).coerceIn(1.5, 20.0) + 0.5
                    notify(if (r.code == 429) "Groq ausgelastet, versuche erneut …" else "Groq antwortet nicht, versuche erneut …")
                    Thread.sleep((secs * 1000).toLong()); attempt++; continue
                }
                return r
            } catch (e: java.io.IOException) {
                if (attempt < retryMax && System.currentTimeMillis() - start < retryBudgetMs) {
                    notify("Keine Verbindung, versuche erneut …")
                    Thread.sleep(minOf(1L shl minOf(attempt, 3), 8L) * 1000); attempt++; continue
                }
                throw ApiError(0, "Netzwerk: ${e.message}")
            }
        }
    }

    private fun check(r: Resp): String {
        if (r.code !in 200..299) throw ApiError(r.code, apiMessage(r.code, r.body))
        return r.body
    }

    fun checkKey(key: String): Int {
        val body = check(send(key, "/models", "GET", null) { null })
        return JSONObject(body).optJSONArray("data")?.length() ?: 0
    }

    // ---------- Whisper ----------

    private fun whisperPrompt(s: JSONObject, terms: List<String>): String {
        val seen = HashSet<String>()
        val vocab = (s.strs("dictionary") + terms + DEFAULT_VOCAB).filter { seen.add(it.lowercase()) }
        val p = StringBuilder()
        if (vocab.isNotEmpty()) {
            var list = vocab.joinToString(", ")
            if (list.length > 520) list = list.take(520)
            p.append("Glossar: ").append(list).append(". ")
        }
        val extra = s.optString("whisper_prompt").trim()
        if (extra.isNotEmpty()) p.append(extra).append(' ')
        p.append(PRIMER)
        return p.toString()
    }

    fun transcribe(s: JSONObject, wav: ByteArray, terms: List<String>): String {
        val (text, lang) = transcribeOnce(s, wav, terms, null)
        if (s.optString("language") == "auto") {
            val l = lang.lowercase()
            if (l.isNotEmpty() && !l.contains("german") && !l.contains("english") && l != "de" && l != "en") {
                return transcribeOnce(s, wav, terms, "de").first
            }
        }
        return text
    }

    private fun transcribeOnce(s: JSONObject, wav: ByteArray, terms: List<String>, forceLang: String?): Pair<String, String> {
        val prompt = whisperPrompt(s, terms)
        val boundary = "----voiceo" + UUID.randomUUID().toString().replace("-", "")
        val language = forceLang ?: s.optString("language").takeIf { it != "auto" && it.isNotEmpty() }
        val body = {
            val out = ByteArrayOutputStream(wav.size + 1024)
            fun field(name: String, value: String) {
                out.write("--$boundary\r\nContent-Disposition: form-data; name=\"$name\"\r\n\r\n$value\r\n".toByteArray())
            }
            out.write("--$boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n".toByteArray())
            out.write(wav); out.write("\r\n".toByteArray())
            field("model", s.optString("stt_model", "whisper-large-v3"))
            field("temperature", s.optDouble("stt_temperature", 0.0).toString())
            field("response_format", "verbose_json")
            field("prompt", prompt)
            if (language != null) field("language", language)
            out.write("--$boundary--\r\n".toByteArray())
            out.toByteArray()
        }
        val v = JSONObject(check(send(s.optString("api_key"), "/audio/transcriptions", "POST", "multipart/form-data; boundary=$boundary", body)))
        val lang = v.optString("language")
        val segs = v.optJSONArray("segments")
        if (segs != null) {
            val kept = ArrayList<String>()
            for (i in 0 until segs.length()) {
                val sg = segs.getJSONObject(i)
                val nsp = sg.optDouble("no_speech_prob", 0.0)
                val lp = sg.optDouble("avg_logprob", 0.0)
                val cr = sg.optDouble("compression_ratio", 1.0)
                val bad = (nsp > 0.5 && lp < -0.8) || nsp > 0.85 || lp < -1.3 || cr > 2.4
                if (!bad) kept.add(sg.optString("text").trim())
            }
            return kept.joinToString(" ").trim() to lang
        }
        return v.optString("text").trim() to lang
    }

    // ---------- LLM cleanup ----------

    private fun basePrompt(): String = ctx.assets.open("prompt_base.txt").bufferedReader().use { it.readText() }.trimEnd()

    private fun modePrompt(s: JSONObject): String? {
        val extra = when (s.optString("mode", "clean")) {
            "raw" -> return null
            "clean" -> "Style: keep the original wording and tone as close as possible. Only clean up."
            "message" -> "Style: casual chat message. Short sentences, natural tone, no greeting or sign-off unless spoken."
            "email" -> "Style: professional email body. Polite, clear, structured in short paragraphs. Add a greeting or closing only if the speaker implied one. Keep it in the speaker's language."
            "prompt" -> "Style: rewrite as a clear, precise instruction for an AI assistant or developer. Imperative, unambiguous, keep technical terms, file names and identifiers exactly. No pleasantries."
            "notes" -> "Style: concise bullet-point notes. One idea per bullet, start each bullet with '- '."
            "custom" -> s.optString("custom_prompt").trim()
            else -> "Style: keep the original wording."
        }
        val dict = s.strs("dictionary")
        return basePrompt() + "\n\n" + extra + if (dict.isNotEmpty()) "\n\nPreferred spellings (use exactly): ${dict.joinToString(", ")}." else ""
    }

    private fun chat(s: JSONObject, model: String, system: String, user: String, json: Boolean = false, temp: Double? = null): JSONObject {
        val body = JSONObject()
            .put("model", model)
            .put("temperature", temp ?: s.optDouble("llm_temperature", 0.2))
            .put("messages", JSONArray()
                .put(JSONObject().put("role", "system").put("content", system))
                .put(JSONObject().put("role", "user").put("content", user)))
        if (json) body.put("response_format", JSONObject().put("type", "json_object"))
        if (model.startsWith("openai/gpt-oss")) body.put("reasoning_effort", "low")
        val bytes = body.toString().toByteArray()
        return JSONObject(check(send(s.optString("api_key"), "/chat/completions", "POST", "application/json", { bytes })))
    }

    /** Returns null when cleanup is off for this mode or the result is unusable. */
    fun cleanup(s: JSONObject, text: String, hints: List<Pair<String, String>>, terms: List<String>): String? {
        var system = modePrompt(s) ?: return null
        if (terms.isNotEmpty()) system += "\nKnown names/terms (exact spelling): ${terms.take(80).joinToString(", ")}."
        if (hints.isNotEmpty()) {
            system += "\nPossibly misheard terms (apply ONLY if it clearly fits the context): " +
                hints.joinToString("; ") { "\"${it.first}\" may be \"${it.second}\"" } + "."
        }
        val primary = s.optString("llm_model")
        val models = listOf(primary) + FALLBACK_MODELS.filter { it != primary }
        var lastErr: ApiError? = null
        var result: JSONObject? = null
        for (m in models) {
            try { result = chat(s, m, system, "<transcript>\n$text\n</transcript>"); break }
            catch (e: ApiError) { if (e.code == 404 || e.code == 400) lastErr = e else throw e }
        }
        val v = result ?: throw (lastErr ?: ApiError(0, "Kein Modell erreichbar"))
        var out = v.getJSONArray("choices").getJSONObject(0).getJSONObject("message").optString("content").trim()
        val a = out.indexOf("<think>"); val b = out.indexOf("</think>")
        if (a in 0 until b) out = out.removeRange(a, b + 8)
        out = out.replace("<transcript>", "").replace("</transcript>", "").trim()
        if (out.isEmpty() || out.length > text.length * 3 + 80) return null
        return out
    }

    /** Which systematic recognition fixes did the user make? Returns (wrong, right) pairs. */
    fun extractLearning(s: JSONObject, original: String, corrected: String): List<Pair<String, String>> {
        val system = "You compare a dictated text (original, produced by speech recognition) with the " +
            "user's final edited version. Return a JSON object {\"entries\":[{\"from\":\"...\",\"to\":\"...\"}]}.\n" +
            "Include ONLY systematic recognition fixes the user made: misrecognized names, brand names, technical " +
            "terms or specific words. \"from\" is the wrongly written form, \"to\" the correct form (max 3 words each, " +
            "exact spelling and capitalization of the final text).\n" +
            "Ignore: rewording, added or removed content, grammar or style changes, punctuation, one-off typos. " +
            "At most 8 entries. If nothing qualifies return {\"entries\":[]}."
        val v = chat(s, s.optString("llm_model"), system, "ORIGINAL:\n$original\n\nFINAL:\n$corrected", json = true, temp = 0.0)
        val content = v.getJSONArray("choices").getJSONObject(0).getJSONObject("message").optString("content", "{}")
        val parsed = JSONObject(content.substring(maxOf(0, content.indexOf('{')), content.lastIndexOf('}').let { if (it < 0) content.length else it + 1 }))
        val entries = parsed.optJSONArray("entries") ?: return emptyList()
        val out = ArrayList<Pair<String, String>>()
        for (i in 0 until entries.length()) {
            val e = entries.getJSONObject(i)
            val from = e.optString("from").trim(); val to = e.optString("to").trim()
            if (from.isNotEmpty() && to.isNotEmpty() && from != to && from.split(Regex("\\s+")).size <= 3) out.add(from to to)
        }
        return out
    }

    companion object {
        /** Mirrors DEFAULT_VOCAB / PRIMER in src-tauri/src/groq.rs. */
        val DEFAULT_VOCAB = listOf(
            "Meeting", "Call", "Deadline", "Ticket", "Sprint", "Review", "Pull Request", "Commit", "Merge", "Branch",
            "Feature Flag", "Deployment", "Pipeline", "Build", "Release", "Bug", "Fix", "Backend", "Frontend", "API",
            "JSON", "Server", "Cache", "Config", "Docker", "Kubernetes", "GitHub",
        )
        const val PRIMER = "Ja, also ich hab das gestern mal ausprobiert, und es funktioniert eigentlich ganz gut. Danach muss ich noch das Deployment checken, weil die Pipeline manchmal hängt. Okay, passt."
        val FALLBACK_MODELS = listOf("openai/gpt-oss-120b", "qwen/qwen3.8-27b", "openai/gpt-oss-20b")

        val HALLUCINATIONS = listOf(
            "untertitel", "amara.org", "thanks for watching", "thank you for watching", "danke fürs zuschauen",
            "vielen dank fürs zuschauen", "subtitles by", "subscribe to",
        )

        fun isTransient(err: String) = err.contains("Rate-Limit") || err.contains("Netzwerk") || err.contains("Groq 5")

        fun isPromptEcho(text: String): Boolean {
            fun norm(s: String) = s.lowercase().split(Regex("[^\\p{L}\\p{N}]+")).filter { it.length > 2 }
            val t = norm(text)
            if (t.isEmpty()) return false
            val p = norm(PRIMER)
            val hits = t.count { it in p }
            return t.size >= 3 && hits * 10 >= t.size * 7
        }
    }
}
