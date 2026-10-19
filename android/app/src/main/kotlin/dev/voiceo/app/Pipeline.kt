package dev.voiceo.app

import android.content.Context

/** audio -> Whisper -> cleanup -> learned replacements. Port of run_pipeline in src-tauri/src/main.rs. */
object Pipeline {
    class Out(val raw: String, val text: String, val warn: String?)

    /** Returns null when there was nothing worth inserting (silence, hallucination). Throws Groq.ApiError. */
    fun run(ctx: Context, store: Store, groq: Groq, samples: FloatArray): Out? {
        val s = store.settings()
        val pcm = samples.copyOf()
        if (Dsp.peak(pcm) < s.optDouble("silence_threshold", 0.01)) return null
        if (s.optBoolean("highpass")) Dsp.highpass(pcm)
        var audio = pcm
        if (s.optBoolean("trim_silence", true)) audio = Dsp.vadSoft(pcm) ?: return null
        if (s.optBoolean("normalize")) Dsp.normalize(audio)
        val wav = Dsp.toWav(audio)

        val learned = store.learnedItems()
        val terms = LinkedHashSet<String>().apply { learned.forEach { add(it.to) }; s.strs("dictionary").forEach { add(it) } }.toList().reversed()

        val raw = groq.transcribe(s, wav, terms)
        val lower = raw.lowercase()
        if (raw.isEmpty() || Groq.isPromptEcho(raw) || (raw.length < 90 && Groq.HALLUCINATIONS.any { lower.contains(it) })) return null

        var text = raw
        var warn: String? = null
        val words = raw.trim().split(Regex("\\s+")).size
        if (s.optBoolean("cleanup", true) && s.optString("mode") != "raw" && words >= s.optInt("min_words_cleanup", 3)) {
            try {
                val hints = Glossary.candidates(ctx, raw, terms)
                groq.cleanup(s, raw, hints, terms)?.let { text = it }
            } catch (e: Groq.ApiError) {
                warn = "Cleanup fehlgeschlagen: ${e.message}"
            }
        }
        text = Learn.apply(text, learned)
        return Out(raw, text, warn)
    }
}
