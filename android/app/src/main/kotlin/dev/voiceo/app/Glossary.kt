package dev.voiceo.app

import android.content.Context

/** Port of src-tauri/src/glossary.rs (data files are shared via assets). */
object Glossary {
    private class Term(val text: String, val key: String, val skel: String)

    @Volatile private var bundled: List<Term>? = null

    private fun norm(s: String) = s.filter { it.isLetterOrDigit() }.lowercase()

    private fun skeleton(key: String): String {
        var s = key
        for ((a, b) in listOf("ph" to "f", "ck" to "k", "ß" to "s", "ä" to "e", "ö" to "o", "ü" to "u", "ae" to "e", "oe" to "o",
            "ue" to "u", "ch" to "k", "sch" to "s", "th" to "t", "c" to "k", "q" to "k", "v" to "f", "w" to "f", "z" to "s",
            "y" to "i", "x" to "ks")) s = s.replace(a, b)
        val out = StringBuilder(); var prev = '\u0000'
        s.forEachIndexed { i, c ->
            if ("aeiou".indexOf(c) >= 0 && i > 0) return@forEachIndexed
            if (c != prev) out.append(c)
            prev = c
        }
        return out.toString()
    }

    private fun terms(ctx: Context): List<Term> {
        bundled?.let { return it }
        val seen = HashSet<String>()
        val list = ArrayList<Term>()
        for (f in listOf("curated.txt", "auto.txt")) {
            val lines = runCatching { ctx.assets.open(f).bufferedReader().readLines() }.getOrDefault(emptyList())
            for (raw in lines) {
                val l = raw.trim()
                if (l.length < 4) continue
                val key = norm(l)
                if (key.length < 4 || !seen.add(key)) continue
                list.add(Term(l, key, skeleton(key)))
            }
        }
        bundled = list
        return list
    }

    private fun editDistanceLe(a: String, b: String, max: Int): Boolean {
        if (kotlin.math.abs(a.length - b.length) > max) return false
        var prev = IntArray(b.length + 1) { it }
        for (i in 1..a.length) {
            val cur = IntArray(b.length + 1) { i }
            for (j in 1..b.length) {
                val cost = if (a[i - 1] != b[j - 1]) 1 else 0
                cur[j] = minOf(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + cost)
            }
            prev = cur
        }
        return prev[b.length] <= max
    }

    /** (heard, suggested term) pairs for words that look like a glossary term but differ. */
    fun candidates(ctx: Context, text: String, extra: List<String>): List<Pair<String, String>> {
        val words = text.split(Regex("\\s+")).map { w -> w.trim { !it.isLetterOrDigit() } }.filter { it.isNotEmpty() }
        val spans = ArrayList<String>(words)
        for (i in 0 until words.size - 1) spans.add(words[i] + " " + words[i + 1])
        val user = extra.filter { norm(it).length >= 3 }.map { val k = norm(it); Term(it, k, skeleton(k)) }
        val all = terms(ctx) + user
        val out = ArrayList<Pair<String, String>>()
        for (span in spans) {
            val key = norm(span)
            if (key.length < 4) continue
            val skel = skeleton(key)
            val max = if (key.length >= 9) 2 else 1
            var exact = false
            var best: Term? = null
            for (t in all) {
                if (t.key == key) { exact = true; break }
                val near = editDistanceLe(key, t.key, max)
                val phon = skel.length >= 3 && t.skel == skel && key[0] == t.key[0]
                if ((near || phon) && best == null) best = t
            }
            if (!exact && best != null && out.none { it.second == best.text }) out.add(span to best.text)
            if (out.size >= 12) break
        }
        return out
    }
}
