package dev.voiceo.app

data class Learned(val from: String, val to: String, val count: Int)

/** Port of src-tauri/src/learn.rs. */
object Learn {
    fun add(items: MutableList<Learned>, fromRaw: String, toRaw: String) {
        val from = fromRaw.trim(); val to = toRaw.trim()
        if (to.isEmpty() || from == to) return
        val same = items.indexOfFirst { it.from == from && it.to == to }
        if (same >= 0) { items[same] = items[same].copy(count = items[same].count + 1); return }
        if (from.isNotEmpty()) {
            val byFrom = items.indexOfFirst { it.from == from }
            if (byFrom >= 0) { items[byFrom] = Learned(from, to, items[byFrom].count + 1); return }
        }
        items.add(Learned(from, to, 1))
    }

    private fun tokens(s: String): List<String> =
        s.split(Regex("\\s+")).map { w -> w.trim { !it.isLetterOrDigit() } }.filter { it.isNotEmpty() }

    /** Small word-level replacements between old and new. */
    fun diffPairs(old: String, new: String): List<Pair<String, String>> {
        val a = tokens(old); val b = tokens(new)
        val n = a.size; val m = b.size
        if (n == 0 || m == 0) return emptyList()
        val dp = Array(n + 1) { IntArray(m + 1) }
        for (i in n - 1 downTo 0) for (j in m - 1 downTo 0) {
            dp[i][j] = if (a[i] == b[j]) dp[i + 1][j + 1] + 1 else maxOf(dp[i + 1][j], dp[i][j + 1])
        }
        val pairs = mutableListOf<Pair<String, String>>()
        val dels = mutableListOf<String>(); val ins = mutableListOf<String>()
        fun flush() {
            if (dels.isNotEmpty() && ins.isNotEmpty() && dels.size <= 3 && ins.size <= 3) pairs.add(dels.joinToString(" ") to ins.joinToString(" "))
            dels.clear(); ins.clear()
        }
        var i = 0; var j = 0
        while (i < n || j < m) {
            if (i < n && j < m && a[i] == b[j]) { flush(); i++; j++ }
            else if (j < m && (i == n || dp[i][j + 1] >= dp[i + 1][j])) { ins.add(b[j]); j++ }
            else { dels.add(a[i]); i++ }
        }
        flush()
        return pairs
    }

    private fun lc(c: Char) = c.lowercaseChar()

    private fun replaceCi(text: String, from: String, to: String): String {
        if (from.isEmpty()) return text
        val sb = StringBuilder()
        var i = 0
        while (i < text.length) {
            val end = i + from.length
            val hit = end <= text.length &&
                (0 until from.length).all { lc(text[i + it]) == lc(from[it]) } &&
                (i == 0 || !text[i - 1].isLetterOrDigit()) &&
                (end == text.length || !text[end].isLetterOrDigit())
            if (hit) {
                if (text[i].isUpperCase() && to.isNotEmpty() && to[0].isLowerCase()) sb.append(to[0].uppercaseChar()).append(to.substring(1))
                else sb.append(to)
                i = end
            } else { sb.append(text[i]); i++ }
        }
        return sb.toString()
    }

    fun apply(text: String, items: List<Learned>): String {
        var out = text
        for (it in items.filter { it.from.isNotEmpty() }.sortedByDescending { it.from.length }) out = replaceCi(out, it.from, it.to)
        return out
    }
}
