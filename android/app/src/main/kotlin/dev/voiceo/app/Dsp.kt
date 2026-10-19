package dev.voiceo.app

import java.io.ByteArrayOutputStream
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sqrt

/** Audio helpers, ports of src-tauri/src/audio.rs. All audio is 16 kHz mono float in [-1, 1]. */
object Dsp {
    const val RATE = 16000
    const val BAND_COUNT = 16

    fun peak(x: FloatArray): Float { var m = 0f; for (s in x) m = max(m, abs(s)); return m }

    fun highpass(x: FloatArray) {
        val rc = 1f / (2f * PI.toFloat() * 80f)
        val dt = 1f / RATE
        val a = rc / (rc + dt)
        var px = 0f; var py = 0f
        for (i in x.indices) { val v = x[i]; val y = a * (py + v - px); px = v; py = y; x[i] = y }
    }

    fun normalize(x: FloatArray) {
        val p = peak(x)
        if (p > 0.0005f && p < 0.7f) { val g = min(0.85f / p, 16f); for (i in x.indices) x[i] *= g }
    }

    /** Goertzel magnitudes on 16 log-spaced bands (90 Hz - 6.5 kHz) for the overlay visualizer. */
    fun bands(win: FloatArray, rate: Int = RATE): FloatArray {
        val n = win.size.toFloat()
        val out = FloatArray(BAND_COUNT)
        if (win.size < 8) return out
        for (i in 0 until BAND_COUNT) {
            val f = 90f * (6500f / 90f).pow(i / (BAND_COUNT - 1f))
            val w = 2f * PI.toFloat() * f / rate
            val coeff = 2f * cos(w)
            var s1 = 0f; var s2 = 0f
            for (k in win.indices) {
                val hann = 0.5f - 0.5f * cos(2f * PI.toFloat() * k / (n - 1f))
                val s = win[k] * hann + coeff * s1 - s2
                s2 = s1; s1 = s
            }
            val p = s1 * s1 + s2 * s2 - coeff * s1 * s2
            out[i] = sqrt(max(0f, p)) * 4f / n
        }
        return out
    }

    /**
     * Inclusive VAD: trims leading/trailing silence and shortens pauses longer than 1 s. Returns null
     * when there is no real speech. Port of audio.rs vad_soft / tools/eval/evalkit.py.
     */
    fun vadSoft(x: FloatArray): FloatArray? {
        val loMul = 2.2f; val loMin = 0.0025f
        val win = RATE / 50
        if (x.size < win * 5) return null
        val nWin = (x.size + win - 1) / win
        val energy = FloatArray(nWin) { w ->
            val a = w * win; val b = min(x.size, a + win)
            var sum = 0f; for (i in a until b) sum += x[i] * x[i]
            sqrt(sum / (b - a))
        }
        val sorted = energy.sortedArray()
        val thr = max(sorted[nWin / 10] * loMul, loMin)
        val voiced = energy.indices.filter { energy[it] > thr }
        if (voiced.size < 10) return null
        val merge = 35
        val segs = ArrayList<IntArray>()
        var s0 = voiced[0]; var prev = voiced[0]
        for (i in voiced.drop(1)) { if (i - prev > merge) { segs.add(intArrayOf(s0, prev)); s0 = i }; prev = i }
        segs.add(intArrayOf(s0, prev))
        segs.removeAll { it[1] - it[0] + 1 < 4 }
        if (segs.isEmpty()) return null
        val pad = 15
        val longGap = RATE; val keepGap = (RATE * 0.4f).toInt()
        val out = ArrayList<Float>(x.size)
        var lastEnd = -1
        for ((a, b) in segs.map { it[0] to it[1] }) {
            var a0 = max(0, a - pad) * win
            val b0 = min(min(b + 1 + pad, nWin) * win, x.size)
            if (lastEnd >= 0) {
                if (a0 < lastEnd) a0 = lastEnd
                if (a0 > lastEnd) {
                    val gap = a0 - lastEnd
                    val take = if (gap > longGap) keepGap else gap
                    for (i in lastEnd until lastEnd + take) out.add(x[i])
                }
            }
            for (i in a0 until b0) out.add(x[i])
            lastEnd = b0
        }
        return out.toFloatArray()
    }

    fun toWav(x: FloatArray): ByteArray {
        val data = ByteArray(x.size * 2)
        for (i in x.indices) {
            val v = (x[i].coerceIn(-1f, 1f) * 32767f).toInt()
            data[2 * i] = (v and 0xff).toByte(); data[2 * i + 1] = ((v shr 8) and 0xff).toByte()
        }
        val out = ByteArrayOutputStream(44 + data.size)
        fun le32(v: Int) { out.write(v and 0xff); out.write((v shr 8) and 0xff); out.write((v shr 16) and 0xff); out.write((v shr 24) and 0xff) }
        fun le16(v: Int) { out.write(v and 0xff); out.write((v shr 8) and 0xff) }
        out.write("RIFF".toByteArray()); le32(36 + data.size); out.write("WAVEfmt ".toByteArray())
        le32(16); le16(1); le16(1); le32(RATE); le32(RATE * 2); le16(2); le16(16)
        out.write("data".toByteArray()); le32(data.size); out.write(data)
        return out.toByteArray()
    }
}
